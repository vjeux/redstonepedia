//! Disposable-world persistence experiments on the UNMODIFIED official server
//! (designs/chunk-regen/NOTES.md). Protocol per case:
//!
//! 1. **Setup leg (commands, labelled)**: a fresh void world, a stone platform over the
//!    target and its 4 neighbours, marker blocks + a named armor stand in every chunk,
//!    then the case payload in the target chunk. Payload values are survival-legal
//!    (writable-book pages ≤1024 chars, sign text, composters, item entities); the
//!    *commands* are setup only — they create exactly the state a survival player could
//!    create, because RCON accepts only 1460-byte packets and the bot has no book GUI.
//!    A finding that needs a full survival replay is called out, never claimed.
//! 2. `save-all flush`, clean stop, manifest of every stream (`before`).
//! 3. Optional offline control on the (disposable) world folder.
//! 4. **Reload leg**: a new server process on a copy of that world, bot set to survival,
//!    teleport in, read the markers back (`execute if block`, entity count), clean stop,
//!    manifest (`after`), and the server log's chunk failure lines.
//!
//! Outcome classification is by markers + manifests, never by a crash alone.

use crate::{Control, write_manifest};
use anyhow::{Context, Result};
use mctas::vanilla::chunkfile::{self, ChunkEntry, Stream};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// Target chunk and its block origin.
const TC: (i32, i32) = (4, 4);
const OX: i32 = TC.0 * 16;
const OZ: i32 = TC.1 * 16;
/// Marker positions (above any payload layer): diamond + gold in opposite corners.
const MARK_Y: i32 = 20;

#[derive(Clone, Debug)]
pub enum Payload {
    None,
    /// Writable books of one random page in a full layer of barrels (256 containers).
    Books { books: usize, page_len: usize },
    /// Standing signs with random front/back text, `layers` platforms of 256.
    Signs { layers: usize },
    /// Composters (farmer POI, no block entity), `layers` full layers of 256.
    Poi { layers: usize },
    /// Dropped writable books as item entities.
    Items { items: usize, page_len: usize },
}

#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub payload: Payload,
    pub control: Option<(Control, Stream)>,
    pub compression: String,
    pub sync: bool,
    pub normal_world: bool,
    pub out_dir: PathBuf,
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Incompressible text from [a-z ] only (SNBT-safe inside a quoted string).
fn rand_text(seed: u64, len: usize) -> String {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let v = (xorshift(&mut state) % 27) as u8;
        s.push(if v == 26 { ' ' } else { (b'a' + v) as char });
    }
    s
}

fn book_arg(seed: u64, page_len: usize) -> String {
    let page = rand_text(seed, page_len);
    format!("minecraft:writable_book[minecraft:writable_book_content={{pages:[\"{page}\"]}}]")
}

fn opts(version: &str, case: &Case, world: Option<PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        keep_dir: true,
        world,
        view_distance: 3,
        sync_chunk_writes: case.sync,
        region_file_compression: case.compression.clone(),
        level_seed: "424242".into(),
        normal_world: case.normal_world,
        frozen_at_start: false,
        save_off_at_start: false,
        heap_mb: 2048,
        ..Default::default()
    }
}

fn sleep_secs(n: u64) {
    std::thread::sleep(Duration::from_secs(n));
}

fn neighbours() -> [(i32, i32); 4] {
    [(TC.0 - 1, TC.1), (TC.0 + 1, TC.1), (TC.0, TC.1 - 1), (TC.0, TC.1 + 1)]
}

/// Platform, neighbours and chunk loading. Setup leg only.
fn setup_base(srv: &mut VanillaServer) -> Result<()> {
    srv.teleport_bot((OX + 8) as f64 + 0.5, 4.0, (OZ + 8) as f64 + 0.5)?;
    srv.cmd_ok(&format!("forceload add {} {} {} {}", OX - 16, OZ - 16, OX + 31, OZ + 31))?;
    sleep_secs(2);
    srv.cmd_ok(&format!("fill {} -1 {} {} -1 {} minecraft:stone", OX - 16, OZ - 16, OX + 31, OZ + 31))?;
    // Neighbour markers: one diamond block at each neighbour chunk's marker height corner.
    for (ncx, ncz) in neighbours() {
        srv.cmd_ok(&format!("setblock {} {MARK_Y} {} minecraft:diamond_block", ncx * 16, ncz * 16))?;
    }
    Ok(())
}

/// The markers every case plants in the target chunk after its payload.
fn setup_markers(srv: &mut VanillaServer) -> Result<()> {
    srv.cmd_ok(&format!("setblock {OX} {MARK_Y} {OZ} minecraft:diamond_block"))?;
    srv.cmd_ok(&format!("setblock {} {MARK_Y} {} minecraft:gold_block", OX + 15, OZ + 15))?;
    srv.cmd_ok(&format!("setblock {} 0 {} minecraft:composter", OX + 2, OZ + 2))?;
    let stand = format!(
        "summon minecraft:armor_stand {} {} {} {{CustomName:\"chunkreg-marker\",CustomNameVisible:1b,PersistenceRequired:1b}}",
        OX + 3,
        MARK_Y + 1,
        OZ + 3
    );
    srv.cmd_ok(&stand)?;
    Ok(())
}

fn setup_books(srv: &mut VanillaServer, books: usize, page_len: usize) -> Result<()> {
    srv.cmd_ok(&format!("fill {OX} 0 {OZ} {} 0 {} minecraft:barrel", OX + 15, OZ + 15))?;
    for i in 0..books {
        let barrel = i / 27;
        let (bx, bz) = (OX + (barrel % 16) as i32, OZ + (barrel / 16) as i32);
        let cmd = format!("item replace block {bx} 0 {bz} container.{} with {}", i % 27, book_arg(0x1000 + i as u64, page_len));
        srv.cmd_ok(&cmd).with_context(|| format!("book {i}"))?;
    }
    Ok(())
}

fn setup_signs(srv: &mut VanillaServer, layers: usize) -> Result<()> {
    for layer in 0..layers {
        let plat_y = (layer * 4) as i32;
        if layer > 0 {
            srv.cmd_ok(&format!("fill {OX} {plat_y} {OZ} {} {plat_y} {} minecraft:stone", OX + 15, OZ + 15))?;
        }
        let y = plat_y + 1;
        srv.cmd_ok(&format!("fill {OX} {y} {OZ} {} {y} {} minecraft:oak_sign", OX + 15, OZ + 15))?;
        let mut seed = 0x5100 + layer as u64 * 1009;
        for lz in 0..16 {
            for lx in 0..16 {
                seed += 1;
                let lines: Vec<String> = (0..4).map(|k| format!("\"{}\"", rand_text(seed * 8 + k, 90))).collect();
                let back: Vec<String> = (0..4).map(|k| format!("\"{}\"", rand_text(seed * 8 + 4 + k, 90))).collect();
                let cmd = format!(
                    "data merge block {} {y} {} {{front_text:{{messages:[{}]}},back_text:{{messages:[{}]}}}}",
                    OX + lx,
                    OZ + lz,
                    lines.join(","),
                    back.join(",")
                );
                srv.cmd_ok(&cmd).with_context(|| format!("sign layer {layer} at {lx},{lz}"))?;
            }
        }
    }
    Ok(())
}

fn setup_poi(srv: &mut VanillaServer, layers: usize) -> Result<()> {
    let top = layers as i32 - 1;
    srv.cmd_ok(&format!("fill {OX} 0 {OZ} {} {top} {} minecraft:composter", OX + 15, OZ + 15))?;
    Ok(())
}

fn setup_items(srv: &mut VanillaServer, items: usize, page_len: usize) -> Result<()> {
    for i in 0..items {
        let x = OX + (i % 16) as i32;
        let z = OZ + ((i / 16) % 16) as i32;
        let y = 2 + (i / 256) as i32 * 3;
        let page = rand_text(0x1E00 + i as u64, page_len);
        let tag = [
            "{Item:{id:\"minecraft:writable_book\",count:1,components:{\"minecraft:writable_book_content\":{pages:[\"",
            page.as_str(),
            "\"]}}}}",
        ]
        .concat();
        let cmd = format!("summon minecraft:item {x} {y} {z} {tag}");
        srv.cmd_ok(&cmd).with_context(|| format!("item {i}"))?;
    }
    // Let them land before saving (positions settle; age is far from despawn).
    sleep_secs(2);
    Ok(())
}

fn setup_payload(srv: &mut VanillaServer, payload: &Payload) -> Result<()> {
    match payload {
        Payload::None => Ok(()),
        Payload::Books { books, page_len } => setup_books(srv, *books, *page_len),
        Payload::Signs { layers } => setup_signs(srv, *layers),
        Payload::Poi { layers } => setup_poi(srv, *layers),
        Payload::Items { items, page_len } => setup_items(srv, *items, *page_len),
    }
}

fn finish_and_snapshot(mut srv: VanillaServer) -> Result<(PathBuf, PathBuf)> {
    srv.cmd_ok("save-all flush")?;
    let world = srv.world_dir();
    let log = srv.log.clone();
    drop(srv);
    Ok((world, log))
}

fn marker_checks(srv: &mut VanillaServer) -> Result<BTreeMap<String, bool>> {
    let mut m = BTreeMap::new();
    m.insert("target-diamond".into(), srv.test_block([OX, MARK_Y, OZ], "minecraft:diamond_block")?);
    m.insert("target-gold".into(), srv.test_block([OX + 15, MARK_Y, OZ + 15], "minecraft:gold_block")?);
    for (ncx, ncz) in neighbours() {
        m.insert(
            format!("neighbour-{ncx}-{ncz}"),
            srv.test_block([ncx * 16, MARK_Y, ncz * 16], "minecraft:diamond_block")?,
        );
    }
    Ok(m)
}

fn log_hits(log: &PathBuf) -> BTreeMap<String, usize> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let needles = [
        "Failed to load chunk",
        "Failed to save chunk",
        "Failed to store",
        "oversized",
        "External chunk",
        "missing level data",
        "is in the wrong location",
        "Chunk file at",
        "Failed to read chunk",
        "Failed to store chunk",
        "Exception loading chunk",
        "ReportedException",
        "chunk/load",
        "chunk/save",
    ];
    needles.iter().map(|n| (n.to_string(), text.matches(n).count())).filter(|(_, c)| *c > 0).collect()
}

fn entry_for<'a>(entries: &'a [ChunkEntry], stream: Stream, cx: i32, cz: i32) -> Option<&'a ChunkEntry> {
    entries.iter().find(|e| e.stream == stream && e.cx == cx && e.cz == cz)
}

fn describe(e: Option<&ChunkEntry>) -> String {
    match e {
        None => "absent".into(),
        Some(e) => {
            let s = e.summary.as_ref();
            format!(
                "{} stored={} decomp={} status={} be={} ent={} poi={} ticks={}",
                e.presence.name(),
                e.stored_len,
                e.decompressed_len.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                s.and_then(|s| s.status.clone()).unwrap_or_else(|| "-".into()),
                s.map(|s| s.block_entities).unwrap_or(0),
                s.map(|s| s.entities).unwrap_or(0),
                s.map(|s| s.poi_records).unwrap_or(0),
                s.map(|s| s.block_ticks).unwrap_or(0),
            )
        }
    }
}

/// Run one case end to end and print its verdict lines. Manifests land in `case.out_dir`.
pub fn run_case(version: &str, case: &Case) -> Result<String> {
    std::fs::create_dir_all(&case.out_dir)?;
    let mut lines = Vec::new();
    lines.push(format!(
        "EXP {} compression={} sync={} normal_world={} control={}",
        case.name,
        case.compression,
        case.sync,
        case.normal_world,
        case.control.map(|(c, s)| format!("{}/{}", c.name(), s.folder())).unwrap_or_else(|| "-".into())
    ));
    // Setup leg.
    let mut srv = VanillaServer::start(&opts(version, case, None))?;
    setup_base(&mut srv)?;
    setup_payload(&mut srv, &case.payload)?;
    setup_markers(&mut srv)?;
    srv.cmd_ok("forceload remove all")?;
    let (world, setup_log) = finish_and_snapshot(srv)?;
    lines.push(format!("setup world={} log_hits={:?}", world.display(), log_hits(&setup_log)));
    let dim = chunkfile::dimension_dir(&world);
    let before = chunkfile::scan_dimension(&dim, &Stream::all())?;
    write_manifest(&case.out_dir.join("before.tsv"), &before)?;
    for stream in Stream::all() {
        lines.push(format!("before {} target: {}", stream.folder(), describe(entry_for(&before, stream, TC.0, TC.1))));
    }
    if let Some((control, stream)) = case.control {
        let msg = crate::apply_control(&world, control, stream, TC.0, TC.1)?;
        lines.push(msg);
    }
    // Reload leg.
    let mut srv = VanillaServer::start(&opts(version, case, Some(world.clone())))?;
    let bot = srv.bot.as_ref().context("no bot")?.name.clone();
    let _ = srv.cmd(&format!("gamemode survival {bot}"));
    srv.teleport_bot((OX + 8) as f64 + 0.5, 24.0, (OZ + 8) as f64 + 0.5)?;
    sleep_secs(3);
    let markers = marker_checks(&mut srv)?;
    let stands = srv.count_entities(&format!("@e[type=minecraft:armor_stand,x={OX},y={MARK_Y},z={OZ},distance=..24]"))?;
    let items_left = srv.count_entities(&format!("@e[type=minecraft:item,x={OX},y=0,z={OZ},distance=..32]")).unwrap_or(0);
    lines.push(format!("markers={markers:?} armor_stands={stands} ground_items={items_left}"));
    let reload_world = srv.world_dir();
    let (stopped_world, reload_log) = finish_and_snapshot(srv)?;
    debug_assert_eq!(stopped_world, reload_world);
    let dim2 = chunkfile::dimension_dir(&stopped_world);
    let after = chunkfile::scan_dimension(&dim2, &Stream::all())?;
    write_manifest(&case.out_dir.join("after.tsv"), &after)?;
    for stream in Stream::all() {
        lines.push(format!("after {} target: {}", stream.folder(), describe(entry_for(&after, stream, TC.0, TC.1))));
    }
    // Neighbour stability: an untouched neighbour must be bit-identical after reloading.
    let mut neighbours_changed = Vec::new();
    for (ncx, ncz) in neighbours() {
        let b = entry_for(&before, Stream::Region, ncx, ncz);
        let a = entry_for(&after, Stream::Region, ncx, ncz);
        let same = match (b, a) {
            (Some(b), Some(a)) => b.sha256 == a.sha256 && b.presence == a.presence,
            (None, None) => true,
            _ => false,
        };
        if !same {
            neighbours_changed.push(format!("{ncx},{ncz}"));
        }
    }
    let target_ok = markers.values().filter(|k| **k).count();
    let outcome = classify(&before, &after, &markers, &neighbours_changed);
    lines.push(format!(
        "OUTCOME {} markers_ok={}/{} neighbours_changed=[{}] reload_log_hits={:?}",
        outcome,
        target_ok,
        markers.len(),
        neighbours_changed.join(","),
        log_hits(&reload_log)
    ));
    Ok(lines.join("\n"))
}

fn classify(before: &[ChunkEntry], after: &[ChunkEntry], markers: &BTreeMap<String, bool>, neighbours_changed: &[String]) -> &'static str {
    let target_alive = markers.get("target-diamond").copied().unwrap_or(false) && markers.get("target-gold").copied().unwrap_or(false);
    let neighbours_alive = markers.iter().filter(|(k, _)| k.starts_with("neighbour")).all(|(_, v)| *v);
    let b_region = entry_for(before, Stream::Region, TC.0, TC.1);
    let a_region = entry_for(after, Stream::Region, TC.0, TC.1);
    let payload_lost = match (b_region, a_region) {
        (Some(b), Some(a)) => {
            let bb = b.summary.as_ref().map(|s| s.block_entities).unwrap_or(0);
            let aa = a.summary.as_ref().map(|s| s.block_entities).unwrap_or(0);
            aa < bb
        }
        (Some(_), None) => true,
        _ => false,
    };
    match (target_alive, neighbours_alive, payload_lost, neighbours_changed.is_empty()) {
        (true, true, false, true) => "normal",
        (true, true, false, false) => "normal-neighbours-resaved",
        (true, _, true, _) => "partial-payload-loss",
        (false, true, _, true) => "regen",
        (false, true, _, false) => "regen-neighbour-hash-differs",
        (false, false, _, _) => "region-wide",
        (true, false, _, _) => "neighbour-loss",
    }
}
