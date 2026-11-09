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
pub const PARTS: [(&str, &str, &str); 6] = [
    ("module", "Farm module", "One whole farm, the unit bigger farms are copies of: its iron an hour, its blocks and its build time."),
    ("cage", "Villager cage", "Where the villagers sleep and watch the zombie: how many villagers, and how many of them watch."),
    ("spawn-area", "Golem spawn area", "The floor where the golems appear: its size, and how often a golem finds a spot on it."),
    ("kill", "Kill chamber", "Where the golems die: the lava, the drop, and whether the villagers see it."),
    ("collection", "Collection", "How the iron reaches your chest, and how long the chest holds it."),
    ("sleep-trigger", "Sleep trigger", "What hides the zombie at night, so the villagers can sleep."),
];

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
    /// The stage's current pick (listed first).
    pub pick: bool,
    /// A player-facing name (else the design's `how:` line).
    pub name: String,
    /// Why it is held off the site (a failed real-game gate: `site variation --regate`); empty: on
    /// the site.
    pub held: String,
}

const VARIATIONS: &str = "variations.tsv";
const VARIATIONS_HEADER: &str = "file\tpick\tname\theld";

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
    tsv_rows(&text)
        .into_iter()
        .filter_map(|r| {
            let file = r.get("file").cloned().filter(|f| !f.is_empty())?;
            Some(Variation { file, pick: r.get("pick").is_some_and(|p| p == "yes"), name: r.get("name").cloned().unwrap_or_default(), held: r.get("held").cloned().unwrap_or_default() })
        })
        .collect()
}

fn write_variations(dir: &Path, rows: &[Variation]) -> Result<()> {
    let mut out = format!("{VARIATIONS_HEADER}\n");
    for r in rows {
        out.push_str(&format!("{}\t{}\t{}\t{}\n", r.file, if r.pick { "yes" } else { "" }, r.name.replace(['\t', '\n'], " "), r.held.replace(['\t', '\n'], " ")));
    }
    std::fs::write(dir.join(VARIATIONS), out)?;
    Ok(())
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
            let (src, from_stage) = if dir.join(&file).exists() {
                (format!("{rel}/{file}"), None)
            } else {
                let s = staged.filter(|s| repo.join(s).join(&file).exists())?;
                (format!("{s}/{file}"), Some(s))
            };
            let mut metrics = crate::circuits::parse_metrics(r.get("metrics").map(String::as_str).unwrap_or(""));
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
            let terrain = std::fs::read_to_string(repo.join(&src)).ok().and_then(|t| crate::pages::terrain_of(&t)).map(String::from);
            Some(Member {
                part: part.to_string(),
                name: r.get("name").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| file.trim_end_matches(".txt").to_string()),
                stage,
                metrics,
                used_by: r.get("used_by").map(|u| u.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                src,
                kind: row.map(|r| r.kind).unwrap_or_default(),
                terrain,
                file,
            })
        })
        .collect()
}

/// The site path of a member's design file (the same in the repository).
pub fn member_design(_farm: &str, m: &Member) -> String {
    m.src.clone()
}

/// A member's design as the site serves it: its working comments out (the parent 2026-11-08: no
/// internal notes in a published design), a header naming it and its page.
pub fn member_text(farm: &str, m: &Member, text: &str) -> String {
    let title = PARTS.iter().find(|p| p.0 == m.part).map(|p| p.1).unwrap_or("Part");
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
    let title = PARTS.iter().find(|p| p.0 == m.part).map(|p| p.1).unwrap_or("");
    format!(
        "<div class=\"viewer k-circuit\" data-mcview=\"full\" data-src=\"{}\"{a} data-kind=\"circuit\"><div class=\"fallback\"><span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>",
        esc(&format!("{prefix}{}", m.src)),
        esc(&m.name),
        esc(title)
    )
}

/// The site path of a component's page.
pub fn part_page_path(farm: &str, part: &str) -> String {
    format!("farms/{farm}/parts/{part}/index.html")
}

static PARTS_LOADED: std::sync::RwLock<Vec<Member>> = std::sync::RwLock::new(Vec::new());

/// Load every iron-farm component's members (the build pages link a farm to its components).
pub fn load_parts(repo: &Path) -> Vec<Member> {
    let all: Vec<Member> = PARTS.iter().flat_map(|(p, _, _)| read_members(repo, "iron", p)).collect();
    if let Ok(mut g) = PARTS_LOADED.write() {
        *g = all.clone();
    }
    all
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

/// A full farm page's "Its parts" section: the components it is built from, each linking to its
/// member on the component's page (empty when it names none).
pub fn parts_section(b: &Build, prefix: &str) -> String {
    let farm = site_model::build_farm_folder(b).unwrap_or_default();
    let used = parts_of(&b.name);
    if used.is_empty() || farm.is_empty() {
        return String::new();
    }
    let items: String = PARTS
        .iter()
        .flat_map(|(p, title, _)| used.iter().filter(move |m| m.part == *p).map(move |m| (title, m)))
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

/// The order of an iron farm stage's farms: the stage's pick first (a variation picked in its
/// variations.tsv, else the archived design, the one live before the variations), then the most
/// iron an hour.
fn stage_key(b: &Build) -> (bool, bool, i64) {
    (!b.flags.iter().any(|f| f == "pick"), is_variation(b), -(rate(b) * 1000.0) as i64)
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
        // (the stage's pick first: a variation picked in its variations.tsv, else the archived
        // design, the one live before the variations; then the most iron an hour)
        of.sort_by_key(|b| stage_key(b));
        let cards: Vec<String> = of.iter().map(|b| build_card(b, prefix)).collect();
        stages.push_str(&format!(
            "<section class=\"stage\"><h3>{}</h3><p class=\"hint\">{}</p>\n{}</section>\n",
            esc(title),
            esc(line),
            pages::grid(&cards)
        ));
    }
    let mut comps: Vec<String> = Vec::new();
    for (p, title, what) in PARTS {
        let members: Vec<&Member> = parts.iter().filter(|m| m.part == p).collect();
        let Some(best) = members.first() else { continue };
        comps.push(format!(
            "<a class=\"card study\" href=\"{prefix}{}\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{} {}</div></div></a>",
            esc(&part_page_path("iron", p)),
            esc(title),
            site_model::html::thumb(prefix, &member_design("iron", best), None, None, None, "circuit", "", ""),
            esc(title),
            esc(what),
            if members.len() == 1 { "1 version.".to_string() } else { format!("{} versions.", members.len()) }
        ));
    }
    let components = if comps.is_empty() {
        String::new()
    } else {
        format!("<h2>Components</h2>\n<p class=\"hint\">The pieces an iron farm is made of, each with the best versions to build into your own farm.</p>\n{}", pages::grid(&comps))
    };
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
    let (title, what) = PARTS.iter().find(|p| p.0 == part).map(|p| (p.1, p.2)).unwrap_or((part, ""));
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}farms/index.html\">Farms</a> › <a href=\"{prefix}farms/{farm}/index.html\">{}</a> › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}</p>\n",
        esc(&farm_title(farm)),
        esc(title),
        esc(title),
        esc(what)
    );
    for m in members {
        let design = member_design(farm, m);
        let rows: String = m.metrics.iter().map(|(k, v)| format!("<tr><th scope=\"row\">{}</th><td>{}</td></tr>", esc(k), esc(&crate::explain::group_numbers(v)))).collect();
        // (a farm module reads as a component: no stage line, ironspec 2026-11-08)
        let stage = if m.part == "module" { String::new() } else { STAGES.iter().find(|s| s.0 == m.stage).map(|s| format!("<tr><th scope=\"row\">stage</th><td>{}</td></tr>", esc(s.1))).unwrap_or_default() };
        let used: Vec<String> = m
            .used_by
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
            // (a heading starts with a capital, whatever members.tsv says: "Bed-plate cage, ...")
            esc(&{
                let mut ch = m.name.chars();
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
    let j = crate::preview::judge(text);
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
            None => vars.push(Variation { file: file.clone(), pick: false, name: a.name.clone().unwrap_or_default(), held: String::new() }),
        }
        if a.pick {
            for v in vars.iter_mut() {
                v.pick = v.file == file;
            }
        }
        println!("{}: {shown}; the real game: {gate_word} ({})", dir.join(&file).display(), if a.pick { "the stage's pick" } else { "a variation" });
    }
    write_verdicts(&dir, &rows)?;
    write_variations(&dir, &vars)?;
    Ok(())
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
    fn a_held_variation_keeps_its_reason() {
        // (site variation --regate: a failed real-game gate holds it off the site; an old file
        // without the `held` column reads as on the site)
        let dir = std::env::temp_dir().join(format!("mctas-farms-held-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VARIATIONS), "file\tpick\tname\nold.txt\tyes\tthe old one\n").unwrap();
        assert_eq!(read_variations(&dir)[0].held, "");
        let rows = vec![Variation { file: "d1.txt".into(), pick: false, name: "bed plate".into(), held: "the real game: build 3: golems walk off the pad".into() }];
        write_variations(&dir, &rows).unwrap();
        let back = read_variations(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back, rows);
    }
}
