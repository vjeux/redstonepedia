//! The design text format: a contraption as human-readable layers.
//!
//! ```text
//! # comment
//! name: tiny-demo
//! origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
//! port lever: 2 1 0              # named positions (inputs, outputs, ...)
//! palette:
//!   S minecraft:stone
//!   P minecraft:sticky_piston[facing=up]
//!   L minecraft:lever[face=floor,facing=east]
//! layer 0:
//!   S.
//! layer 1:
//!   PL
//! ```
//!
//! Rows run north to south (z grows downward), columns west to east (x grows rightward).
//! `.` and spaces are air. Every other character must be in the palette.
//!
//! Entities (minecarts, armor stands, boats...) are header lines, world coordinates as decimals
//! (not shifted by `origin`), with optional SNBT for the entity's own data (rotation, contents):
//!
//! ```text
//! entity: minecraft:minecart 0.5 1.0625 3.5
//! entity: minecraft:armor_stand 2.5 1 0.5 {Rotation:[90f,0f]}
//! ```
//!
//! Block-entity data (container contents: the items of a hopper, a dropper, a crafter...) are
//! header lines too, one per block, world coordinates like `port:` (not shifted by `origin`), the
//! SNBT merged into the block entity right after the raw paste exactly like
//! `data merge block x y z <snbt>` ([`apply_block_entities`]):
//!
//! ```text
//! nbt: 1 0 0 {Items:[{Slot:0b,id:"minecraft:stone",count:3}]}
//! ```

use crate::blockstate::BlockState;
use crate::sim::{Pos, Worker};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::path::Path;

/// An entity of a design: its type (`minecraft:minecart`), its position (the entity's feet, world
/// coordinates as decimals) and optional SNBT data (`{Rotation:[90f,0f]}`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntitySpec {
    pub kind: String,
    pub pos: [f64; 3],
    pub nbt: Option<String>,
}

// Positions are finite decimals read from text (never NaN), so equality is total.
impl Eq for EntitySpec {}

impl EntitySpec {
    /// `minecraft:minecart 0.5 1.0625 3.5 [{snbt}]` (the text after `entity:`).
    pub fn parse(s: &str) -> Result<EntitySpec> {
        fn next_word(s: &str) -> (&str, &str) {
            let r = s.trim_start();
            let end = r.find(char::is_whitespace).unwrap_or(r.len());
            r.split_at(end)
        }
        let s = s.trim();
        let usage = || anyhow!("expected 'entity: <type> <x> <y> <z> [snbt]', got {s:?}");
        let (kind, mut rest) = next_word(s);
        if kind.is_empty() {
            return Err(usage());
        }
        let mut pos = [0.0f64; 3];
        for p in pos.iter_mut() {
            let (w, r) = next_word(rest);
            rest = r;
            if w.is_empty() {
                return Err(usage());
            }
            *p = w.parse::<f64>().with_context(|| format!("bad entity coordinate {w:?} in {s:?}"))?;
            if !p.is_finite() {
                bail!("bad entity coordinate {w:?} in {s:?}");
            }
        }
        let nbt = rest.trim();
        if !nbt.is_empty() && !nbt.starts_with('{') {
            bail!("entity data must be SNBT in braces, got {nbt:?}");
        }
        let kind = if kind.contains(':') { kind.to_string() } else { format!("minecraft:{kind}") };
        Ok(EntitySpec { kind, pos, nbt: (!nbt.is_empty()).then(|| nbt.to_string()) })
    }

    /// The text after `entity: ` ([`EntitySpec::parse`] reads it back exactly).
    pub fn to_text(&self) -> String {
        let [x, y, z] = self.pos;
        match &self.nbt {
            Some(n) => format!("{} {x} {y} {z} {n}", self.kind),
            None => format!("{} {x} {y} {z}", self.kind),
        }
    }

    /// The block cell the entity's position is in.
    pub fn cell(&self) -> Pos {
        [self.pos[0].floor() as i32, self.pos[1].floor() as i32, self.pos[2].floor() as i32]
    }
}

/// How [`Design::parse_with`] reads a whitespace-only line inside a layer that more rows of the
/// same layer follow (farmopt, Nov 12 2026, ironspec's ruling: ironfarm's lanes file wrote its
/// empty rows as two spaces, which the parser skipped, so every later row of the layer moved one
/// north and the file loaded as a different design). A blank line at a layer's end (before the
/// next header line or the end of the file) is a separator in every reading, and a comment-only
/// line is never a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankRows {
    /// Refuse the file, naming the line (what [`Design::parse`] does: nothing silently dropped).
    Refuse,
    /// The old reading: no row, so the layer's later rows move north.
    Skip,
    /// An empty row (what a writer that emits two spaces for an empty row meant).
    Air,
}

/// The 0-based line numbers of the whitespace-only lines inside a layer (or a Nether layer) that
/// more rows of the same layer follow: the lines [`BlankRows`] decides (`mctas design blank-rows`).
pub fn interior_blank_rows(text: &str) -> Vec<usize> {
    let mut in_layer = false;
    let mut pending: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            if in_layer {
                pending.push(lineno);
            }
            continue;
        }
        if raw.starts_with("  ") {
            if in_layer {
                out.append(&mut pending);
            }
            continue;
        }
        // a header line (a comment-only line is neither a row nor the layer's end)
        let l = raw.split('#').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        pending.clear();
        in_layer = (l.starts_with("layer ") || l.starts_with("nether layer ")) && l.ends_with(':');
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Design {
    pub name: String,
    /// Other `key: value` header lines, in order of keys.
    pub meta: BTreeMap<String, String>,
    pub ports: BTreeMap<String, Pos>,
    /// Non-air blocks.
    pub blocks: BTreeMap<Pos, String>,
    /// Entities (`entity:` lines), in file order. Most designs have none.
    pub entities: Vec<EntitySpec>,
    /// Block-entity data (`nbt: x y z {snbt}` lines): SNBT merged into the block entity at that
    /// position after the raw paste ([`apply_block_entities`]). Most designs have none.
    pub block_entities: BTreeMap<Pos, String>,
    /// (tickexact 2026-11-10, ironspec's portal pad) A mob farm's Nether part: its blocks
    /// (`nether layer Y:` sections, from `nether-origin:`, sharing the palette), in coordinates
    /// relative to the Nether image of the design's anchor (port `nether-anchor`, the design's
    /// 0 0 0 at first, moved and turned with the design): placed at floor(anchor / 8) in x and z
    /// (y as written), turned by the design's quarter turns (port `nether-axis` minus the anchor).
    /// Empty for every other design.
    pub nether_blocks: BTreeMap<Pos, String>,
    /// Its ports (`port nether-<name>:`, the Nether output chest `nether-output`), in the same
    /// coordinates.
    pub nether_ports: BTreeMap<String, Pos>,
    /// (farmopt 2026-11-10) Its block-entity data (`nether nbt: x y z {snbt}`, in the same
    /// coordinates), merged after the part's paste: a sorter's stock, a crafter's slots.
    pub nether_block_entities: BTreeMap<Pos, String>,
}

/// `nbt: x y z {snbt}` (the text after `nbt:`): the position and the SNBT.
fn parse_block_entity(s: &str) -> Result<(Pos, String)> {
    let s = s.trim();
    let usage = || anyhow!("expected 'nbt: <x> <y> <z> {{snbt}}', got {s:?}");
    let brace = s.find('{').ok_or_else(usage)?;
    let pos = parse_pos(&s[..brace]).map_err(|_| usage())?;
    let nbt = s[brace..].trim();
    if !nbt.ends_with('}') {
        return Err(usage());
    }
    Ok((pos, nbt.to_string()))
}

/// Write a design's block-entity data into a sandbox where its blocks were just pasted: every
/// `nbt:` line merged like `data merge block` ([`Worker::merge_block_entity`]: loadWithComponents
/// + setChanged, so comparators reading a container get notified), in position order. Placement
/// order (clock judge, `door drive`): raw paste, this, wire shapes, self updates.
pub fn apply_block_entities(w: &mut Worker, sandbox: u32, d: &Design) -> Result<()> {
    for (p, nbt) in &d.block_entities {
        w.merge_block_entity(sandbox, *p, nbt).with_context(|| format!("nbt: {} {} {} {nbt}", p[0], p[1], p[2]))?;
    }
    Ok(())
}

fn parse_pos(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split_whitespace().map(|t| t.parse::<i32>()).collect::<Result<_, _>>()?;
    if v.len() != 3 {
        bail!("expected 'x y z', got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

impl Design {
    pub fn load(path: &Path) -> Result<Design> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Design::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Design> {
        Design::parse_with(text, BlankRows::Refuse)
    }

    /// `parse` with a chosen reading of a blank line inside a layer that more rows of the same
    /// layer follow ([`BlankRows`]); `mctas design blank-rows` compares the two old readings.
    pub fn parse_with(text: &str, blank_rows: BlankRows) -> Result<Design> {
        enum Section {
            Header,
            Palette,
            Layer(i32),
            NetherLayer(i32),
        }
        // a merge left unresolved: the file is not a design, and silently parsing the HEAD half
        // archives a broken file (clk-item 2026-10-20: markers reached main and broke the site)
        if let Some((n, l)) = text.lines().enumerate().find(|(_, l)| l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> ") || *l == "=======") {
            bail!("line {}: {l:?}: unresolved merge markers, not a design", n + 1);
        }
        let mut d = Design::default();
        let mut origin: Pos = [0, 0, 0];
        let mut nether_origin: Pos = [0, 0, 0];
        let mut palette: BTreeMap<char, String> = BTreeMap::new();
        let mut section = Section::Header;
        let mut row = 0i32;
        // blank lines seen inside the current layer, not yet known to be its end (a separator) or
        // an interior row (more rows of the layer follow)
        let mut pending_blank: Vec<usize> = Vec::new();
        for (lineno, raw) in text.lines().enumerate() {
            let err = || format!("line {}: {raw:?}", lineno + 1);
            let line = match raw.find('#') {
                // '#' starts a comment except inside a layer row, where it can be a palette char.
                Some(i) if !matches!(section, Section::Layer(_) | Section::NetherLayer(_)) || !raw.starts_with("  ") => &raw[..i],
                _ => raw,
            };
            if line.trim().is_empty() {
                // a comment-only line is no row; a whitespace-only line inside a layer may be one
                if raw.trim().is_empty() && matches!(section, Section::Layer(_) | Section::NetherLayer(_)) {
                    pending_blank.push(lineno);
                }
                continue;
            }
            let indented = line.starts_with("  ");
            if !indented {
                pending_blank.clear();
                let l = line.trim();
                if l == "palette:" {
                    section = Section::Palette;
                } else if let Some(y) = l.strip_prefix("nether layer ").and_then(|r| r.strip_suffix(':')) {
                    section = Section::NetherLayer(y.trim().parse().with_context(err)?);
                    row = 0;
                } else if let Some(y) = l.strip_prefix("layer ").and_then(|r| r.strip_suffix(':')) {
                    section = Section::Layer(y.trim().parse().with_context(err)?);
                    row = 0;
                } else if let Some(rest) = l.strip_prefix("port ") {
                    let (name, pos) = rest.split_once(':').ok_or_else(|| anyhow!("{}", err()))?;
                    // `port <name>: x y z [args...]`: words after the position (a clock's start
                    // kick: `port start: 1 0 0 minecraft:redstone_block 1`) are kept in `meta`
                    // under `port <name>` and written back on the port line.
                    let words: Vec<&str> = pos.split_whitespace().collect();
                    let (xyz, args) = if words.len() > 3 { (words[..3].join(" "), words[3..].join(" ")) } else { (pos.to_string(), String::new()) };
                    let name = name.trim();
                    // a Nether part's own ports (its output chest) are in its coordinates
                    if name.starts_with("nether-") && name != "nether-anchor" && name != "nether-axis" {
                        d.nether_ports.insert(name.to_string(), parse_pos(&xyz).with_context(err)?);
                    } else {
                        d.ports.insert(name.to_string(), parse_pos(&xyz).with_context(err)?);
                    }
                    if !args.is_empty() {
                        d.meta.insert(format!("port {name}"), args);
                    }
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("entity:") {
                    d.entities.push(EntitySpec::parse(rest).with_context(err)?);
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("nether nbt:") {
                    let (p, nbt) = parse_block_entity(rest).with_context(err)?;
                    if d.nether_block_entities.insert(p, nbt).is_some() {
                        bail!("two nether nbt: lines for {p:?} at {}", err());
                    }
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("nbt:") {
                    let (p, nbt) = parse_block_entity(rest).with_context(err)?;
                    if d.block_entities.insert(p, nbt).is_some() {
                        bail!("two nbt: lines for {p:?} at {}", err());
                    }
                    section = Section::Header;
                } else if let Some((k, v)) = l.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "name" => d.name = v.to_string(),
                        "origin" => origin = parse_pos(v).with_context(err)?,
                        "nether-origin" => nether_origin = parse_pos(v).with_context(err)?,
                        _ => {
                            d.meta.insert(k.to_string(), v.to_string());
                        }
                    }
                    section = Section::Header;
                } else {
                    bail!("unexpected {}", err());
                }
                continue;
            }
            if !pending_blank.is_empty() && matches!(section, Section::Layer(_) | Section::NetherLayer(_)) {
                match blank_rows {
                    BlankRows::Refuse => {
                        let at = pending_blank[0];
                        bail!(
                            "line {}: a blank line inside a layer with more rows of it after: write an empty row as `  .` (a blank line is read as no row, which moves every later row of the layer)",
                            at + 1
                        );
                    }
                    BlankRows::Skip => {}
                    BlankRows::Air => row += pending_blank.len() as i32,
                }
                pending_blank.clear();
            }
            match section {
                Section::Header => bail!("indented line outside palette/layer: {}", err()),
                Section::Palette => {
                    let body = line.trim();
                    let mut chars = body.chars();
                    let c = chars.next().unwrap();
                    let state = chars.as_str().trim();
                    if c == '.' || state.is_empty() {
                        bail!("bad palette entry {}", err());
                    }
                    let parsed = BlockState::parse(state).with_context(err)?;
                    palette.insert(c, parsed.to_string());
                }
                Section::Layer(y) => {
                    let body = &line[2..];
                    for (col, c) in body.chars().enumerate() {
                        if c == '.' || c == ' ' {
                            continue;
                        }
                        let state = palette.get(&c).ok_or_else(|| anyhow!("unknown palette char {c:?} at {}", err()))?;
                        let pos = [origin[0] + col as i32, origin[1] + y, origin[2] + row];
                        d.blocks.insert(pos, state.clone());
                    }
                    row += 1;
                }
                Section::NetherLayer(y) => {
                    let body = &line[2..];
                    for (col, c) in body.chars().enumerate() {
                        if c == '.' || c == ' ' {
                            continue;
                        }
                        let state = palette.get(&c).ok_or_else(|| anyhow!("unknown palette char {c:?} at {}", err()))?;
                        let pos = [nether_origin[0] + col as i32, nether_origin[1] + y, nether_origin[2] + row];
                        d.nether_blocks.insert(pos, state.clone());
                    }
                    row += 1;
                }
            }
        }
        // a Nether part's anchor: the design's 0 0 0 and its x axis, moved and turned with it
        if !d.nether_blocks.is_empty() || !d.nether_ports.is_empty() || !d.nether_block_entities.is_empty() {
            match (d.ports.contains_key("nether-anchor"), d.ports.contains_key("nether-axis")) {
                (false, false) => {
                    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
                    d.ports.insert("nether-axis".into(), [1, 0, 0]);
                }
                (true, true) => {}
                _ => bail!("a Nether part needs both `port nether-anchor:` and `port nether-axis:`, or neither"),
            }
        }
        Ok(d)
    }

    /// (tickexact) A Nether part's quarter turns (clockwise from above, as the design's turns):
    /// how far port `nether-axis` stands from `nether-anchor`; None without a Nether part or for
    /// a broken pair.
    pub fn nether_turns(&self) -> Option<u8> {
        let a = self.ports.get("nether-anchor")?;
        let b = self.ports.get("nether-axis")?;
        match [b[0] - a[0], b[1] - a[1], b[2] - a[2]] {
            [1, 0, 0] => Some(0),
            [0, 0, 1] => Some(1),
            [-1, 0, 0] => Some(2),
            [0, 0, -1] => Some(3),
            _ => None,
        }
    }

    /// Inclusive bounding box of the blocks.
    pub fn bounds(&self) -> Option<(Pos, Pos)> {
        let mut it = self.blocks.keys();
        let first = *it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Some((lo, hi))
    }

    /// Number of non-air blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// The same design moved by `d` (ports, entities and block-entity data included).
    pub fn translated(&self, d: Pos) -> Design {
        let mv = |p: &Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Design {
            name: self.name.clone(),
            meta: self.meta.clone(),
            ports: self.ports.iter().map(|(k, p)| (k.clone(), mv(p))).collect(),
            blocks: self.blocks.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
            block_entities: self.block_entities.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
            entities: self
                .entities
                .iter()
                .map(|e| EntitySpec {
                    pos: [e.pos[0] + d[0] as f64, e.pos[1] + d[1] as f64, e.pos[2] + d[2] as f64],
                    ..e.clone()
                })
                .collect(),
            // the Nether part follows its anchor port (moved above)
            nether_blocks: self.nether_blocks.clone(),
            nether_ports: self.nether_ports.clone(),
            nether_block_entities: self.nether_block_entities.clone(),
        }
    }

    /// A key identifying the design's content (blocks, ports, and entities and block-entity data
    /// when it has any), for deduplication and verdict caches: a design without entities or
    /// block-entity data keeps its old key.
    pub fn content_key(&self) -> String {
        let mut key = format!("{:?}|{:?}", self.blocks, self.ports);
        if !self.entities.is_empty() {
            let ents: Vec<String> = self.entities.iter().map(|e| e.to_text()).collect();
            key.push_str(&format!("|{ents:?}"));
        }
        if !self.block_entities.is_empty() {
            key.push_str(&format!("|nbt{:?}", self.block_entities));
        }
        if !self.nether_blocks.is_empty() || !self.nether_ports.is_empty() {
            key.push_str(&format!("|nether{:?}{:?}", self.nether_blocks, self.nether_ports));
        }
        if !self.nether_block_entities.is_empty() {
            key.push_str(&format!("|nethernbt{:?}", self.nether_block_entities));
        }
        key
    }

    /// Blocks as a list for [`crate::sim::Worker::set_blocks`].
    pub fn block_list(&self) -> Vec<(Pos, String)> {
        self.blocks.iter().map(|(p, s)| (*p, s.clone())).collect()
    }

    /// Serialize; palette characters are picked from the block names.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if !self.name.is_empty() {
            out.push_str(&format!("name: {}\n", self.name));
        }
        for (k, v) in &self.meta {
            if k.starts_with("port ") {
                continue;
            }
            out.push_str(&format!("{k}: {v}\n"));
        }
        for (k, p) in &self.ports {
            match self.meta.get(&format!("port {k}")) {
                Some(args) => out.push_str(&format!("port {k}: {} {} {} {args}\n", p[0], p[1], p[2])),
                None => out.push_str(&format!("port {k}: {} {} {}\n", p[0], p[1], p[2])),
            }
        }
        for (k, p) in &self.nether_ports {
            out.push_str(&format!("port {k}: {} {} {}\n", p[0], p[1], p[2]));
        }
        for e in &self.entities {
            out.push_str(&format!("entity: {}\n", e.to_text()));
        }
        for (p, nbt) in &self.block_entities {
            out.push_str(&format!("nbt: {} {} {} {nbt}\n", p[0], p[1], p[2]));
        }
        for (p, nbt) in &self.nether_block_entities {
            out.push_str(&format!("nether nbt: {} {} {} {nbt}\n", p[0], p[1], p[2]));
        }
        let Some((lo, hi)) = self.bounds() else { return out };
        out.push_str(&format!("origin: {} 0 {}\n", lo[0], lo[2]));
        // Assign palette characters: uppercase initial, then lowercase, then a fallback pool.
        let mut states: Vec<&String> = self.blocks.values().chain(self.nether_blocks.values()).collect();
        states.sort();
        states.dedup();
        let mut used: BTreeMap<char, ()> = BTreeMap::new();
        let mut chars: BTreeMap<&String, char> = BTreeMap::new();
        let pool: Vec<char> =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$%&*+=?!<>^~/\\|;,'`\"(){}"
                .chars()
                // past the ASCII pool (a design with more than ~90 states: the slab fixture),
                // Latin-1 letters, then Greek and Cyrillic ones
                .chain(('À'..='ÿ').filter(|c| c.is_alphabetic()))
                .chain(('Α'..='ω').filter(|c| c.is_alphabetic()))
                .chain(('Ѐ'..='ӿ').filter(|c| c.is_alphabetic()))
                .collect();
        for s in &states {
            let short = BlockState::parse(s).map(|b| b.short_name().to_string()).unwrap_or_default();
            let initial = short.chars().next().unwrap_or('X');
            let candidates = [initial.to_ascii_uppercase(), initial.to_ascii_lowercase()];
            let c = candidates
                .into_iter()
                .chain(pool.iter().copied())
                .find(|c| !used.contains_key(c))
                .expect("palette exhausted");
            used.insert(c, ());
            chars.insert(s, c);
        }
        out.push_str("palette:\n");
        let mut by_char: Vec<(char, &String)> = chars.iter().map(|(s, c)| (*c, *s)).collect();
        by_char.sort();
        for (c, s) in by_char {
            out.push_str(&format!("  {c} {s}\n"));
        }
        for y in lo[1]..=hi[1] {
            out.push_str(&format!("layer {y}:\n"));
            for z in lo[2]..=hi[2] {
                let mut row = String::from("  ");
                for x in lo[0]..=hi[0] {
                    row.push(self.blocks.get(&[x, y, z]).map(|s| chars[s]).unwrap_or('.'));
                }
                let trimmed = row.trim_end_matches('.');
                // An all-air row must stay visible or the parser would skip it and shift the rows.
                out.push_str(if trimmed.len() <= 2 { "  ." } else { trimmed });
                out.push('\n');
            }
        }
        // the Nether part (its own origin and layers, the same palette)
        let mut it = self.nether_blocks.keys();
        if let Some(first) = it.next() {
            let (mut lo, mut hi) = (*first, *first);
            for p in it {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
            out.push_str(&format!("nether-origin: {} 0 {}\n", lo[0], lo[2]));
            for y in lo[1]..=hi[1] {
                out.push_str(&format!("nether layer {y}:\n"));
                for z in lo[2]..=hi[2] {
                    let mut row = String::from("  ");
                    for x in lo[0]..=hi[0] {
                        row.push(self.nether_blocks.get(&[x, y, z]).map(|s| chars[s]).unwrap_or('.'));
                    }
                    let trimmed = row.trim_end_matches('.');
                    out.push_str(if trimmed.len() <= 2 { "  ." } else { trimmed });
                    out.push('\n');
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_line_inside_a_layer_is_refused_and_its_two_readings_differ() {
        // (farmopt, Nov 12 2026, ironspec's ruling) ironfarm's lanes file wrote an empty row as
        // two spaces; the old parser skipped it and the layer's later rows moved one north.
        let text = "name: b\npalette:\n  S stone\nlayer 0:\n  S\n  \n  S\n\nlayer 1:\n  S\n\n# a comment line\n";
        assert_eq!(interior_blank_rows(text), vec![5]);
        let err = Design::parse(text).unwrap_err().to_string();
        assert!(err.contains("line 6") && err.contains("`  .`"), "{err}");
        let skip = Design::parse_with(text, BlankRows::Skip).unwrap();
        let air = Design::parse_with(text, BlankRows::Air).unwrap();
        assert!(skip.blocks.contains_key(&[0, 0, 1]) && !skip.blocks.contains_key(&[0, 0, 2]));
        assert!(air.blocks.contains_key(&[0, 0, 2]) && !air.blocks.contains_key(&[0, 0, 1]));
        // the explicit empty row parses as the air reading
        let explicit = text.replacen("  S\n  \n", "  S\n  .\n", 1);
        assert_eq!(Design::parse(&explicit).unwrap(), air);
        // a blank line at a layer's end, a comment-only line inside a layer, an empty line
        // between rows of a Nether layer
        let ok = "name: c\npalette:\n  S stone\nlayer 0:\n  S\n# note\n  S\n  \nlayer 1:\n  S\n";
        assert!(interior_blank_rows(ok).is_empty());
        assert!(Design::parse(ok).unwrap().blocks.contains_key(&[0, 0, 1]));
        let nether = "name: n\npalette:\n  S stone\nlayer 0:\n  S\nnether layer 64:\n  S\n\n  S\n";
        assert_eq!(interior_blank_rows(nether), vec![7]);
        assert!(Design::parse(nether).is_err());
        assert!(Design::parse_with(nether, BlankRows::Air).unwrap().nether_blocks.contains_key(&[0, 64, 2]));
    }

    #[test]
    fn nether_part_round_trip() {
        let text = "name: n\nport output: 0 0 0\npalette:\n  S stone\n  P nether_portal[axis=z]\n  H hopper\nlayer 0:\n  S\nnether-origin: 2 0 3\nnether layer 64:\n  SPH\nport nether-output: 4 64 3\nnether nbt: 4 64 3 {Items:[{Slot:0b,id:\"minecraft:paper\",count:1}]}\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.nether_blocks[&[2, 64, 3]], "minecraft:stone");
        assert_eq!(d.nether_blocks[&[3, 64, 3]], "minecraft:nether_portal[axis=z]");
        assert_eq!(d.nether_ports["nether-output"], [4, 64, 3]);
        assert_eq!(d.nether_block_entities[&[4, 64, 3]], "{Items:[{Slot:0b,id:\"minecraft:paper\",count:1}]}");
        assert!(d.block_entities.is_empty());
        assert!(d.content_key().contains("|nethernbt"));
        assert_eq!(d.ports["nether-anchor"], [0, 0, 0]);
        assert_eq!(d.ports["nether-axis"], [1, 0, 0]);
        assert_eq!(d.nether_turns(), Some(0));
        assert!(!d.ports.contains_key("nether-output"));
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        // moved: the anchor moves, the part (relative to its image) stays
        let moved = d.translated([9, 1, -17]);
        assert_eq!(moved.nether_blocks, d.nether_blocks);
        assert_eq!(moved.nether_block_entities, d.nether_block_entities);
        assert_eq!(moved.ports["nether-anchor"], [9, 1, -17]);
        assert_eq!(moved.nether_turns(), Some(0));
    }

    #[test]
    fn round_trip() {
        let text = "name: demo\nport lever: 1 1 0\npalette:\n  S stone\n  P sticky_piston[facing=up]\nlayer 0:\n  S.\n  .S\nlayer 1:\n  P\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.block_count(), 3);
        assert_eq!(d.blocks[&[0, 1, 0]], "minecraft:sticky_piston[facing=up]");
        assert_eq!(d.blocks[&[1, 0, 1]], "minecraft:stone");
        assert_eq!(d.ports["lever"], [1, 1, 0]);
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again.blocks, d.blocks);
        assert_eq!(again.ports, d.ports);
        assert!(again.entities.is_empty());
        assert_eq!(again.content_key(), format!("{:?}|{:?}", d.blocks, d.ports));
    }

    #[test]
    fn port_args_round_trip() {
        let text = "name: c\nport output: 1 0 0\nport start: 0 0 1 minecraft:redstone_block 1\npalette:\n  O observer[facing=east]\nlayer 0:\n  OO\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.ports["start"], [0, 0, 1]);
        assert_eq!(d.meta["port start"], "minecraft:redstone_block 1");
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        assert!(d.to_text().contains("port start: 0 0 1 minecraft:redstone_block 1\n"));
    }

    #[test]
    fn entities_round_trip() {
        let text = "name: e\nport door: 0 0 0\nentity: minecraft:minecart 0.5 1.0625 -3.25\nentity: armor_stand 2 1 0.5 {Rotation:[90f,0f]}\npalette:\n  S stone\nlayer 0:\n  S\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.entities.len(), 2);
        assert_eq!(d.entities[0], EntitySpec { kind: "minecraft:minecart".into(), pos: [0.5, 1.0625, -3.25], nbt: None });
        assert_eq!(d.entities[1].kind, "minecraft:armor_stand");
        assert_eq!(d.entities[1].nbt.as_deref(), Some("{Rotation:[90f,0f]}"));
        assert_eq!(d.entities[0].cell(), [0, 1, -4]);
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        let moved = d.translated([1, -2, 3]);
        assert_eq!(moved.entities[0].pos, [1.5, -0.9375, -0.25]);
        assert!(Design::parse("entity: minecraft:minecart 1 2\n").is_err());
        assert!(Design::parse("entity: minecraft:minecart 1 2 3 Rotation\n").is_err());
    }

    #[test]
    fn block_entities_round_trip() {
        let text = "name: hc\nnbt: 1 0 0 {Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}\nnbt: 0 0 0 {Items:[],TransferCooldown:0}\npalette:\n  H hopper[facing=east]\n  h hopper[facing=west]\nlayer 0:\n  Hh\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.block_entities.len(), 2);
        assert_eq!(d.block_entities[&[1, 0, 0]], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}");
        assert_eq!(d.block_entities[&[0, 0, 0]], "{Items:[],TransferCooldown:0}");
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        // the key changes with the contents; a design without any keeps the old key
        assert!(d.content_key().ends_with(&format!("|nbt{:?}", d.block_entities)));
        let mut empty = d.clone();
        empty.block_entities.clear();
        assert_eq!(empty.content_key(), format!("{:?}|{:?}", empty.blocks, empty.ports));
        let moved = d.translated([2, 1, -1]);
        assert_eq!(moved.block_entities[&[3, 1, -1]], d.block_entities[&[1, 0, 0]]);
        assert!(Design::parse("nbt: 1 0 0\n").is_err());
        assert!(Design::parse("nbt: 1 0 {Items:[]}\n").is_err());
        assert!(Design::parse("nbt: 1 0 0 {Items:[]\n").is_err());
        assert!(Design::parse("nbt: 1 0 0 {}\nnbt: 1 0 0 {}\n").is_err());
    }
}
