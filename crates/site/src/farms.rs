//! The Farms section (vjeux 2026-11-08 12:14: "a new farms top level category, and you can drill
//! down with iron farm and then full farm / individual components"; 12:15: every promising
//! variation of a stage on the site, the stage's pick first, its numbers the numbers; 13:14: a
//! farm type is what it produces, "Melons and sugarcane are two different farms"; 13:16: melons and
//! pumpkins together, "They can be together").
//!
//! - `farms/index.html`: the farm types (the iron farm, then one per crop), the Farms tab's page.
//! - `farms/iron/index.html`: the iron farm's hub: its full farms by stage (Day 1, Middle, Late),
//!   every variation of each stage with its pick first, then its components.
//! - `farms/<crop>/index.html` (`melon-pumpkin`, `sugar-cane`, `bamboo`...): a crop's farms, the
//!   kind's pick first ([`site_model::farm_folder_of`]).
//! - A farm's page, `farms/<farm>/<stem>.html`: an archived design's or a variation's, the same
//!   page as any design's.
//! - Variations: `designs/farms/<farm>/<stage>/` holds every promising design of a stage
//!   (`<stem>.txt` with the judge's line on top), `verdicts.tsv` (the judge's rows, the archive's
//!   format) and `variations.tsv` (`file`, `pick`, `name`), all written by `mctas site variation`.
//! - Components: `designs/farms/<farm>/parts/<part>/` holds a part's members (`<stem>.txt`, the
//!   part alone) and `members.tsv` (`file`, `name`, `stage`, `metrics`, `used_by`, best first),
//!   written by the farm optimizer; `farms/<farm>/parts/<part>/index.html` lists them, and each full
//!   farm's page links to the members it is built from.

use crate::pages::{self, Scripts, SITE_NAME};
use anyhow::{Context, Result, bail};
use site_model::html::{build_card, esc};
use site_model::{Build, Site};
use std::path::{Path, PathBuf};

/// A farm's stages: (folder, title, its kind's tier word, one line for the player).
pub const STAGES: [(&str, &str, &str, &str); 3] = [
    ("day-1", "Day 1", "day-1", "Built from what you have on your first day."),
    ("middle", "Middle", "middle", "An evening's build, for more iron."),
    ("late", "Late", "fastest", "The most iron: modules you copy side by side."),
];

/// An iron farm's components: (folder, name, what it is, for the player).
/// An iron farm's parts, in the order its pages list them: (folder, the other name the optimizer
/// may write it under, name, what it is, for the player). The parent 2026-11-08 20:59: every farm
/// page lists its parts (cage, calm, pad, transport, kill, collection, storage, access), each with
/// its own numbers and a link to that part on its component page.
pub const PARTS: [(&str, &str, &str, &str); 9] = [
    ("module", "", "Farm module", "One whole farm, the unit bigger farms are copies of: its iron an hour, its blocks and its build time."),
    ("cage", "", "Villager cage", "Where the villagers sleep and watch the zombie: how many villagers, and how many of them watch."),
    ("sleep-trigger", "calm", "Sleep trigger", "What hides the zombie at night, so the villagers can sleep."),
    ("spawn-area", "pad", "Golem spawn area", "The floor where the golems appear: its size, and how often a golem finds a spot on it."),
    ("transport", "", "Golem drop", "How the golems get from where they appear to where they die."),
    ("kill", "", "Kill chamber", "Where the golems die: the lava, the drop, and whether the villagers see it."),
    ("collection", "", "Collection", "How the iron reaches your chest, and how long the chest holds it."),
    ("storage", "", "Storage", "The chests that hold the iron until you come back for it."),
    ("access", "", "Access", "The way to the chest and back, and the way in to build it."),
];

/// A part's folder from its name or its other name (`calm` -> `sleep-trigger`).
pub fn part_folder(key: &str) -> Option<&'static str> {
    PARTS.iter().find(|p| p.0 == key || (!p.1.is_empty() && p.1 == key)).map(|p| p.0)
}

/// The villager carousel's parts (vjeux 2026-11-10: "Can you publish the components in the site so I
/// can see how it looks like"; the parent: under a "Villager carousel (in progress)" heading on the
/// iron farm's page): (folder under designs/farms/iron/parts/, name, what it is, for the player).
/// Each gets its component page as soon as its catalog (`members.tsv`) has a member.
pub const CAROUSEL: [(&str, &str, &str, &str); 1] = [
    // (vjeux 2026-11-10 rejected the 7-lane layout for one lane of 21 carts released 3 by 3: only the
    // station carries over; the counter, the hold, the yard, the scheduler and the assembly were
    // the 7-lane farm's and stay off)
    ("carousel-station", "", "Station, portal pad and Nether kill", "Where each batch of villagers panics and summons its golem: the golem lands on a pad touching a portal, leaves for the Nether at once, dies there in lava, and its iron is sorted and stored."),
];

/// A part's name and what it is (`PARTS`, `CAROUSEL`).
fn part_words(folder: &str) -> (&'static str, &'static str) {
    PARTS.iter().find(|p| p.0 == folder).map(|p| (p.2, p.3)).or_else(|| CAROUSEL.iter().find(|p| p.0 == folder).map(|p| (p.2, p.3))).unwrap_or(("Part", ""))
}

/// Whether a part is one of the villager carousel's.
pub fn is_carousel(folder: &str) -> bool {
    CAROUSEL.iter().any(|p| p.0 == folder)
}

/// Every part folder with a component page: the iron farm's parts, then the carousel's.
pub fn all_part_folders() -> Vec<&'static str> {
    PARTS.iter().map(|p| p.0).chain(CAROUSEL.iter().map(|p| p.0)).collect()
}

/// A part's members: its folder's catalog, else (a carousel part) its folder's other name's
/// (ironfarm's first push named them `carousel-release-hold`, `carousel-sleep-yard`).
fn part_members(repo: &Path, folder: &str) -> Vec<Member> {
    let mut v = read_members(repo, "iron", folder);
    if v.is_empty()
        && let Some(alias) = CAROUSEL.iter().find(|p| p.0 == folder && !p.1.is_empty()).map(|p| p.1)
    {
        v = read_members(repo, "iron", alias);
        for m in &mut v {
            m.part = folder.to_string();
        }
    }
    v
}

/// A carousel member's name as a player reads it (the owners' catalogs name versions `v8` and
/// count `gt`): `version 8`, game ticks.
fn carousel_name(name: &str) -> String {
    name.split(' ')
        .map(|w| {
            let core = w.trim_end_matches([':', ',', ';', ')']);
            let tail = &w[core.len()..];
            let lead = core.strip_prefix('(').map(|_| "(").unwrap_or("");
            let core = core.trim_start_matches('(');
            match core.strip_prefix('v').filter(|r| r.starts_with(|c: char| c.is_ascii_digit()) && r.chars().all(|c| c.is_ascii_alphanumeric())) {
                Some(r) => format!("{lead}version {r}{tail}"),
                None => match core {
                    "gt" => format!("{lead}game ticks{tail}"),
                    "judged" => format!("{lead}checked{tail}"),
                    _ => w.to_string(),
                },
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .replace("-gt ", "-game-tick ")
        // (the lanes that built a part are no words for a player: "for ironfarm's paired lanes")
        .replace("ironfarm's ", "the ")
        .replace("farmopt's ", "the ")
        .replace("ironspec's ", "the ")
}

/// A carousel member's number as a player reads it (the owners' catalogs write the bench's terms:
/// `T_c 25 gt`, `fallback m 6..8`): its label and value, game ticks also in seconds.
fn carousel_metric(k: &str, v: &str) -> (String, String) {
    let ticks = |v: &str| -> String {
        let t = v.trim();
        match t.strip_suffix(" gt").and_then(|n| n.trim().parse::<f64>().ok()) {
            Some(n) => format!("{n} game ticks ({} s)", (n / 20.0 * 100.0).round() / 100.0),
            None => t.replace(" gt", " game ticks"),
        }
    };
    // (a test world is a seed to the owners; to a player, a world; a game tick is no `gt`)
    let worlds = |v: &str| v.replace(" seeds x ", " worlds × ").replace(" seeds", " worlds");
    let v = &worlds(v);
    let words = |v: &str| v.replace(" gt", " game ticks");
    match k {
        "clean seeds" => ("worlds where it ran clean".into(), words(v)),
        "arrival margin min" => ("a batch arrives before its slot, at least".into(), ticks(v)),
        "arrival margin mean" => ("a batch arrives before its slot, on average".into(), ticks(v)),
        "golem spawn spots" if v.trim() == "0" => ("stray golem spawn spots".into(), "none".into()),
        "visit median" => ("a bed visit takes (median)".into(), ticks(v).replace("..", " to ")),
        "junctions clear max" => ("the tracks are clear again within".into(), ticks(v)),
        "T_c" => ("next batch called".into(), format!("within {} of a golem", ticks(v))),
        "transition gap" => ("between two batches".into(), format!("at most {} with none called, never two at once", ticks(v))),
        "fallback m" => ("a missed slot".into(), match v.split_once("..").and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))) {
            Some((a, b)) => format!("it steps by itself {} to {} game ticks after its last step", 100 + a, 100 + b),
            None => v.to_string(),
        }),
        "blocks" => ("blocks placed".into(), v.to_string()),
        "box" => ("size".into(), v.replace('x', "×")),
        k if k.starts_with("box ") => ("size".into(), format!("{} {}", &k[4..], v).replace('x', "×").replace(" Nether ", ", Nether ")),
        "build minutes" if v.split_whitespace().count() > 1 => {
            // (`68.8 middle 19.2 late`: the minutes at each stage's prices)
            let w: Vec<&str> = v.split_whitespace().collect();
            let pairs: Vec<String> = w.chunks(2).map(|c| if c.len() == 2 { format!("{} at {} prices", c[0], c[1].replace("day-1", "day 1")) } else { c[0].to_string() }).collect();
            ("build minutes".into(), pairs.join(", "))
        }
        "success per slot" => ("a golem each slot".into(), match v.split_once(' ') {
            Some((pct, rest)) => format!("{pct} of the time {}", rest.replace(" seeds x ", " worlds × ").replace(" seeds", " worlds")),
            None => format!("{v} of the time"),
        }),
        "pulse +1 gt" => ("signal to the counter".into(), format!("1 game tick after the golem appears, lasting {}", ticks(v))),
        "golem stay" => ("a golem stays".into(), format!("{} before it leaves through the portal", ticks(v))),
        "Nether kill" => ("in the Nether".into(), v.replace(" gt", " game ticks after it arrives,").replace(" a golem", " per golem")),
        "storage" => ("storage".into(), v.replace(" h at ", " hours at ").replace(" iron/h", " iron an hour")),
        _ => (k.to_string(), if v.trim().ends_with(" gt") { ticks(v) } else { v.replace(" gt", " game ticks") }),
    }
}

/// The title of a farm folder, by what it produces (`iron` -> `Iron farm`, `sugar-cane` ->
/// `Sugar cane farm`, `melon-pumpkin` -> `Melon and pumpkin farm`: the names players search for,
/// docs/search-demand/terms.tsv).
pub fn farm_title(folder: &str) -> String {
    match folder {
        "iron" => "Iron farm".to_string(),
        "melon-pumpkin" => "Melon and pumpkin farm".to_string(),
        "" => "Farms".to_string(),
        f => {
            let words = f.replace('-', " ");
            let mut ch = words.chars();
            ch.next().map(|c| c.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default() + " farm"
        }
    }
}

/// What a farm type gives, one line for its tile and its page.
pub fn farm_blurb(folder: &str) -> String {
    match folder {
        "iron" => crate::explain::study_blurb("mobfarm").to_string(),
        "melon-pumpkin" => "Melons and pumpkins without replanting or collecting: one farm grows either, to trade or eat.".to_string(),
        "sugar-cane" => "Sugar cane without replanting or collecting: paper, books and rockets.".to_string(),
        "bamboo" => "Bamboo without replanting or collecting: scaffolding, and fuel for furnaces.".to_string(),
        "kelp" => "Kelp without replanting or collecting: dry it into fuel blocks.".to_string(),
        "cactus" => "Cactus without replanting or collecting: green dye.".to_string(),
        _ => "Crops without replanting or collecting.".to_string(),
    }
}

/// The farm types' order on the Farms page: iron first, then the crops as players search for them
/// (docs/search-demand/terms.tsv), any other crop after, by name.
fn farm_order(folder: &str) -> (usize, String) {
    let known = ["iron", "melon-pumpkin", "sugar-cane", "bamboo", "cactus", "kelp"];
    (known.iter().position(|k| *k == folder).unwrap_or(known.len()), folder.to_string())
}

/// Every farm folder the site's builds make, in [`farm_order`].
pub fn farm_folders(site: &Site) -> Vec<String> {
    let mut v: Vec<String> = site.builds.iter().filter_map(site_model::build_farm_folder).collect();
    v.sort_by_key(|f| farm_order(f));
    v.dedup();
    v
}

/// A variation of a farm stage (`designs/farms/<farm>/<stage>/`), not an archived design.
pub fn is_variation(b: &Build) -> bool {
    b.flags.iter().any(|f| f == "variation")
}

/// The stage a farm build belongs to (an index into [`STAGES`]), from its kind's tier word.
pub fn stage_of(b: &Build) -> Option<usize> {
    let tier = b.circuit.as_ref()?.kind.split_whitespace().last()?.to_string();
    STAGES.iter().position(|s| s.2 == tier)
}

/// A variation's kind with its stage's tier word (`mob-farm iron_ingot` in `middle` ->
/// `mob-farm iron_ingot middle`); a kind that names a tier already keeps it.
pub fn stage_kind(kind: &str, stage: &str) -> String {
    let tier = STAGES.iter().find(|s| s.0 == stage).map(|s| s.2).unwrap_or(stage);
    if STAGES.iter().any(|s| kind.split_whitespace().last() == Some(s.2)) {
        kind.to_string()
    } else {
        format!("{kind} {tier}")
    }
}

/// The repository's farms folder from the circuits' root (`<repo>/designs/circuits`).
fn farms_root(circuits_root: &Path) -> PathBuf {
    circuits_root.parent().map(|p| p.join("farms")).unwrap_or_else(|| PathBuf::from("designs/farms"))
}

/// A farm study's variation folders that have designs: (stage, folder). Only the iron farm has
/// stages (designs/farms/iron/<stage>/).
pub fn variation_dirs(circuits_root: &Path, study: &str) -> Vec<(String, PathBuf)> {
    if study != "mobfarm" {
        return Vec::new();
    }
    let base = farms_root(circuits_root).join("iron");
    STAGES.iter().map(|s| (s.0.to_string(), base.join(s.0))).filter(|(_, d)| d.join(circuit::archive::VERDICTS).exists()).collect()
}

/// One row of a stage's `variations.tsv`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Variation {
    pub file: String,
    /// The stage's pick, listed first: `yes`, or the value of the stage's fork it is the pick for
    /// (`redstone`: day 1 has one pick per redstone fork value, ironspec rev 9 iteration 19); empty:
    /// not a pick.
    pub pick: String,
    /// A player-facing name (else the design's `how:` line).
    pub name: String,
    /// Why it is held off the site (a bar, a takedown: ironspec's); empty: on the site.
    pub held: String,
    /// Columns this tool does not know (another lane's), kept as they are when it writes the file.
    pub extra: Vec<(String, String)>,
}

impl Variation {
    pub fn is_pick(&self) -> bool {
        !self.pick.is_empty()
    }
}

const VARIATIONS: &str = "variations.tsv";
const VARIATIONS_COLUMNS: [&str; 4] = ["file", "pick", "name", "held"];

/// Make `file` the stage's pick for `pick` (`yes`, or a fork value: `redstone`): only the rows
/// holding that same value lose it; every other pick stays (the day-1 lane 2026-11-08: a `--pick`
/// had cleared the redstone pick, by a binary older than the fork picks).
pub fn apply_pick(vars: &mut [Variation], file: &str, pick: &str) {
    for v in vars.iter_mut() {
        if v.file == file {
            v.pick = pick.to_string();
        } else if v.pick == pick {
            v.pick.clear();
        }
    }
}

/// A tab-separated file's rows by its header's column names.
fn tsv_rows(text: &str) -> Vec<std::collections::BTreeMap<String, String>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(head) = lines.next() else { return Vec::new() };
    let cols: Vec<&str> = head.split('\t').map(str::trim).collect();
    lines.map(|l| cols.iter().zip(l.split('\t')).map(|(c, v)| (c.to_string(), v.trim().to_string())).collect()).collect()
}

/// A stage folder's `variations.tsv` rows (none without the file).
pub fn read_variations(dir: &Path) -> Vec<Variation> {
    let text = std::fs::read_to_string(dir.join(VARIATIONS)).unwrap_or_default();
    // (the header's own order, for the columns this tool does not know)
    let head: Vec<String> = text.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).map(|h| h.split('\t').map(|c| c.trim().to_string()).collect()).unwrap_or_default();
    tsv_rows(&text)
        .into_iter()
        .filter_map(|r| {
            let file = r.get("file").cloned().filter(|f| !f.is_empty())?;
            let pick = r.get("pick").map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty() && p != "no").unwrap_or_default();
            let extra = head.iter().filter(|c| !c.is_empty() && !VARIATIONS_COLUMNS.contains(&c.as_str())).map(|c| (c.clone(), r.get(c).cloned().unwrap_or_default())).collect();
            Some(Variation { file, pick, name: r.get("name").cloned().unwrap_or_default(), held: r.get("held").cloned().unwrap_or_default(), extra })
        })
        .collect()
}

fn write_variations(dir: &Path, rows: &[Variation]) -> Result<()> {
    let mut extra_cols: Vec<String> = Vec::new();
    for r in rows {
        for (c, _) in &r.extra {
            if !extra_cols.contains(c) {
                extra_cols.push(c.clone());
            }
        }
    }
    let clean = |s: &str| s.replace(['\t', '\n'], " ");
    let mut out = VARIATIONS_COLUMNS.iter().map(|c| c.to_string()).chain(extra_cols.iter().cloned()).collect::<Vec<_>>().join("\t");
    out.push('\n');
    for r in rows {
        let mut cells = vec![r.file.clone(), r.pick.clone(), clean(&r.name), clean(&r.held)];
        cells.extend(extra_cols.iter().map(|c| r.extra.iter().find(|(k, _)| k == c).map(|(_, v)| clean(v)).unwrap_or_default()));
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    std::fs::write(dir.join(VARIATIONS), out)?;
    Ok(())
}

/// One point of a module's copies curve (`<stem>.copies.tsv` beside its design, written by the
/// farm optimizer from its copies pricing; ironspec 2026-11-09): N copies of the module, each
/// module's build minutes, room (blocks of volume) and iron an hour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Copies {
    pub n: u32,
    pub build_minutes: f64,
    pub volume: f64,
    pub iron_per_hour: f64,
}

/// A design's copies curve (`<stem>.copies.tsv` in `dir`: `n build_minutes_per_module
/// volume_per_module iron_per_hour_per_module`, `#` lines are notes), by N; none without the file.
pub fn read_copies(dir: &Path, stem: &str) -> Vec<Copies> {
    let text = std::fs::read_to_string(dir.join(format!("{stem}.copies.tsv"))).unwrap_or_default();
    let num = |r: &std::collections::BTreeMap<String, String>, c: &str| r.get(c).and_then(|v| v.replace(',', "").trim().parse::<f64>().ok());
    let mut v: Vec<Copies> = tsv_rows(&text)
        .iter()
        .filter_map(|r| Some(Copies { n: num(r, "n")? as u32, build_minutes: num(r, "build_minutes_per_module")?, volume: num(r, "volume_per_module")?, iron_per_hour: num(r, "iron_per_hour_per_module")? }))
        .filter(|c| c.n > 0)
        .collect();
    v.sort_by_key(|c| c.n);
    v
}

/// A copies curve as a build's metrics (`copies build minutes at 4 16.7`, `copies volume at 4
/// 10080`, `copies iron per hour at 4 461`): its card fields `copies_build_minutes_at_4`...
pub fn copies_metrics(rows: &[Copies]) -> Vec<(String, String)> {
    rows.iter()
        .flat_map(|c| {
            [
                (format!("copies build minutes at {}", c.n), format!("{:.1}", c.build_minutes)),
                (format!("copies volume at {}", c.n), format!("{:.0}", c.volume)),
                (format!("copies iron per hour at {}", c.n), format!("{:.0}", c.iron_per_hour)),
            ]
        })
        .collect()
}

/// The redstone dust a design's blocks take to craft, by recipe (ironspec rev 9 iteration 19: a
/// day-1 card says plainly whether its farm needs redstone and how much): a dust, a redstone torch
/// 1; a repeater 3 (2 torches and a dust); a comparator 3 (3 torches); an observer 2; a piston, a
/// sticky piston, a dispenser, a dropper, a note block 1; a redstone lamp, a target 4; a crafter 3
/// (2 and its dropper's); a block of redstone 9; a powered, detector or activator rail 1 per 6 (the
/// recipe makes 6); a copper bulb 1 per 4 (the recipe makes 4). None: the design does not parse.
pub fn redstone_dust(text: &str) -> Option<u32> {
    let d = mctas::design::Design::parse(text).ok()?;
    let (mut dust, mut rails, mut bulbs) = (0u32, 0u32, 0u32);
    for s in d.blocks.values() {
        let id = s.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        dust += match id {
            "redstone_wire" | "redstone_torch" | "redstone_wall_torch" => 1,
            "repeater" | "comparator" | "crafter" => 3,
            "observer" => 2,
            "piston" | "sticky_piston" | "dispenser" | "dropper" | "note_block" => 1,
            "redstone_lamp" | "target" => 4,
            "redstone_block" => 9,
            _ => 0,
        };
        if matches!(id, "powered_rail" | "detector_rail" | "activator_rail") {
            rails += 1;
        }
        if id.ends_with("copper_bulb") {
            bulbs += 1;
        }
    }
    Some(dust + rails.div_ceil(6) + bulbs.div_ceil(4))
}

/// One member of a component's catalog (`members.tsv`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Member {
    pub part: String,
    pub file: String,
    pub name: String,
    pub stage: String,
    /// `key value` pairs, in order (the optimizer's metrics).
    pub metrics: Vec<(String, String)>,
    /// The stems of the full farms built with it.
    pub used_by: Vec<String>,
    /// Its design's path in the repository (and on the site): the part's folder, else a judged
    /// design of its stage (`designs/farms/iron/<stage>/`: a farm module, ironspec 2026-11-08).
    pub src: String,
    /// A judged stage design's kind (`mob-farm iron_ingot middle`): it runs in its viewer like a
    /// farm's page; empty: a part that does not run alone (the orbit and layers viewer).
    pub kind: String,
    /// The terrain sample it is judged in, if any (its viewer fetches it, `data-terrain`).
    pub terrain: Option<String>,
    /// Its site offset there (`x,y,z`: [`crate::pages::site_of`]), for its viewer's `data-site`.
    pub site: Option<String>,
}

impl Member {
    pub fn stem(&self) -> &str {
        self.file.trim_end_matches(".txt")
    }
}

/// A component folder's members, best first (none without its `members.tsv`). A member whose file
/// is not in the part's folder is a judged design of its stage (`designs/farms/<farm>/<stage>/`),
/// its numbers then read from its verdict when `metrics` is empty: its judge's, never typed.
pub fn read_members(repo: &Path, farm: &str, part: &str) -> Vec<Member> {
    let rel = format!("designs/farms/{farm}/parts/{part}");
    let dir = repo.join(&rel);
    let text = std::fs::read_to_string(dir.join("members.tsv")).unwrap_or_default();
    tsv_rows(&text)
        .into_iter()
        .filter_map(|r| {
            let file = r.get("file").cloned().filter(|f| !f.is_empty())?;
            let file = if file.ends_with(".txt") { file } else { format!("{file}.txt") };
            let stage = r.get("stage").cloned().unwrap_or_default();
            let staged = STAGES.iter().find(|s| s.0 == stage || s.2 == stage).map(|s| format!("designs/farms/{farm}/{}", s.0));
            // (a member kept elsewhere in the repository, named by its path: the carousel's
            // assembly, designs/farms/iron/carousel-assembly/carousel-v0a.txt)
            let (src, from_stage, file) = if file.starts_with("designs/") && repo.join(&file).exists() {
                let name = Path::new(&file).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                (file.clone(), None, name)
            } else if dir.join(&file).exists() {
                (format!("{rel}/{file}"), None, file)
            } else {
                let s = staged.filter(|s| repo.join(s).join(&file).exists())?;
                (format!("{s}/{file}"), Some(s), file)
            };
            let mut metrics = parse_member_metrics(r.get("metrics").map(String::as_str).unwrap_or(""));
            let row = from_stage.as_ref().and_then(|s| circuit::archive::read_rows(&repo.join(s)).ok()).and_then(|rows| rows.into_iter().find(|x| x.file == file));
            if metrics.is_empty()
                && let Some(row) = &row
            {
                // (a module's numbers only: iron an hour, blocks, build minutes, villagers; no stage
                // rank or tier, ironspec 2026-11-08)
                let f = crate::cards::fields(&crate::circuits::parse_metrics(&row.metrics), row.blocks as u32, &row.dims);
                for (key, label) in [("iron_per_hour", "iron per hour"), ("blocks", "blocks"), ("build_minutes", "build minutes"), ("villagers", "villagers")] {
                    if let Some(v) = f.get(key).filter(|v| !v.is_empty()) {
                        metrics.push((label.to_string(), v.clone()));
                    }
                }
            }
            let design = std::fs::read_to_string(repo.join(&src)).ok();
            let terrain = design.as_deref().and_then(crate::pages::terrain_of).map(String::from);
            let site = design.as_deref().filter(|_| terrain.is_some()).and_then(crate::pages::site_of);
            Some(Member {
                part: part.to_string(),
                name: r.get("name").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| file.trim_end_matches(".txt").to_string()),
                stage,
                metrics,
                used_by: r.get("used_by").map(|u| u.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                src,
                kind: row.map(|r| r.kind).unwrap_or_default(),
                terrain,
                site,
                file,
            })
        })
        .collect()
}

/// The site path of a member's design file (the same in the repository).
pub fn member_design(_farm: &str, m: &Member) -> String {
    m.src.clone()
}

/// The optimizer's member metrics (`members.tsv`: `key value, key value`) as (key, value): a known
/// key of several words first (`watching villagers 2`, `hide time 28 ticks`, `storage hours at 480
/// iron/h 3.2`); else the last word when it is the number (`spawn success per check 88%`, `in sight
/// of the villagers yes`); else from the first number on (`drop 22 blocks`); else the first word.
pub fn parse_member_metrics(s: &str) -> Vec<(String, String)> {
    const KEYS: [&str; 12] = ["storage hours at 480 iron/h", "in sight of the villagers", "spawn success per check", "blocks with wiring", "watching villagers", "build seconds", "build minutes", "iron per hour", "lava buckets", "floor cells", "hide time", "storage hours"];
    let number = |t: &str| t.trim_end_matches(['%', 's']).replace(',', "").parse::<f64>().is_ok() || t == "yes" || t == "no";
    // (split at the commas outside parentheses: `blocks 332 (pad 210, Nether 122)` is one)
    let mut items: Vec<String> = vec![String::new()];
    let mut depth = 0i32;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth <= 0 && chars.peek() == Some(&' ') => {
                chars.next();
                items.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(last) = items.last_mut() {
            last.push(ch);
        }
    }
    items
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let Some(k) = KEYS.iter().find(|k| p.strip_prefix(**k).is_some_and(|r| r.starts_with(' '))) {
                return (k.to_string(), p[k.len() + 1..].trim().to_string());
            }
            let words: Vec<&str> = p.split_whitespace().collect();
            if words.len() > 1 && number(words[words.len() - 1]) {
                return (words[..words.len() - 1].join(" "), words[words.len() - 1].to_string());
            }
            if let Some(i) = words.iter().position(|w| w.starts_with(|c: char| c.is_ascii_digit())).filter(|i| *i > 0) {
                return (words[..i].join(" "), words[i..].join(" "));
            }
            match p.split_once(' ') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (p.to_string(), String::new()),
            }
        })
        .collect()
}

/// A member's design as the site serves it: its working comments out (the parent 2026-11-08: no
/// internal notes in a published design), a header naming it and its page.
pub fn member_text(farm: &str, m: &Member, text: &str) -> String {
    let title = part_words(&m.part).0;
    let body: String = text.lines().filter(|l| !l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    format!("# {title}: {}\n# {}{}#{}\n{body}", m.name, crate::deploy::PAGES_URL, part_page_path(farm, &m.part), m.stem())
}

/// A member's viewer on its component's page (vjeux 2026-11-08 16:26, "Why is it not
/// interactive?"): the full viewer a farm's page has. A judged whole farm (a farm module) runs as
/// its farm page's does (its kind: play, speed, the scrubber, the spawn spots, day and night); a
/// part that does not run alone gets the same viewer without a run to play (turn it, look inside).
fn member_viewer(prefix: &str, m: &Member) -> String {
    let mut a = String::new();
    if !m.kind.is_empty() {
        a.push_str(&format!(" data-input-mode=\"{}\" data-circuit=\"{}\"", crate::explain::input_mode(&m.kind), esc(&m.kind)));
    }
    if let Some(t) = &m.terrain {
        a.push_str(&format!(" data-terrain=\"{}\"", esc(&format!("{prefix}{}", crate::pages::terrain_file(t)))));
    }
    if let Some(s) = &m.site {
        a.push_str(&format!(" data-site=\"{}\"", esc(s)));
    }
    let title = part_words(&m.part).0;
    format!(
        "<div class=\"viewer k-circuit\" data-mcview=\"full\" data-src=\"{}\"{a} data-kind=\"circuit\"><div class=\"fallback\"><span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>",
        esc(&format!("{prefix}{}", m.src)),
        esc(&if is_carousel(&m.part) { carousel_name(&m.name) } else { m.name.clone() }),
        esc(title)
    )
}

/// The site path of a component's page.
pub fn part_page_path(farm: &str, part: &str) -> String {
    format!("farms/{farm}/parts/{part}/index.html")
}

static PARTS_LOADED: std::sync::RwLock<Vec<Member>> = std::sync::RwLock::new(Vec::new());

/// One part of a listed farm (`<stem>.parts.tsv` beside its design, written by the farm
/// optimizer: each block tagged by the part that placed it; the parent 2026-11-08 20:59).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarmPart {
    /// The part's folder (`cage`, `sleep-trigger` for `calm`...: [`part_folder`]).
    pub part: String,
    /// Its member's stem in the part's catalog (`parts/<part>/members.tsv`), if it is one.
    pub member: String,
    pub blocks: u32,
    pub dug: u32,
    pub build_seconds: f64,
}

static FARM_PARTS: std::sync::RwLock<std::collections::BTreeMap<String, Vec<FarmPart>>> = std::sync::RwLock::new(std::collections::BTreeMap::new());

/// The repository the farms' designs were loaded from (`load_parts`): a farm page without a parts
/// file splits its design from there (`crate::split`).
static REPO: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// An iron farm's parts from its design alone, when its designer wrote no parts file (the parent
/// 2026-11-08 20:59: every farm page lists its parts): `crate::split`, at its stage's prices, the
/// card's build minutes shared out. None (and a note) for a design of another shape.
fn split_parts(b: &Build) -> Option<Vec<FarmPart>> {
    let Some(text) = design_text(b) else {
        eprintln!("  note: {}: no parts table (its design is not in the repository)", b.design);
        return None;
    };
    let stage = stage_of(b).unwrap_or(1);
    let card = card_build(b).and_then(|c| c.minutes).map(|m| m * 60.0);
    match crate::split::split(&text, stage, card) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) => None,
        Err(e) => {
            eprintln!("  note: {}: no parts table ({e:#})", b.design);
            None
        }
    }
}

/// A listed farm's design text from the repository (an archived farm's site copy sits beside its
/// study's archive folder: `designs/circuits/mobfarm/iron-day1.txt` is the repository's
/// `designs/circuits/mobfarm/archive/iron-day1.txt`).
fn design_text(b: &Build) -> Option<String> {
    let repo = REPO.read().ok()?.clone()?;
    let archived = Path::new(&b.design).parent().map(|p| p.join("archive")).zip(Path::new(&b.design).file_name()).map(|(d, f)| d.join(f));
    std::iter::once(PathBuf::from(&b.design)).chain(archived).find_map(|p| std::fs::read_to_string(repo.join(p)).ok())
}

/// Whether a farm's parts file still describes its design: its rows' blocks placed are the
/// design's (farmopt's builder counts every block it placed; a design changed since, say trimmed,
/// places another number: the day-1 lane's trim of bed-plate-lava-out-of-sight 2026-11-09, 102 in
/// the file against the design's 71). Unknown (no design to read): taken as current.
fn parts_current(b: &Build, rows: &[FarmPart]) -> bool {
    let Some(n) = design_text(b).and_then(|t| crate::split::placed_blocks(&t)) else { return true };
    let file: u32 = rows.iter().map(|p| p.blocks).sum();
    if file != n {
        eprintln!("  note: {}.parts.tsv: {file} blocks placed, its design {n}: the design changed since; its own split instead", b.name);
    }
    file == n
}

/// A farm's parts file's rows (`part member blocks dug build_seconds`, `#` lines are notes),
/// in the file's order; a part name the site does not know fails, naming it.
pub fn read_farm_parts(text: &str) -> Result<Vec<FarmPart>> {
    let mut v = Vec::new();
    for r in tsv_rows(text) {
        let key = r.get("part").cloned().unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let part = part_folder(&key).with_context(|| format!("unknown part {key:?} (one of {})", PARTS.iter().map(|p| if p.1.is_empty() { p.0.to_string() } else { format!("{} ({})", p.1, p.0) }).collect::<Vec<_>>().join(", ")))?;
        let num = |c: &str| r.get(c).and_then(|v| v.replace(',', "").parse::<f64>().ok()).unwrap_or(0.0);
        v.push(FarmPart { part: part.to_string(), member: r.get("member").cloned().unwrap_or_default().trim_end_matches(".txt").to_string(), blocks: num("blocks") as u32, dug: num("dug") as u32, build_seconds: num("build_seconds") });
    }
    Ok(v)
}

/// Every listed farm's parts: `designs/farms/<farm>/<stage>/<stem>.parts.tsv` and
/// `designs/circuits/mobfarm/archive/<stem>.parts.tsv`, by stem.
fn load_farm_parts(repo: &Path) -> Result<std::collections::BTreeMap<String, Vec<FarmPart>>> {
    let mut dirs: Vec<PathBuf> = STAGES.iter().map(|s| repo.join("designs/farms/iron").join(s.0)).collect();
    dirs.push(repo.join("designs/circuits/mobfarm/archive"));
    let mut all = std::collections::BTreeMap::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".parts.tsv") {
                let text = std::fs::read_to_string(e.path())?;
                all.insert(stem.to_string(), read_farm_parts(&text).with_context(|| format!("{}", e.path().display()))?);
            }
        }
    }
    Ok(all)
}

/// Load every iron-farm component's members and every listed farm's parts (the build pages link a
/// farm to its parts, the parts' pages to the farms built with them).
pub fn load_parts(repo: &Path) -> Vec<Member> {
    if let Ok(mut g) = REPO.write() {
        *g = Some(repo.to_path_buf());
    }
    let all: Vec<Member> = all_part_folders().into_iter().flat_map(|p| part_members(repo, p)).collect();
    if let Ok(mut g) = PARTS_LOADED.write() {
        *g = all.clone();
    }
    match load_farm_parts(repo) {
        Ok(f) => {
            // (a parts file naming a version its part's catalog lacks: its row links to the part's
            // page instead; said, so the optimizer can add it)
            for (stem, parts) in &f {
                for p in parts.iter().filter(|p| !p.member.is_empty() && !all.iter().any(|m| m.part == p.part && m.stem() == p.member)) {
                    eprintln!("  note: {stem}.parts.tsv: {} names {}, which designs/farms/iron/parts/{}/members.tsv does not list", p.part, p.member, p.part);
                }
            }
            if let Ok(mut g) = FARM_PARTS.write() {
                *g = f;
            }
        }
        Err(e) => eprintln!("  WARNING: the farms' parts files: {e:#}"),
    }
    all
}

/// A farm's parts (its `<stem>.parts.tsv`), none without the file.
pub fn farm_parts(stem: &str) -> Vec<FarmPart> {
    FARM_PARTS.read().ok().and_then(|g| g.get(stem).cloned()).unwrap_or_default()
}

/// The stems of the listed farms a member is built into: its `used_by`, and every farm whose parts
/// file names it.
pub fn farms_using(m: &Member) -> Vec<String> {
    let mut v = m.used_by.clone();
    if let Ok(g) = FARM_PARTS.read() {
        for (stem, parts) in g.iter() {
            if parts.iter().any(|p| p.part == m.part && p.member == m.stem()) && !v.contains(stem) {
                v.push(stem.clone());
            }
        }
    }
    v
}

/// The loaded members a full farm (by its design's stem) is built with.
pub fn parts_of(stem: &str) -> Vec<Member> {
    PARTS_LOADED.read().map(|g| g.iter().filter(|m| m.used_by.iter().any(|u| u == stem)).cloned().collect()).unwrap_or_default()
}

/// The component pages the loaded parts make (one per part with members).
pub fn loaded_part_pages() -> Vec<String> {
    let parts: std::collections::BTreeSet<String> = PARTS_LOADED.read().map(|g| g.iter().map(|m| m.part.clone()).collect()).unwrap_or_default();
    parts.into_iter().map(|p| part_page_path("iron", &p)).collect()
}

/// Build seconds as a player reads them: `40 s`, `3 min`, `1 h 05 min`.
fn build_time(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", (s + 30) / 60),
        _ => format!("{} h {:02} min", s / 3600, (s % 3600 + 30) / 60),
    }
}

/// A farm card's build line (`build time 39 min at day-1 prices (203 placed with 31 cobblestone
/// seals, 617 dug, ...)`, `build minutes 39.0`): what the whole build takes where it stands.
struct CardBuild {
    placed: u64,
    dug: u64,
    minutes: Option<f64>,
    /// What the placed blocks include beyond the design's own (`309 cobblestone slabs capping stray
    /// golem spots and 28 cobblestone seals`), empty when nothing.
    extras: String,
}

fn card_build(b: &Build) -> Option<CardBuild> {
    let c = b.circuit.as_ref()?;
    let line = c.metrics.iter().find(|(k, _)| k == "build time").map(|(_, v)| v.as_str())?;
    let inner = line.split_once('(').map(|x| x.1).unwrap_or("");
    let num_before = |word: &str| -> Option<u64> {
        let i = inner.find(word)?;
        inner[..i].trim_end().rsplit(|ch: char| !ch.is_ascii_digit()).next()?.parse().ok()
    };
    let placed = num_before(" placed")?;
    let dug = num_before(" dug").unwrap_or(0);
    let extras = inner.split_once(" placed with ").and_then(|(_, r)| r.split(", ").next()).map(|s| s.trim().to_string()).unwrap_or_default();
    let minutes = c.metrics.iter().find(|(k, _)| k == "build minutes").and_then(|(_, v)| v.split_whitespace().next()?.parse::<f64>().ok());
    Some(CardBuild { placed, dug, minutes, extras })
}

/// A full farm page's "What it is made of" section (the parent 2026-11-08 20:59: a visitor sees
/// what each part takes): each part of its parts file with its blocks placed, its blocks dug and
/// its build time, linking to that part's version on its component page; else (no parts file) the
/// members whose `used_by` names it; empty when it has neither.
pub fn parts_section(b: &Build, prefix: &str) -> String {
    let farm = site_model::build_farm_folder(b).unwrap_or_default();
    if farm.is_empty() {
        return String::new();
    }
    let mut rows = farm_parts(&b.name);
    // (a parts file its design no longer matches: the design's own split, as for an iron farm with
    // no parts file)
    if !rows.is_empty() && farm == "iron" && !parts_current(b, &rows) {
        rows.clear();
    }
    if rows.is_empty() && farm == "iron" {
        rows = split_parts(b).unwrap_or_default();
    }
    if !rows.is_empty() {
        let members = PARTS_LOADED.read().map(|g| g.clone()).unwrap_or_default();
        let has_page = |part: &str| members.iter().any(|m| m.part == part);
        let mut body = String::new();
        for p in &rows {
            let (title, _) = part_words(&p.part);
            let name = match members.iter().find(|m| m.part == p.part && !p.member.is_empty() && m.stem() == p.member) {
                Some(m) => format!("<a href=\"{prefix}{}#{}\">{}</a>", esc(&part_page_path(&farm, &p.part)), esc(m.stem()), esc(&m.name)),
                None if has_page(&p.part) => format!("its own · <a href=\"{prefix}{}\">other versions</a>", esc(&part_page_path(&farm, &p.part))),
                None => String::new(),
            };
            body.push_str(&format!(
                "<tr><th scope=\"row\">{}</th><td>{name}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(title),
                site_model::html::group_thousands(p.blocks as u64),
                site_model::html::group_thousands(p.dug as u64),
                esc(&build_time(p.build_seconds))
            ));
        }
        let (blocks, dug, secs) = rows.iter().fold((0u64, 0u64, 0.0f64), |a, p| (a.0 + p.blocks as u64, a.1 + p.dug as u64, a.2 + p.build_seconds));
        // what the farm takes where it is built, beyond its own parts (the card's build line: the
        // slabs capping the spots where a stray golem could spawn, the seals, the digging at the
        // spot), so the parts and that row add up to the whole farm's card
        let card = card_build(b);
        let (all_placed, all_dug, all_secs) = match &card {
            Some(c) => (c.placed.max(blocks), c.dug.max(dug), c.minutes.map(|m| m * 60.0).unwrap_or(secs).max(secs)),
            None => (blocks, dug, secs),
        };
        if all_placed > blocks || all_dug > dug {
            let extra = all_secs - secs;
            body.push_str(&format!(
                "<tr><th scope=\"row\">At the farm's spot</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&card.as_ref().map(|c| c.extras.clone()).filter(|e| !e.is_empty()).unwrap_or_else(|| "fitting it into the ground".to_string())),
                site_model::html::group_thousands(all_placed - blocks),
                site_model::html::group_thousands(all_dug - dug),
                if extra >= 30.0 { esc(&build_time(extra)) } else { "counted above".to_string() }
            ));
        }
        body.push_str(&format!(
            "<tr class=\"total\"><th scope=\"row\">The whole farm</th><td></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            site_model::html::group_thousands(all_placed),
            site_model::html::group_thousands(all_dug),
            esc(&build_time(all_secs))
        ));
        return format!(
            "<section><h2>What it is made of</h2><p>Each part of this farm, what it takes to build, and the version of it this farm uses: follow a part to see its other versions side by side.</p><div class=\"scroll-x\"><table class=\"list parts\"><thead><tr><th>part</th><th>this farm's</th><th>blocks placed</th><th>blocks dug</th><th>build time</th></tr></thead><tbody>{body}</tbody></table></div></section>\n"
        );
    }
    let used = parts_of(&b.name);
    if used.is_empty() {
        return String::new();
    }
    let items: String = PARTS
        .iter()
        .flat_map(|p| used.iter().filter(move |m| m.part == p.0).map(move |m| (p.2, m)))
        .map(|(title, m)| format!("<li><b>{}</b>: <a href=\"{prefix}{}#{}\">{}</a></li>", esc(title), esc(&part_page_path(&farm, &m.part)), esc(m.stem()), esc(&m.name)))
        .collect();
    format!("<section><h2>What it is made of</h2><ul class=\"parts\">{items}</ul></section>\n")
}

/// The builds of a farm folder (`iron`, `melon-pumpkin`...), the variations included.
fn farm_builds<'a>(site: &'a Site, folder: &str) -> Vec<&'a Build> {
    site.builds.iter().filter(|b| site_model::build_farm_folder(b).as_deref() == Some(folder)).collect()
}

/// A farm build's iron an hour (for the order under its stage), if measured.
fn rate(b: &Build) -> f64 {
    b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "iron per hour")).and_then(|(_, v)| v.split_whitespace().next()?.parse().ok()).unwrap_or(0.0)
}

/// A crop farm's order on its page: each kind's pick first (a design kept beside it, flag
/// `size`, after), then the larger farm.
fn crop_key(b: &Build) -> (bool, String, i64) {
    (b.flags.iter().any(|f| f == "size"), b.circuit.as_ref().map(|c| c.kind.clone()).unwrap_or_default(), -(b.blocks as i64))
}

/// `farms/index.html`: the farm types, each a tile with a picture of one of its farms: the iron
/// farm first, then one per crop (vjeux 2026-11-08 13:14: a farm type is what it produces).
pub fn farms_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let mut tiles: Vec<String> = Vec::new();
    for folder in farm_folders(site) {
        let mut list = farm_builds(site, &folder);
        list.sort_by_key(|b| crop_key(b));
        // (the picture: the iron farm's first stage pick, else its archived design; a crop's pick)
        let first = if folder == "iron" { list.iter().find(|b| b.flags.iter().any(|f| f == "pick")).or_else(|| list.iter().find(|b| !is_variation(b))).or(list.first()) } else { list.first() };
        let Some(first) = first else { continue };
        tiles.push(format!(
            "<a class=\"card study\" href=\"{prefix}farms/{folder}/index.html\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{}</div></div></a>",
            esc(&farm_title(&folder)),
            site_model::html::thumb(prefix, site_model::html::card_src(first), first.trace.as_deref(), first.still.as_deref(), first.picture.as_deref(), &first.kind, "", ""),
            esc(&farm_title(&folder)),
            esc(&farm_blurb(&folder))
        ));
    }
    let body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Farms</p>\n<h1>Farms</h1>\n<p class=\"lede\">Farms make resources by themselves: build one, stay near it and empty its chests. Each page says what it makes, what it costs and how to build it, and lets you watch it work in 3D.</p>\n{}",
        pages::grid(&tiles)
    );
    pages::shell_full("Farms", "Redstone farms you can download: iron farms for every stage of the game, and a farm for each crop.", prefix, &body, scripts, site, None, Some("farm"))
}

/// `farms/<crop>/index.html`: a crop's farms (`melon-pumpkin`, `sugar-cane`, `bamboo`...), each
/// kind's pick first.
pub fn crop_hub(site: &Site, folder: &str, scripts: Scripts) -> String {
    let prefix = "../../";
    let mut list = farm_builds(site, folder);
    list.sort_by_key(|b| crop_key(b));
    let cards: Vec<String> = list.iter().map(|b| build_card(b, prefix)).collect();
    let title = farm_title(folder);
    let body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}farms/index.html\">Farms</a> › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}</p>\n<p class=\"hint\">The pictures show each farm with its chests; the block counts leave those out.</p>\n{}",
        esc(&title),
        esc(&title),
        esc(&farm_blurb(folder)),
        pages::grid(&cards)
    );
    pages::shell_full(&title, &farm_blurb(folder), prefix, &body, scripts, site, None, Some("farm"))
}

/// The order of an iron farm stage's farms: the stage's picks first (variations picked in its
/// variations.tsv: the `yes` pick, then each fork value's pick by its value, e.g. day 1's
/// no-redstone pick and then its redstone pick; else the archived design, the one live before the
/// variations), then the most iron an hour.
fn stage_key(b: &Build) -> (bool, String, bool, i64) {
    let pick = b.flags.iter().any(|f| f == "pick");
    let fork = b.flags.iter().find_map(|f| f.strip_prefix(PICK_FOR)).unwrap_or("").to_string();
    (!pick, fork, is_variation(b), -(rate(b) * 1000.0) as i64)
}

/// The flag of a pick for a fork value (`pick-for-redstone`), beside `pick`.
pub const PICK_FOR: &str = "pick-for-";

/// A pick's slot name on its stage, by what its farm needs (the parent 2026-11-08: day 1's two
/// pick slots, without and with redstone): its redstone dust (`farms::redstone_dust`, the
/// `redstone dust` metric), else its fork value.
fn pick_slot(b: &Build) -> String {
    let dust = b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "redstone dust")).and_then(|(_, v)| v.trim().parse::<u32>().ok());
    match dust {
        Some(0) => "Without redstone".to_string(),
        Some(_) => "With redstone".to_string(),
        None => match b.flags.iter().find_map(|f| f.strip_prefix(PICK_FOR)) {
            Some(v) => format!("With {}", v.replace('-', " ")),
            None => "Our pick".to_string(),
        },
    }
}

/// The folders whose judged designs an iron-farm card can quote without listing them: the bars
/// (`designs/farms/iron/bars/`: ironspec 2026-11-08, "a bar is never listed, only quoted as the
/// reference line on a farm's card") and each stage's variations, the ones held off the site too.
pub fn quotable_dirs(repo: &Path) -> Vec<PathBuf> {
    let iron = repo.join("designs/farms/iron");
    std::iter::once(iron.join("bars")).chain(STAGES.iter().map(|s| iron.join(s.0))).collect()
}

/// `farms/iron/index.html`: the iron farm's full farms by stage, every variation, each stage's pick
/// first; then its components.
pub fn iron_hub(site: &Site, parts: &[Member], scripts: Scripts) -> String {
    let prefix = "../../";
    let list = farm_builds(site, "iron");
    let mut stages = String::new();
    for (k, (_, title, _, line)) in STAGES.iter().enumerate() {
        let mut of: Vec<&&Build> = list.iter().filter(|b| stage_of(b) == Some(k)).collect();
        if of.is_empty() {
            continue;
        }
        // (the stage's picks first: variations picked in its variations.tsv, else the archived
        // design, the one live before the variations; then the most iron an hour)
        of.sort_by_key(|b| stage_key(b));
        // a stage with a pick per fork value (day 1: with and without redstone, ironspec rev 9
        // iteration 19; the parent 2026-11-08 20:59, "two pick slots"): each pick in its own slot,
        // named by what its farm needs, then the other farms
        let picks: Vec<&&&Build> = of.iter().filter(|b| b.flags.iter().any(|f| f == "pick")).collect();
        let forked = picks.iter().any(|b| b.flags.iter().any(|f| f.starts_with(PICK_FOR)));
        let body = if forked {
            let mut s = String::new();
            for b in &picks {
                s.push_str(&format!("<h4>{}</h4>\n{}", esc(&pick_slot(b)), pages::grid(&[build_card(b, prefix)])));
            }
            let rest: Vec<String> = of.iter().filter(|b| !b.flags.iter().any(|f| f == "pick")).map(|b| build_card(b, prefix)).collect();
            if !rest.is_empty() {
                s.push_str(&format!("<h4>Other farms</h4>\n{}", pages::grid(&rest)));
            }
            s
        } else {
            pages::grid(&of.iter().map(|b| build_card(b, prefix)).collect::<Vec<_>>())
        };
        stages.push_str(&format!("<section class=\"stage\"><h3>{}</h3><p class=\"hint\">{}</p>\n{body}</section>\n", esc(title), esc(line)));
    }
    let card = |p: &str, title: &str, what: &str| -> Option<String> {
        let members: Vec<&Member> = parts.iter().filter(|m| m.part == p).collect();
        let best = members.first()?;
        Some(format!(
            "<a class=\"card study\" href=\"{prefix}{}\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{} {}</div></div></a>",
            esc(&part_page_path("iron", p)),
            esc(title),
            site_model::html::thumb(prefix, &member_design("iron", best), None, None, None, "circuit", "", ""),
            esc(title),
            esc(what),
            if members.len() == 1 { "1 version.".to_string() } else { format!("{} versions.", members.len()) }
        ))
    };
    let comps: Vec<String> = PARTS.iter().filter_map(|(p, _, title, what)| card(p, title, what)).collect();
    let mut components = if comps.is_empty() {
        String::new()
    } else {
        format!("<h2>Components</h2>\n<p class=\"hint\">The pieces an iron farm is made of, each with the best versions to build into your own farm.</p>\n{}", pages::grid(&comps))
    };
    // the villager carousel's parts, under their own heading while it is being built (vjeux
    // 2026-11-10: "Can you publish the components in the site so I can see how it looks like")
    let carousel: Vec<String> = CAROUSEL.iter().filter_map(|(p, _, title, what)| card(p, title, what)).collect();
    if !carousel.is_empty() {
        components.push_str(&format!(
            "<h2 id=\"carousel\">Villager carousel (in progress)</h2>\n<p class=\"hint\">An iron farm being built: batches of villagers ride to one station by turns, and each batch summons its golem there. Each part as it stands today, with what it measured.</p>\n{}",
            pages::grid(&carousel)
        ));
    }
    let body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}farms/index.html\">Farms</a> › Iron farm</p>\n<h1>Iron farm</h1>\n<p class=\"lede\">{}</p>\n<h2>Full farms</h2>\n{stages}{components}",
        esc(crate::explain::study_uses("mobfarm"))
    );
    pages::shell_full("Iron farm", "Iron farms you can download, for every stage of the game: what each makes, costs and needs, and their parts.", prefix, &body, scripts, site, None, Some("farm"))
}

/// `farms/<farm>/parts/<part>/index.html`: a component's members, best first, each with its 3D
/// view, its numbers and the full farms built with it.
pub fn part_page(site: &Site, farm: &str, part: &str, members: &[Member], scripts: Scripts) -> String {
    let path = part_page_path(farm, part);
    let prefix_s = pages::prefix_of(&path);
    let prefix = prefix_s.as_str();
    let (title, what) = part_words(part);
    // (a carousel part: its crumbs and a line naming the farm it is for, being built)
    let (crumb, note) = if is_carousel(part) {
        (
            format!(" › <a href=\"{prefix}farms/{farm}/index.html#carousel\">Villager carousel (in progress)</a>"),
            "<p class=\"hint\">Part of the villager carousel, an iron farm still being built: this part as it stands today, with what it measured.</p>\n".to_string(),
        )
    } else {
        (String::new(), String::new())
    };
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}farms/index.html\">Farms</a> › <a href=\"{prefix}farms/{farm}/index.html\">{}</a>{crumb} › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}</p>\n{note}",
        esc(&farm_title(farm)),
        esc(title),
        esc(title),
        esc(what)
    );
    // the versions the listed farms are built with first (the parent 2026-11-08 20:59: each
    // component page lists its part's best members first, with their numbers), then the others,
    // each group in the catalog's order (best first)
    let listed = |m: &Member| farms_using(m).iter().any(|u| site.builds.iter().any(|b| &b.name == u && site_model::is_farm(b)));
    let mut members: Vec<&Member> = members.iter().collect();
    members.sort_by_key(|m| !listed(m));
    for m in members {
        let design = member_design(farm, m);
        // (its numbers as a player reads them: blocks placed, build time in minutes)
        let rows: String = m
            .metrics
            .iter()
            // (a carousel catalog's bench-only numbers stay off: the villagers' REST ticks, where
            // the smallest margin fell)
            .filter(|(k, _)| !(is_carousel(&m.part) && matches!(k.as_str(), "REST" | "arrival margin min at")))
            .map(|(k, v)| {
                let (k, v) = match k.as_str() {
                    _ if is_carousel(&m.part) => carousel_metric(k, v),
                    "blocks" => ("blocks placed".to_string(), v.clone()),
                    "build seconds" => ("build time".to_string(), v.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()).map(build_time).unwrap_or_else(|| v.clone())),
                    _ => (k.clone(), v.clone()),
                };
                format!("<tr><th scope=\"row\">{}</th><td>{}</td></tr>", esc(&k), esc(&crate::explain::group_numbers(&v)))
            })
            .collect();
        // (a module's copies curve beside its design: each N's build minutes, room and iron an
        // hour per module; ironspec 2026-11-09)
        let curve = Path::new(&m.src).parent().map(|d| read_copies(&REPO.read().ok().and_then(|g| g.clone()).unwrap_or_default().join(d), m.stem())).unwrap_or_default();
        let rows = rows
            + &curve
                .iter()
                .map(|c| {
                    let label = if c.n == 1 { "built once".to_string() } else { format!("built {} times", c.n) };
                    format!(
                        "<tr><th scope=\"row\">{}</th><td>{}{:.1} minutes to build, {} blocks of room, {} iron an hour</td></tr>",
                        esc(&label),
                        if c.n == 1 { "" } else { "per module: " },
                        c.build_minutes,
                        crate::explain::group_numbers(&format!("{:.0}", c.volume)),
                        crate::explain::group_numbers(&format!("{:.0}", c.iron_per_hour))
                    )
                })
                .collect::<String>();
        // (a farm module reads as a component: no stage line, ironspec 2026-11-08)
        let stage = if m.part == "module" { String::new() } else { STAGES.iter().find(|s| s.0 == m.stage).map(|s| format!("<tr><th scope=\"row\">stage</th><td>{}</td></tr>", esc(s.1))).unwrap_or_default() };
        let used: Vec<String> = farms_using(m)
            .iter()
            .filter_map(|u| site.builds.iter().find(|b| &b.name == u && site_model::is_farm(b)))
            .map(|b| format!("<a href=\"{prefix}{}\">{}</a>", esc(&b.page), esc(&pages::page_title(b))))
            .collect();
        // (a judged whole farm runs in its viewer: what to press, as on a farm's page)
        let try_line = if m.kind.is_empty() {
            String::new()
        } else {
            let c = site_model::Circuit { kind: m.kind.clone(), study: "mobfarm".into(), ..Default::default() };
            let t = crate::plain::try_it(&c, true);
            if t.is_empty() { String::new() } else { format!("<p>{}</p>", esc(&t)) }
        };
        body.push_str(&format!(
            "<section class=\"member\" id=\"{}\"><h2>{}</h2>{}{try_line}<table class=\"props\">{stage}{rows}</table>{}<p><a href=\"{prefix}{}\" download>Download it as text</a></p></section>\n",
            esc(m.stem()),
            // (a heading starts with a capital, whatever members.tsv says: "Bed-plate cage, ...";
            // a carousel member's versions and ticks in words)
            esc(&{
                let name = if is_carousel(&m.part) { carousel_name(&m.name) } else { m.name.clone() };
                let mut ch = name.chars();
                ch.next().map(|c| c.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
            }),
            member_viewer(prefix, m),
            if used.is_empty() { String::new() } else { format!("<p>Built into: {}</p>", used.join(", ")) },
            esc(&design)
        ));
    }
    pages::shell_full(title, what, prefix, &body, scripts, site, None, Some("farm"))
}

/// The real-game gate a mob farm must pass to go on the site (the brief's one rule: "works as its
/// card says, verified in the real game"; the parent and ironspec's G1 2026-11-08): the same gate
/// `circuit archive` runs (`circuit::mobfarm_game::confirm`: the 26.3 server, 8 builds, or a
/// design made for terrain in its sample's kept world), about 2 minutes a farm.
fn game_gate(c: &circuit::Circuit) -> Result<circuit::mobfarm_game::GateReport> {
    let mut go = circuit::mobfarm_game::GateOpts::new(mctas::DEFAULT_VERSION);
    if let Some(sample) = circuit::mobfarm::terrain_sample(c).map_err(|e| anyhow::anyhow!(e))? {
        go.terrain = Some(sample.name.to_string());
    }
    circuit::mobfarm_game::gate(c, &go)
}

/// A build's break, in the player's words (one of the gate's checks, `circuit::mobfarm_game`), or
/// None for what is not the farm's (a harness failure) or what the card says another way.
fn break_words(f: &str) -> Option<String> {
    let f = f.split_once(": ").filter(|(head, _)| head.starts_with("gt ")).map(|(_, rest)| rest).unwrap_or(f);
    let words = if f.contains("drops were lost") {
        "iron drops are lost".to_string()
    } else if f.contains("a golem appeared outside the farm") {
        "a golem spawns outside the farm".to_string()
    } else if f.contains("a golem left the farm") || f.contains("a golem vanished outside the farm") {
        "a golem gets out of the farm".to_string()
    } else if f.contains("is still alive") {
        format!("a golem stays alive past {}", site_model::real_time(redsim::entity::mob::farm::MAX_GOLEM_LIFE as u32))
    } else if f.contains(" villagers of ") {
        "a villager is lost".to_string()
    } else if f.contains("a villager was hurt") {
        "a villager gets hurt".to_string()
    } else if f.contains("a zombie villager in the world") {
        "a villager turns into a zombie villager".to_string()
    } else if let Some((n, of)) = f.split_once(" zombies of ") {
        let (n, of): (u32, u32) = (n.trim().parse().ok()?, of.trim().parse().ok()?);
        if n < of { "the zombie is lost".to_string() } else { "another zombie gets in".to_string() }
    } else if f.contains("the zombie was hurt") {
        "the zombie gets hurt".to_string()
    } else if f.contains("of the farm's drops outside the farm") {
        "drops land outside the farm".to_string()
    } else {
        return None;
    };
    Some(words)
}

/// What the real-game gate measured, written into a farm's metrics (ironspec rev 9 iteration 14,
/// the parent's ruling on vjeux 2026-11-08 16:49, "Seriously, just upload them"): a pass adds its
/// `real game` line; a failure is no refusal: where the game's rate falls outside the card's
/// interval (either way) the card's numbers become the game's (golems and iron an hour), and what
/// broke in its builds becomes `game tradeoffs` lines with their counts ("a golem stays alive past
/// 1 min in 1 of 8 builds"), never a real-game pass.
fn game_findings(metrics: &str, g: &circuit::mobfarm_game::GateReport, blocks: u32) -> String {
    if g.passed() {
        return format!("{metrics}, real game {}", g.line());
    }
    let mut m = crate::circuits::parse_metrics(metrics);
    let deaths: u32 = g.builds.iter().map(|b| b.deaths).sum();
    let iron: u32 = g.builds.iter().map(|b| b.iron()).sum();
    let per_golem = (deaths > 0).then(|| iron as f64 / deaths as f64);
    let num = |v: &str| v.split_whitespace().next().and_then(|x| x.parse::<f64>().ok());
    let get = |m: &[(String, String)], k: &str| m.iter().find(|(kk, _)| kk == k).and_then(|(_, v)| num(v));
    let set = |m: &mut Vec<(String, String)>, k: &str, v: String| {
        if let Some(e) = m.iter_mut().find(|(kk, _)| kk == k) {
            e.1 = v;
        }
    };
    for w in g.windows.iter().filter(|w| !w.passed() && w.sim > 0.0) {
        let iron_h = per_golem.map(|p| w.game * p);
        if w.name.contains("natural") {
            let old = get(&m, "iron per hour");
            let iron_h = iron_h.or_else(|| old.map(|o| o * w.game / w.sim));
            set(&mut m, "golems per hour", format!("{:.1}", w.game));
            set(&mut m, "seconds per golem", format!("{:.1}", 3600.0 / w.game.max(0.001)));
            if let Some(ih) = iron_h {
                set(&mut m, "iron per hour", format!("{ih:.0}"));
                set(&mut m, "iron per hour per block", format!("{:.2}", ih / blocks.max(1) as f64));
                if let Some(o) = old.filter(|o| *o > 0.0) {
                    for k in ["iron per hour per module", "iron per hour sleeping"] {
                        if let Some(x) = get(&m, k) {
                            set(&mut m, k, format!("{:.0}", x * ih / o));
                        }
                    }
                }
            }
            // (redsim's own interval and share are no longer the card's numbers)
            m.retain(|(k, _)| k != "iron range" && k != "share of ceiling");
        } else if let Some(ih) = iron_h {
            // the forced day of rain, of thunder: the rain line's own part
            let part = if w.name.contains("thunder") { "thunder day" } else if w.name.contains("rain") { "rain day" } else { continue };
            if let Some(e) = m.iter_mut().find(|(k, _)| k == "rain") {
                e.1 = e.1.split("; ").map(|p| if p.starts_with(part) { format!("{part} {ih:.0} iron/h") } else { p.to_string() }).collect::<Vec<_>>().join("; ");
            }
        }
    }
    // what broke, each kind once per build, with its count over the builds
    let mut counts: Vec<(String, usize)> = Vec::new();
    for b in &g.builds {
        let mut seen: Vec<String> = Vec::new();
        for w in b.failures.iter().filter_map(|f| break_words(f)) {
            if !seen.contains(&w) {
                seen.push(w);
            }
        }
        for w in seen {
            match counts.iter_mut().find(|(x, _)| *x == w) {
                Some(e) => e.1 += 1,
                None => counts.push((w, 1)),
            }
        }
    }
    if !counts.is_empty() {
        let n = g.builds.len();
        m.push(("game tradeoffs".into(), counts.iter().map(|(w, k)| format!("{w} in {k} of {n} builds")).collect::<Vec<_>>().join("; ")));
    }
    m.iter().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k} {v}") }).collect::<Vec<_>>().join(", ")
}

/// A design's judge and gate for its stage: its verdicts row (the redsim judge's numbers, the
/// game gate's findings unless `gate` is off), or why it cannot go on the site.
fn judge_for_stage(path: &Path, text: &str, c: &circuit::Circuit, stage: &str, file: &str, gate: bool) -> std::result::Result<circuit::archive::Row, String> {
    let kind = c.kind.clone().ok_or_else(|| "no `circuit: <kind>` line".to_string())?;
    // judged at its stage's prices (ironspec: the late pair's card read "48 min at middle prices"):
    // an untiered design's circuit line takes the stage's tier for the judge
    let tiered: String = text.lines().map(|l| match l.strip_prefix("circuit: ") {
        Some(k) => format!("circuit: {}\n", stage_kind(k.trim(), stage)),
        None => format!("{l}\n"),
    }).collect();
    let j = crate::preview::judge(&tiered);
    if !j.pass {
        return Err(format!("its judge does not pass it on redsim: {}", if j.why.is_empty() { j.summary.clone() } else { j.why.clone() }));
    }
    let mut metrics = j.metrics.clone();
    if gate && kind.name().starts_with("mob-farm") {
        eprintln!("{}: the real-game gate (about 2 minutes)...", path.display());
        let g = game_gate(c).map_err(|e| format!("the real-game gate could not run: {e:#}"))?;
        metrics = game_findings(&metrics, &g, c.blocks() as u32);
    }
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let kind_name = if j.kind.is_empty() { kind.name() } else { j.kind.clone() };
    Ok(circuit::archive::Row {
        archived: clock::catalog::now_utc(),
        file: file.to_string(),
        kind: stage_kind(&kind_name, stage),
        axis: j.axis.clone(),
        metrics,
        blocks: c.blocks(),
        dims: if j.dims.is_empty() { format!("{}x{}x{}", d[0], d[1], d[2]) } else { j.dims.clone() },
        volume: if j.volume > 0 { j.volume } else { (d[0] * d[1] * d[2]) as i64 },
        family: j.family.clone(),
        robustness: j.robustness.clone(),
        kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
        type_key: circuit::archive::type_key(c),
        hash: String::new(),
        summary: j.summary.clone(),
    })
}

fn write_verdicts(dir: &Path, rows: &[circuit::archive::Row]) -> Result<()> {
    let mut out = format!("{}\n", crate::preview::VERDICTS_HEADER);
    for r in rows {
        out.push_str(&crate::preview::verdicts_line(r));
        out.push('\n');
    }
    std::fs::write(dir.join(circuit::archive::VERDICTS), out)?;
    Ok(())
}

/// `mctas site variation --regate [design...]`: every listed variation of the farm (or only the ones
/// named) judged and gated again, its row rewritten with what the game measured: a pass adds the
/// `real game` line; a failure stays listed, its card showing the game's numbers where they fall
/// outside the card's interval and what broke as `game tradeoffs` lines (ironspec rev 9 iteration
/// 14). A held variation (a bar, a takedown: `held` is theirs alone) is left as it is; a design the
/// redsim judge no longer passes, or a gate that could not run, keeps its row and is reported.
fn regate(repo: &Path, farm: &str, only: &[PathBuf]) -> Result<()> {
    let named: Vec<String> = only.iter().filter_map(|p| p.file_name().map(|f| f.to_string_lossy().to_string())).collect();
    let mut report: Vec<String> = Vec::new();
    for (stage, _, _, _) in STAGES {
        let dir = repo.join("designs/farms").join(farm).join(stage);
        let vars = read_variations(&dir);
        if vars.is_empty() {
            continue;
        }
        let mut rows = circuit::archive::read_rows(&dir).unwrap_or_default();
        for v in vars.iter().filter(|v| named.is_empty() || named.contains(&v.file)) {
            if !v.held.is_empty() {
                report.push(format!("SKIP {stage}/{}: held ({})", v.file, v.held));
                continue;
            }
            let path = dir.join(&v.file);
            let res = std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display())).and_then(|text| {
                let c = circuit::Circuit::load(&path).map_err(|e| format!("{e:#}"))?;
                judge_for_stage(&path, &text, &c, stage, &v.file, true)
            });
            match res {
                Ok(row) => {
                    let ms = crate::circuits::parse_metrics(&row.metrics);
                    let line = |k: &str| ms.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());
                    report.push(match (line("real game"), line("game tradeoffs")) {
                        (Some(g), _) => format!("PASS {stage}/{}: real game {g}", v.file),
                        (None, t) => format!("FAIL {stage}/{} (listed; its card shows the game's numbers where they differ{}): iron per hour {}", v.file, t.map(|t| format!(" and what broke: {t}")).unwrap_or_default(), line("iron per hour").unwrap_or_default()),
                    });
                    rows.retain(|r| r.file != v.file);
                    rows.push(row);
                }
                Err(why) => report.push(format!("KEPT {stage}/{} as it was: {why}", v.file)),
            }
        }
        write_verdicts(&dir, &rows)?;
    }
    for l in &report {
        println!("{l}");
    }
    Ok(())
}

/// `mctas site variation`: a design joins its farm stage's variations (judged on redsim, then a mob
/// farm through the real-game gate: its numbers on the site are its judge's, confirmed in the
/// game), or a variation's row changes (`--pick`, `--name`); `--regate` re-checks every variation.
pub fn variation(a: &crate::VariationArgs) -> Result<()> {
    let repo = crate::published::repo_root();
    // (only the iron farm has stages: designs/farms/iron/<stage>/)
    if a.farm != "iron" {
        bail!("--farm {}: only the iron farm has stage variations (designs/farms/iron/<stage>/)", a.farm);
    }
    if a.regate {
        return regate(&repo, &a.farm, &a.designs);
    }
    if !STAGES.iter().any(|s| s.0 == a.stage) {
        bail!("--stage {}: one of {}", a.stage, STAGES.iter().map(|s| s.0).collect::<Vec<_>>().join(", "));
    }
    let study = "mobfarm";
    let dir = repo.join("designs/farms").join(&a.farm).join(&a.stage);
    std::fs::create_dir_all(&dir)?;
    let mut rows = circuit::archive::read_rows(&dir)?;
    let mut vars = read_variations(&dir);
    for path in &a.designs {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let c = circuit::Circuit::load(path)?;
        let Some(kind) = c.kind.clone() else { bail!("{}: no `circuit: <kind>` line", path.display()) };
        if kind.study() != study {
            bail!("{}: a {} design, not a {} farm", path.display(), kind.name(), a.farm);
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let file = format!("{stem}.txt");
        // (vjeux 2026-11-08 16:49, "Seriously, just upload them": `--no-gate` puts it up on its
        // redsim judge; `--regate` runs the game gate afterwards)
        let row = judge_for_stage(path, &text, &c, &a.stage, &file, !a.no_gate).map_err(|why| anyhow::anyhow!("{}: not on the site: {why}", path.display()))?;
        // its card must render as the site will render it (the parent 2026-11-08: a --no-gate
        // farm's missing weather fields failed the deploy's site build; caught here instead)
        let cards = crate::cards::read(&repo, study)?;
        let mine = crate::cards::fields(&crate::circuits::parse_metrics(&row.metrics), row.blocks as u32, &row.dims);
        let other = |s: &str| crate::cards::archived_fields(&repo, study, s);
        crate::cards::card_of(&cards, &format!("{}/{study}/cards.tsv", crate::circuits::ROOT), &stem, &row.kind, &mine, &other).map_err(|e| anyhow::anyhow!("{}: not on the site: its card does not render: {e:#}", path.display()))?;
        // the design with its judge's line on top, as the archive writes it
        let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.join(&file), format!("# {}\n{body}", row.summary.trim_start_matches("# ")))?;
        let shown = row.metrics.split(", ").take(3).collect::<Vec<_>>().join(", ");
        let gate_word = if a.no_gate {
            "not run (--no-gate: `site variation --regate` runs it)".to_string()
        } else if row.metrics.contains("real game ") {
            "passed".to_string()
        } else {
            let t = crate::circuits::parse_metrics(&row.metrics).into_iter().find(|(k, _)| k == "game tradeoffs").map(|(_, v)| format!("; what broke: {v}")).unwrap_or_default();
            format!("failed (listed: its card shows the game's numbers where they differ{t})")
        };
        rows.retain(|r| r.file != file);
        rows.push(row);
        match vars.iter_mut().find(|v| v.file == file) {
            Some(v) => {
                if let Some(n) = &a.name {
                    v.name = n.clone();
                }
                // (a held one stays held: `held` is a bar's or a takedown's, ironspec's call; its
                // row is refreshed all the same)
                if !v.held.is_empty() {
                    eprintln!("{file}: still held off the site ({}); clear its `held` in variations.tsv to list it", v.held);
                }
            }
            None => vars.push(Variation { file: file.clone(), name: a.name.clone().unwrap_or_default(), ..Default::default() }),
        }
        // the stage's pick (`--pick`: `yes`), or its pick for one value of the stage's fork
        // (`--pick-for redstone`: day 1 keeps one pick per redstone fork value, ironspec rev 9
        // iteration 19): it takes that value from any other variation, the other picks stay
        let pick = pick_value(a)?;
        if let Some(p) = &pick {
            apply_pick(&mut vars, &file, p);
        }
        let role = match pick.as_deref() {
            Some("yes") => "the stage's pick".to_string(),
            Some(p) => format!("the stage's pick for {p}"),
            None => "a variation".to_string(),
        };
        println!("{}: {shown}; the real game: {gate_word} ({role})", dir.join(&file).display());
    }
    write_verdicts(&dir, &rows)?;
    write_variations(&dir, &vars)?;
    Ok(())
}

/// The pick `site variation` sets: `yes` for `--pick`, the fork value of `--pick-for` (a word:
/// `redstone`), None for neither.
fn pick_value(a: &crate::VariationArgs) -> Result<Option<String>> {
    match (&a.pick_for, a.pick) {
        (Some(_), true) => bail!("--pick and --pick-for: one or the other (--pick-for <value> is the pick for that fork value)"),
        (Some(v), false) => {
            let v = v.trim().to_lowercase();
            if v.is_empty() || v == "yes" || v == "no" || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                bail!("--pick-for {v:?}: a fork value is one word (letters, digits, '-'), e.g. --pick-for redstone");
            }
            Ok(Some(v))
        }
        (None, true) => Ok(Some("yes".into())),
        (None, false) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_and_kinds() {
        assert_eq!(stage_kind("mob-farm iron_ingot", "middle"), "mob-farm iron_ingot middle");
        assert_eq!(stage_kind("mob-farm iron_ingot", "late"), "mob-farm iron_ingot fastest");
        assert_eq!(stage_kind("mob-farm iron_ingot day-1", "middle"), "mob-farm iron_ingot day-1");
        let b = Build { circuit: Some(site_model::Circuit { study: "mobfarm".into(), kind: "mob-farm iron_ingot fastest".into(), ..Default::default() }), ..Default::default() };
        assert_eq!(stage_of(&b), Some(2));
        assert!(site_model::is_farm(&b));
    }

    #[test]
    fn variations_and_members_read_by_their_headers() {
        let rows = tsv_rows("file\tpick\tname\nm2-open.txt\tyes\tthe open drop\nd1.txt\t\t\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("pick").map(String::as_str), Some("yes"));
        let dir = std::env::temp_dir().join(format!("mctas-farms-test-{}", std::process::id()));
        let part = dir.join("designs/farms/iron/parts/kill");
        std::fs::create_dir_all(&part).unwrap();
        std::fs::write(part.join("members.tsv"), "file\tname\tstage\tmetrics\tused_by\nblade.txt\tlava blade after a 22-block drop, fire-safe\tmiddle\tlava buckets 4, drop 22 blocks, blocks 38\tm2-open d4\nmissing.txt\tx\tday-1\t\t\n").unwrap();
        std::fs::write(part.join("blade.txt"), "name: blade\n").unwrap();
        let m = read_members(&dir, "iron", "kill");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(m.len(), 1, "a member whose design is missing is left out");
        assert_eq!(m[0].metrics[1], ("drop".to_string(), "22 blocks".to_string()));
        assert_eq!(m[0].used_by, vec!["m2-open".to_string(), "d4".to_string()]);
    }

    #[test]
    fn a_farm_s_parts_and_its_members_numbers() {
        // (the parent 2026-11-08 20:59: every farm page lists its parts with their numbers; the
        // optimizer's parts file, its other names for two parts: calm, pad)
        let p = read_farm_parts("# travel by share of placed blocks\npart\tmember\tblocks\tdug\tbuild_seconds\ncage\tbed-plate-2-watching.txt\t75\t40\t1122\ncalm\tbed-plate\t35\t10\t193\npad\t\t80\t60\t502.5\n").unwrap();
        assert_eq!(p.iter().map(|p| p.part.as_str()).collect::<Vec<_>>(), ["cage", "sleep-trigger", "spawn-area"]);
        assert_eq!(p[0].member, "bed-plate-2-watching");
        assert_eq!((p[2].blocks, p[2].dug, p[2].build_seconds), (80, 60, 502.5));
        assert!(read_farm_parts("part\tmember\tblocks\tdug\tbuild_seconds\nmoat\t\t1\t0\t1\n").unwrap_err().to_string().contains("unknown part"));
        assert_eq!((build_time(40.0), build_time(193.0), build_time(1122.0), build_time(3930.0)), ("40 s".into(), "3 min".into(), "19 min".into(), "1 h 06 min".into()));
        // the optimizer's member metrics, keys of several words
        let m = parse_member_metrics("villagers 3, watching villagers 2, blocks 75, build seconds 1122, spawn success per check 88%, in sight of the villagers no, drop 22 blocks, hide time while the bed villager sleeps, storage hours at 480 iron/h 3.2");
        let get = |k: &str| m.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str());
        assert_eq!((get("villagers"), get("watching villagers"), get("blocks"), get("build seconds")), (Some("3"), Some("2"), Some("75"), Some("1122")));
        assert_eq!((get("spawn success per check"), get("in sight of the villagers"), get("drop")), (Some("88%"), Some("no"), Some("22 blocks")));
        assert_eq!((get("hide time"), get("storage hours at 480 iron/h")), (Some("while the bed villager sleeps"), Some("3.2")));
        // the card's build line: what the whole build takes where it stands
        let b = Build {
            circuit: Some(site_model::Circuit {
                metrics: crate::circuits::parse_metrics("build time 39 min at day-1 prices (439 placed with 309 cobblestone slabs capping stray golem spots and 28 cobblestone seals, 502 dug, 1 lava trips, 3 villagers moved), build minutes 39.0"),
                ..Default::default()
            }),
            ..Default::default()
        };
        let c = card_build(&b).unwrap();
        assert_eq!((c.placed, c.dug, c.minutes), (439, 502, Some(39.0)));
        assert_eq!(c.extras, "309 cobblestone slabs capping stray golem spots and 28 cobblestone seals");
    }

    #[test]
    fn a_failed_gate_puts_the_game_s_numbers_and_breaks_on_the_card() {
        // (ironspec rev 9 iteration 14: a failed gate is no refusal; a rate outside the card's
        // interval puts the game's numbers on the card; what broke is a counted tradeoff line)
        use circuit::mobfarm_game::{BuildRun, GateReport, WindowCheck};
        let mut builds: Vec<BuildRun> = (0..8).map(|_| BuildRun { deaths: 100, iron_chest: 400, ..Default::default() }).collect();
        builds[3].failures.push("gt 5000: a golem spawned by gt 3000 is still alive at 1.5 2.0 3.5 (not killed within 1200 gt)".into());
        builds[3].failures.push("gt 1000: the AFK bot is no longer on the server (a harness failure, not the farm's: the chunks around the farm unload)".into());
        let g = GateReport {
            builds,
            windows: vec![
                WindowCheck { name: "3 days of natural weather".into(), game: 132.0, sim: 110.0, interval: (100.0, 120.0), z: 2.0 },
                WindowCheck { name: "a forced day of rain".into(), game: 111.0, sim: 110.0, interval: (100.0, 120.0), z: 2.6 },
            ],
            failures: vec!["3 days of natural weather: outside".into()],
            ..Default::default()
        };
        let m = game_findings("iron per hour per block 1.19, iron per hour 440, iron range 95% 436-444 (64 runs), golems per hour 110.0, seconds per golem 32.7, share of ceiling 90%, rain clear day 445 iron/h; rain day 441 iron/h; thunder day 417 iron/h", &g, 370);
        let f = crate::circuits::parse_metrics(&m);
        let get = |k: &str| f.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());
        assert_eq!(get("golems per hour").as_deref(), Some("132.0"), "{m}");
        assert_eq!(get("iron per hour").as_deref(), Some("528"), "the game's golems times its 4 iron each: {m}");
        assert_eq!(get("iron per hour per block").as_deref(), Some("1.43"), "{m}");
        assert!(get("iron range").is_none() && get("share of ceiling").is_none() && get("real game").is_none(), "{m}");
        assert_eq!(get("rain").as_deref(), Some("clear day 445 iron/h; rain day 441 iron/h; thunder day 417 iron/h"), "a window inside its interval keeps the card's: {m}");
        let t = get("game tradeoffs").unwrap_or_default();
        assert!(t.starts_with("a golem stays alive past") && t.ends_with("in 1 of 8 builds") && !t.contains("AFK"), "{t}");
    }

    #[test]
    fn a_pick_per_fork_value_and_the_redstone_a_farm_takes() {
        // (ironspec rev 9 iteration 19: day 1 keeps one pick per redstone fork value; each card
        // says how much redstone its farm needs)
        let dir = std::env::temp_dir().join(format!("mctas-farms-picks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VARIATIONS), "file\tpick\tname\theld\nd1u-deep.txt\tyes\tdeep\t\ndust.txt\tRedstone\tdust\t\nside.txt\tno\tside\t\n").unwrap();
        let v = read_variations(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(v.iter().map(|v| v.pick.as_str()).collect::<Vec<_>>(), ["yes", "redstone", ""]);
        assert!(v[0].is_pick() && v[1].is_pick() && !v[2].is_pick());
        // the picks first (`yes`, then by fork value), then the most iron an hour
        let b = |name: &str, flags: &[&str], rate: u32| Build {
            name: name.into(),
            flags: flags.iter().map(|f| f.to_string()).collect(),
            circuit: Some(site_model::Circuit { metrics: vec![("iron per hour".into(), rate.to_string())], ..Default::default() }),
            ..Default::default()
        };
        let mut farms = [b("other", &["variation"], 400), b("dust", &["variation", "pick", "pick-for-redstone"], 367), b("deep", &["variation", "pick"], 320)];
        farms.sort_by_key(stage_key);
        assert_eq!(farms.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["deep", "dust", "other"]);
        // a repeater 3 (2 torches, a dust), a comparator 3, an observer 2, dust 1, 7 rails 2 (6 a dust)
        let t = "name: t\npalette:\n  R minecraft:repeater[facing=north]\n  C minecraft:comparator[facing=north]\n  O minecraft:observer[facing=north]\n  W minecraft:redstone_wire\n  S minecraft:stone\n  P minecraft:powered_rail[shape=north_south]\nlayer 0:\n  RCOWS\nlayer 1:\n  PPPPPPP\n";
        assert_eq!(redstone_dust(t), Some(3 + 3 + 2 + 1 + 2));
        assert_eq!(redstone_dust("name: t\npalette:\n  S minecraft:stone\nlayer 0:\n  S\n"), Some(0));
    }

    #[test]
    fn a_pick_leaves_the_other_fork_s_pick_and_unknown_columns_alone() {
        // (the day-1 lane 2026-11-08: a `--pick` cleared the redstone pick; the file as on main,
        // with a column this tool does not know)
        let dir = std::env::temp_dir().join(format!("mctas-farms-pick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VARIATIONS), "file\tpick\tname\theld\tnote\nd1u-underground.txt\t\tUnderground farm\t\tkeep me\nd1u-deep.txt\tyes\tDeep pit\t\t\ndust-pulse-out-of-sight.txt\tredstone\tDust pulse\t\t\nd1-bed-plate.txt\t\t\tnot presented\t\n").unwrap();
        let mut v = read_variations(&dir);
        // `--pick` on another design: only the `yes` row moves
        apply_pick(&mut v, "d1u-underground.txt", "yes");
        write_variations(&dir, &v).unwrap();
        let back = read_variations(&dir);
        let text = std::fs::read_to_string(dir.join(VARIATIONS)).unwrap();
        // `--pick-for redstone` on the deep farm: only the redstone row moves
        let mut w = back.clone();
        apply_pick(&mut w, "d1u-deep.txt", "redstone");
        let _ = std::fs::remove_dir_all(&dir);
        let picks = |v: &[Variation]| -> Vec<(String, String)> { v.iter().map(|v| (v.file.clone(), v.pick.clone())).collect() };
        let pair = |f: &str, p: &str| (f.to_string(), p.to_string());
        assert_eq!(picks(&back), [pair("d1u-underground.txt", "yes"), pair("d1u-deep.txt", ""), pair("dust-pulse-out-of-sight.txt", "redstone"), pair("d1-bed-plate.txt", "")]);
        assert_eq!(picks(&w)[1..3], [pair("d1u-deep.txt", "redstone"), pair("dust-pulse-out-of-sight.txt", "")]);
        assert_eq!(w[0].pick, "yes", "the yes pick stays");
        assert!(text.starts_with("file\tpick\tname\theld\tnote\n") && text.contains("\tkeep me\n"), "{text}");
        assert_eq!(back[3].held, "not presented");
    }

    #[test]
    fn a_held_variation_keeps_its_reason() {
        // (site variation --regate: a failed real-game gate holds it off the site; an old file
        // without the `held` column reads as on the site)
        let dir = std::env::temp_dir().join(format!("mctas-farms-held-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VARIATIONS), "file\tpick\tname\nold.txt\tyes\tthe old one\n").unwrap();
        assert_eq!(read_variations(&dir)[0].held, "");
        let rows = vec![Variation { file: "d1.txt".into(), name: "bed plate".into(), held: "the real game: build 3: golems walk off the pad".into(), ..Default::default() }];
        write_variations(&dir, &rows).unwrap();
        let back = read_variations(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back, rows);
    }
}
