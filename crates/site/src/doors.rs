//! The 3x3 piston doors: every working design of designs/piston-door-3x3/archive with the facts
//! the boards and the catalog use, read from the caches next to the archive (verdicts, shapes,
//! properties, dynamic checks; no JVM), as [`Build`]s.

use anyhow::Result;
use mctas::design::Design;
use piston_door::leaderboard::{self, Entry, tier_rank};
use piston_door::showcase::{self, BLOCK_BOARDS};
use site_model::{Build, Door};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const ARCHIVE: &str = "designs/piston-door-3x3/archive";
/// The 2x2 doors (clk-item, the judge's `door-size: 2`; the coordinator 2026-10-21: their own
/// boards, not mixed with the 3x3 records), archived with the same caches as the 3x3's.
pub const ARCHIVE_2X2: &str = "designs/piston-door-2x2/archive";

/// The boards a 2x2 door can lead (designs/piston-door-2x2/WR.md: smallest flush, fastest,
/// slime-free; the 3x3's flat, wall and ground boards and the community's seamless tiers are not
/// the 2x2's), as the leads read them.
const BOARDS_2X2: [&str; 3] = ["solid", "slime-free", "fastest open + close (any category)"];

/// The archive a door build was exported from (by its opening's side).
pub fn archive_of(b: &Build) -> &'static str {
    if site_model::door_side(b) == 2 { ARCHIVE_2X2 } else { ARCHIVE }
}
/// Which archived doors are the project's own work (vjeux, 01:15Z: "We should only have things we
/// generated on the website to avoid stealing people's work"): `name<TAB>origin` rows, origin
/// `ours` or `community-derived` (written by track c70b). Only `ours` is published.
pub const PROVENANCE: &str = "provenance.tsv";

/// Name prefixes of designs derived from community doors (the seed rule, applied whether or not
/// provenance.tsv exists; a published door matching it fails the build).
// Not "pockets-63b": "63b" is only the 7x9 box volume. Most pockets-63b-* descend from t7b / e89b
// (ours); the c79-seeded ones are flagged by provenance.tsv's run-seed lineage (c70b, a11b72084).
pub const COMMUNITY_PREFIXES: [&str; 6] = ["community", "c79", "closed-b-fill", "satwire-c79", "satwire-63b", "c88"];

/// Why a design is community-derived by the seed rule: its name, or a `note: door fill of`
/// line naming a community design (a path under tracks/c77/community, or a file with one of the
/// prefixes). `None`: not by this rule.
pub fn community_seed(stem: &str, text: &str) -> Option<String> {
    let has_prefix = |name: &str| COMMUNITY_PREFIXES.iter().find(|p| name.starts_with(*p)).copied();
    if let Some(p) = has_prefix(stem) {
        return Some(format!("its name starts with {p}"));
    }
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("note: door fill of ") else { continue };
        let path = rest.split(": ").next().unwrap_or(rest).trim();
        if path.contains("tracks/c77/community") {
            return Some(format!("it is a door fill of {path}"));
        }
        let file = path.rsplit('/').next().unwrap_or(path);
        if let Some(p) = has_prefix(file) {
            return Some(format!("it is a door fill of {path} ({p}...)"));
        }
    }
    None
}

/// The provenance table: stem -> origin (`ours`, `community-derived`).
pub fn read_provenance(dir: &Path) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(dir.join(PROVENANCE)).ok()?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines.next()?.split('\t').map(|h| h.trim().to_lowercase()).collect();
    let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
    let (ni, oi) = (col(&["name", "design", "file", "stem"]).unwrap_or(0), col(&["origin", "provenance"]).unwrap_or(1));
    let mut m = HashMap::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        let (Some(n), Some(o)) = (f.get(ni), f.get(oi)) else { continue };
        let n = n.trim();
        let stem = n.rsplit('/').next().unwrap_or(n).trim_end_matches(".txt").to_string();
        m.insert(stem, o.trim().to_lowercase());
    }
    Some(m)
}

/// The doors left out as not ours: stem -> why. With provenance.tsv: every door not marked `ours`,
/// and every door the seed rule flags even when marked `ours` (never publish what either signal
/// calls community work; such conflicts are reported); without it: the seed rule.
pub fn not_ours(dir: &Path, stems_and_texts: &[(String, String)]) -> anyhow::Result<BTreeMap<String, String>> {
    let prov = read_provenance(dir);
    let mut out = BTreeMap::new();
    for (stem, text) in stems_and_texts {
        let seed = community_seed(stem, text);
        match &prov {
            Some(p) => match p.get(stem).map(String::as_str) {
                Some("ours") => {
                    if let Some(why) = seed {
                        out.insert(stem.clone(), format!("CONFLICT: {PROVENANCE} says ours but {why} (the seed rule wins)"));
                    }
                }
                Some(o) => {
                    out.insert(stem.clone(), format!("{PROVENANCE}: {o}"));
                }
                None => {
                    out.insert(stem.clone(), format!("not listed in {PROVENANCE}"));
                }
            },
            None => {
                if let Some(why) = seed {
                    out.insert(stem.clone(), why);
                }
            }
        }
    }
    Ok(out)
}

/// The site's knobs for the doors.
pub struct DoorOptions {
    /// Asset names of the GitHub release with the videos (`<stem>.mp4`); empty: no video links.
    pub release_assets: HashSet<String>,
    pub release_tag: String,
}

/// The archive's candidates as `door catalog` takes them: every `.txt` with door and lever
/// ports, files marked invalid left out.
fn candidates(dir: &Path) -> Result<Vec<(String, Design)>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if stem.contains("invalid") {
            continue;
        }
        let Ok(d) = Design::load(&p) else { continue };
        if d.ports.contains_key("door") && d.ports.contains_key("lever") {
            out.push((p.display().to_string(), d));
        }
    }
    Ok(out)
}

/// Block kinds of a design (ids without `minecraft:`; a piston head is part of its piston, a wall
/// torch is a torch, every copper bulb a copper bulb), plus its entities' kinds: the clock
/// study's normalization, so one block-kind filter works across doors and clocks.
pub fn kinds_of(d: &Design) -> Vec<String> {
    let norm = |s: &str| -> Option<String> {
        let id = s.split('[').next().unwrap_or(s);
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        if id == "piston_head" || id == "moving_piston" || id == "air" {
            return None;
        }
        Some(if id == "redstone_wall_torch" {
            "redstone_torch".to_string()
        } else if id.contains("copper_bulb") {
            "copper_bulb".to_string()
        } else {
            id.to_string()
        })
    };
    let set: BTreeSet<String> = d.blocks.values().filter_map(|s| norm(s)).chain(d.entities.iter().filter_map(|e| norm(&e.kind))).collect();
    set.into_iter().collect()
}

/// `YYYY-MM-DD HH:MMZ` from a design's `# archived ...` header line.
pub fn archived_at(text: &str) -> String {
    text.lines()
        .take(4)
        .find_map(|l| l.strip_prefix("# archived ").map(|r| r.split(" from ").next().unwrap_or("").trim().to_string()))
        .unwrap_or_default()
}

/// Every working archived door that is the project's own work ([`not_ours`] leaves the others
/// out), sorted by size (volume, blocks): the 3x3 doors, then the 2x2 doors (their own archive,
/// boards and records). Returns the builds, the stems left out because main's judge has no cached
/// verdict for them yet, and the stems left out as not ours (with why).
pub fn export(repo: &Path, o: &DoorOptions) -> Result<(Vec<Build>, Vec<String>, BTreeMap<String, String>)> {
    let (mut out, mut uncached, mut excluded) = export_archive(repo, ARCHIVE, 3, o)?;
    if repo.join(ARCHIVE_2X2).is_dir() {
        let (small, u, x) = export_archive(repo, ARCHIVE_2X2, 2, o)?;
        // (one page per stem: a 2x2 door named like a 3x3 one would overwrite its page)
        for b in &small {
            if out.iter().any(|o| o.name == b.name) {
                anyhow::bail!("the 2x2 door {} has the name of a 3x3 door: rename one of them (their pages share doors/<name>.html)", b.name);
            }
        }
        out.extend(small);
        uncached.extend(u.into_iter().map(|s| format!("{s} (2x2)")));
        excluded.extend(x.into_iter().map(|(s, why)| (format!("{s} (2x2)"), why)));
    }
    Ok((out, uncached, excluded))
}

/// One door archive's builds ([`export`]); `side` is its doors' opening (3 or 2).
fn export_archive(repo: &Path, archive: &str, side: u32, o: &DoorOptions) -> Result<(Vec<Build>, Vec<String>, BTreeMap<String, String>)> {
    let dir = repo.join(archive);
    let cands = candidates(&dir)?;
    let texts: Vec<(String, String)> = cands
        .iter()
        .map(|(src, _)| {
            let p = Path::new(src);
            (p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), std::fs::read_to_string(p).unwrap_or_default())
        })
        .collect();
    let excluded = not_ours(&dir, &texts)?;
    let cands: Vec<(String, Design)> = cands.into_iter().filter(|(src, _)| !Path::new(src).file_stem().is_some_and(|s| excluded.contains_key(s.to_string_lossy().as_ref()))).collect();
    let (entries, uncached) = leaderboard::cached_entries(cands, &dir);
    let working: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok()).collect();
    let all: Vec<showcase::Archived> = working.iter().map(|e| showcase::archived_of(e)).collect();
    let records = showcase::progression(&dir);
    let record_how: HashMap<String, String> = records.iter().map(|(_, s, h)| (s.clone(), h.clone())).collect();
    let record_blocks: HashMap<String, usize> = records.iter().map(|(b, s, _)| (s.clone(), *b)).collect();
    let captions = showcase::captions(&dir);
    let builds_v = piston_door::catalog::buildability(&dir);
    let props = piston_door::properties::load_cache(&dir);
    let dynp = piston_door::properties::load_dyn(&dir);
    // board leaders, labeled as the catalog labels them (a 2x2 door: only the 2x2's boards)
    let mut leads: HashMap<String, Vec<String>> = HashMap::new();
    for b in showcase::boards(&all, &record_how) {
        let label = match b.key {
            Some("sculk-unrotated") => "sculk, not rotation-safe".to_string(),
            Some(k) => k.to_string(),
            None => b.title.to_lowercase(),
        };
        if side == 2 && !BOARDS_2X2.contains(&label.as_str()) {
            continue;
        }
        leads.entry(b.best.stem.clone()).or_default().push(label);
    }
    let mut out = Vec::new();
    for (e, a) in working.iter().zip(&all) {
        let h = leaderboard::design_hash(&e.design);
        // (a 2x2 door: only its size and the judge's facts; the 3x3 catalogue's properties, the
        // seamless tiers, the frame, the hallway, are not measured for a 2x2 opening yet)
        let three = side == 3;
        let get = |k: &str| -> String {
            // (clk-item 2026-10-21: on a 2x2 opening the volume and the hallway check mean
            // something; the seamless tiers, the frame and the regulation are the 3x3's)
            if !three && !matches!(k, "volume" | "volume_dims" | "hallway") {
                return String::new();
            }
            dynp.get(&h).and_then(|p| p.get(k)).or_else(|| props.get(&h).and_then(|p| p.get(k))).cloned().unwrap_or_default()
        };
        let yes = |k: &str| get(k).starts_with("yes");
        let f = &a.facts;
        let tier = get("seamless").split_whitespace().next().unwrap_or("").to_string();
        let rank = tier_rank(&tier);
        let hallway = yes("hallway");
        let eligible = rank > 0 && yes("lever_surface") && yes("lever_support") && yes("repeater_input") && yes("uses") && yes("steps") && hallway;
        let frame = get("frame");
        let mut boards: Vec<String> = if three { BLOCK_BOARDS.iter().filter(|b| (b.3)(f)).map(|b| b.1.to_string()).collect() } else { Vec::new() };
        if !f.rotation_safe {
            boards.push("Not rotation-safe".into());
        }
        let my_leads = leads.get(&a.stem).cloned().unwrap_or_default();
        let video = format!("{}.mp4", a.stem);
        let video = o.release_assets.contains(&video).then(|| showcase::asset_url(&o.release_tag, &video));
        // the board leaders' GIFs (door gallery) as interim pictures until the 3D viewer
        let gif = format!("{}.gif", a.stem);
        let picture = o.release_assets.contains(&gif).then(|| showcase::asset_url(&o.release_tag, &gif));
        let buildable = builds_v.get(&a.stem).map(|(v, _)| v.clone());
        let text = std::fs::read_to_string(&a.path).unwrap_or_default();
        let shape = f.shape;
        let mut flags: Vec<&str> = Vec::new();
        let mut flag = |on: bool, k: &'static str| {
            if on {
                flags.push(k);
            }
        };
        flag(f.rotation_safe, "rotation-safe");
        flag(f.slime_free, "slime-free");
        flag(f.flat, "flat");
        flag(shape.is_some_and(|s| s.front <= 1 && s.behind <= 1), "wall");
        flag(f.ground_safe && shape.is_some_and(|s| s.below == 0), "ground");
        flag(rank > 0, "seamless");
        flag(rank >= 3, "seamless-full");
        flag(hallway, "hallway");
        flag(frame == "12/12", "frame-full");
        flag(eligible, "eligible");
        flag(record_blocks.contains_key(&a.stem), "record");
        flag(!my_leads.is_empty(), "leader");
        flag(video.is_some(), "video");
        flag(buildable.as_deref().is_some_and(|b| b.starts_with("yes")), "buildable");
        flag(!e.design.entities.is_empty(), "entities");
        let caption = captions
            .get(&a.stem)
            .map(|c| showcase::sentence(c))
            .or_else(|| showcase::track_of(&a.path).map(|t| format!("From track {t}.")))
            .unwrap_or_default();
        // (a 2x2 door: no community-regulation table; its size is the build's volume)
        let mut all_props: Vec<(String, String)> = props.get(&h).filter(|_| three).map(|p| p.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
        if let Some(dp) = dynp.get(&h).filter(|_| three) {
            for (k, v) in dp {
                if !all_props.iter().any(|(kk, _)| kk == k) {
                    all_props.push((k.clone(), v.clone()));
                }
            }
        }
        all_props.sort();
        let rotation = match &e.rotation {
            Some(Ok(())) => "safe".to_string(),
            Some(Err((deg, _))) => format!("breaks at {deg}°"),
            None => String::new(),
        };
        out.push(Build {
            id: format!("doors/{}", a.stem),
            kind: "door".into(),
            group: e.category.to_string(),
            name: a.stem.clone(),
            page: format!("doors/{}.html", a.stem),
            design: format!("designs/doors/{}.txt", a.stem),
            blocks: e.blocks as u32,
            volume: e.volume.map(|v| v as u32),
            dims: e.volume_dims.clone().filter(|d| !d.is_empty()),
            kinds: kinds_of(&e.design),
            caption,
            flags: flags.into_iter().map(String::from).collect(),
            door: Some(Door {
                open: e.open_ticks,
                close: e.close_ticks,
                rotation,
                seamless: tier,
                frame,
                boards,
                leads: my_leads,
                record: record_blocks.get(&a.stem).map(|b| *b as u32),
                video,
                buildable,
                track: showcase::track_of(&a.path),
                archived: archived_at(&text),
                entities: e.design.entities.len() as u32,
                props: all_props,
                size: if three { 0 } else { side },
                ..Default::default()
            }),
            clock: None,
            circuit: None,
            trace: None,
            still: None,
            picture,
            archived: site_model::normalize_time(&archived_at(&text)),
            curated: Default::default(),
        });
    }
    out.sort_by(|a, b| (a.volume.unwrap_or(u32::MAX), a.blocks, &a.name).cmp(&(b.volume.unwrap_or(u32::MAX), b.blocks, &b.name)));
    fold_fills(&mut out, &dir);
    // the guard: nothing community-derived is ever published (whatever the provenance file says)
    for b in &out {
        let text = std::fs::read_to_string(dir.join(format!("{}.txt", b.name))).unwrap_or_default();
        if let Some(why) = community_seed(&b.name, &text) {
            anyhow::bail!("refusing to publish {}: {why} (community work is never published)", b.name);
        }
    }
    let uncached = uncached.iter().map(|s| Path::new(s).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()).collect();
    Ok((out, uncached, excluded))
}

/// The design a fill variant was made from: `note: door fill of <path>: <changes>`.
pub fn fill_parent(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix("note: door fill of ").map(|r| r.split(": ").next().unwrap_or(r).trim().to_string()))
}

/// Where an archived design came from: the path in its `# archived ... from <ref>:<path> (repo
/// at ...)` header line, the ref dropped.
pub fn archived_from(text: &str) -> Option<String> {
    let l = text.lines().take(4).find(|l| l.starts_with("# archived "))?;
    let from = l.split(" from ").nth(1)?.split(" (repo at").next()?.trim();
    Some(from.rsplit_once(':').map(|(_, p)| p).unwrap_or(from).to_string())
}

/// Fold the fill variants of each mechanism (vjeux: "record all variants"; the archive holds
/// hundreds): the doors filled from one design, with that design itself when it is archived,
/// form a group; the best of the group (the list is sorted by size, then blocks) stands for it
/// and the others become its variants (hidden by default, like the clock variants).
fn fold_fills(out: &mut [Build], dir: &Path) {
    let text = |b: &Build| std::fs::read_to_string(dir.join(format!("{}.txt", b.name))).unwrap_or_default();
    let mut parent_of: Vec<Option<String>> = Vec::with_capacity(out.len());
    let mut source_of: Vec<Option<String>> = Vec::with_capacity(out.len());
    for b in out.iter() {
        let t = text(b);
        parent_of.push(fill_parent(&t));
        source_of.push(archived_from(&t));
    }
    let parents: BTreeSet<String> = parent_of.iter().flatten().cloned().collect();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..out.len() {
        let key = parent_of[i].clone().or_else(|| source_of[i].clone().filter(|s| parents.contains(s)));
        if let Some(k) = key {
            groups.entry(k).or_default().push(i);
        }
    }
    for (parent, members) in groups {
        for &i in &members {
            if let Some(d) = out[i].door.as_mut() {
                d.fill_of = parent_of[i].clone().unwrap_or_default();
            }
        }
        if members.len() < 2 {
            continue;
        }
        // the list is sorted by (volume, blocks, name): the first member is the best
        let host = members[0];
        let host_id = out[host].id.clone();
        let ids: Vec<String> = members[1..].iter().map(|&i| out[i].id.clone()).collect();
        for &i in &members[1..] {
            if let Some(d) = out[i].door.as_mut() {
                d.host = Some(host_id.clone());
            }
            out[i].flags.push("variant".into());
        }
        if let Some(d) = out[host].door.as_mut() {
            d.variants = ids;
        }
        let _ = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_normalized() {
        let d = Design::parse("name: t\npalette:\n  P minecraft:sticky_piston[facing=up,extended=true]\n  H minecraft:piston_head[facing=up,type=sticky]\n  T minecraft:redstone_wall_torch[facing=east]\n  C minecraft:waxed_copper_bulb[lit=false]\nlayer 0:\n  PTC\nlayer 1:\n  H\n").unwrap();
        assert_eq!(kinds_of(&d), vec!["copper_bulb", "redstone_torch", "sticky_piston"]);
    }

    #[test]
    fn archived_header() {
        assert_eq!(archived_at("# verified WORKS\n# archived 2026-10-19 10:35Z from /tmp/x.txt (repo at ccbb856)\nname: x\n"), "2026-10-19 10:35Z");
        assert_eq!(archived_at("name: x\n"), "");
    }

    #[test]
    fn community_work_is_recognized() {
        for name in ["community-7x8-stone", "c79-49-tl1-49b", "closed-b-fill-48b-1-48b", "satwire-c79-x", "satwire-63b-49-49b", "c88-x"] {
            assert!(community_seed(name, "").is_some(), "{name}");
        }
        assert!(community_seed("e78st-56b-37-37b", "name: x\n").is_none());
        let fill = "note: door fill of designs/piston-door-3x3/tracks/c77/community/closed-b.txt: (1,2) stone\n";
        assert!(community_seed("x-fill-1", fill).unwrap().contains("tracks/c77/community"));
        let fill2 = "note: door fill of designs/piston-door-3x3/tracks/pockets/c79-open-b.txt: (1,2) stone\n";
        assert!(community_seed("y-fill-2", fill2).is_some());
        let ours = "note: door fill of designs/piston-door-3x3/tracks/shrink/wip/t7b-pw42.txt: (1,2) stone\n";
        assert!(community_seed("t7b-fill", ours).is_none());
        // with a provenance table: only `ours` is published, and a community name marked ours fails
        let dir = std::env::temp_dir().join(format!("site-prov-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PROVENANCE), "name\torigin\nt7b-a.txt\tours\nmystery\tcommunity-derived\n").unwrap();
        let ex = not_ours(&dir, &[("t7b-a".into(), String::new()), ("mystery".into(), String::new()), ("unlisted".into(), String::new())]).unwrap();
        assert_eq!(ex.keys().cloned().collect::<Vec<_>>(), vec!["mystery", "unlisted"]);
        std::fs::write(dir.join(PROVENANCE), "name\torigin\nc79-x\tours\n").unwrap();
        let ex = not_ours(&dir, &[("c79-x".into(), String::new())]).unwrap();
        assert!(ex["c79-x"].starts_with("CONFLICT"), "{ex:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fill_headers() {
        let t = "# verified WORKS by door leaderboard: 37 blocks, category sculk\n# archived 2026-10-19 20:23Z from origin/explore/c77:designs/piston-door-3x3/tracks/c77/runs/x/e78st-fill-37b-1.txt (repo at 7c5e37c16)\nname: e78st-fill\nnote: door fill of designs/piston-door-3x3/tracks/c77/e78st-56b-38-1s.txt: (0,-3) calibrated_sculk_sensor[facing=west] (-2,-1) air\n";
        assert_eq!(fill_parent(t).as_deref(), Some("designs/piston-door-3x3/tracks/c77/e78st-56b-38-1s.txt"));
        assert_eq!(archived_from(t).as_deref(), Some("designs/piston-door-3x3/tracks/c77/runs/x/e78st-fill-37b-1.txt"));
        assert_eq!(archived_from("# archived 2026-10-19 10:35Z from /tmp/x.txt (repo at ccbb856)\n").as_deref(), Some("/tmp/x.txt"));
        assert_eq!(fill_parent("name: x\n"), None);
    }
}
