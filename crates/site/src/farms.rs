//! The Farms section (vjeux 2026-11-08 12:14: "a new farms top level category, and you can drill
//! down with iron farm and then full farm / individual components"; 12:15: every promising
//! variation of a stage on the site, the stage's pick first, its numbers the numbers).
//!
//! - `farms/index.html`: the farm types (the iron farm, the crop farms), the Farms tab's page.
//! - `farms/iron/index.html`: the iron farm's hub: its full farms by stage (Day 1, Middle, Late),
//!   every variation of each stage with its pick first, then its components.
//! - A full farm's page, `farms/iron/<stem>.html`: an archived design's or a variation's, the same
//!   page as any design's; the crop farms' collection is `farms/crops/index.html`.
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
pub const PARTS: [(&str, &str, &str); 5] = [
    ("cage", "Villager cage", "Where the villagers sleep and watch the zombie: how many villagers, and how many of them watch."),
    ("spawn-area", "Golem spawn area", "The floor where the golems appear: its size, and how often a golem finds a spot on it."),
    ("kill", "Kill chamber", "Where the golems die: the lava, the drop, and whether the villagers see it."),
    ("collection", "Collection", "How the iron reaches your chest, and how long the chest holds it."),
    ("sleep-trigger", "Sleep trigger", "What hides the zombie at night, so the villagers can sleep."),
];

/// The title of a farm folder (`iron` -> `Iron farm`).
pub fn farm_title(folder: &str) -> &'static str {
    match folder {
        "iron" => "Iron farm",
        "crops" => "Crop farms",
        _ => "Farms",
    }
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

/// A farm study's variation folders that have designs: (stage, folder).
pub fn variation_dirs(circuits_root: &Path, study: &str) -> Vec<(String, PathBuf)> {
    let Some(farm) = site_model::farm_folder(study) else { return Vec::new() };
    let base = farms_root(circuits_root).join(farm);
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
}

const VARIATIONS: &str = "variations.tsv";
const VARIATIONS_HEADER: &str = "file\tpick\tname";

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
            Some(Variation { file, pick: r.get("pick").is_some_and(|p| p == "yes"), name: r.get("name").cloned().unwrap_or_default() })
        })
        .collect()
}

fn write_variations(dir: &Path, rows: &[Variation]) -> Result<()> {
    let mut out = format!("{VARIATIONS_HEADER}\n");
    for r in rows {
        out.push_str(&format!("{}\t{}\t{}\n", r.file, if r.pick { "yes" } else { "" }, r.name.replace(['\t', '\n'], " ")));
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
}

impl Member {
    pub fn stem(&self) -> &str {
        self.file.trim_end_matches(".txt")
    }
}

/// A component folder's members, best first (none without its `members.tsv`).
pub fn read_members(repo: &Path, farm: &str, part: &str) -> Vec<Member> {
    let dir = repo.join("designs/farms").join(farm).join("parts").join(part);
    let text = std::fs::read_to_string(dir.join("members.tsv")).unwrap_or_default();
    tsv_rows(&text)
        .into_iter()
        .filter_map(|r| {
            let file = r.get("file").cloned().filter(|f| !f.is_empty())?;
            let file = if file.ends_with(".txt") { file } else { format!("{file}.txt") };
            dir.join(&file).exists().then_some(())?;
            Some(Member {
                part: part.to_string(),
                name: r.get("name").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| file.trim_end_matches(".txt").to_string()),
                stage: r.get("stage").cloned().unwrap_or_default(),
                metrics: crate::circuits::parse_metrics(r.get("metrics").map(String::as_str).unwrap_or("")),
                used_by: r.get("used_by").map(|u| u.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                file,
            })
        })
        .collect()
}

/// The site path of a member's design file.
pub fn member_design(farm: &str, m: &Member) -> String {
    format!("designs/farms/{farm}/parts/{}/{}", m.part, m.file)
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
    let farm = b.circuit.as_ref().and_then(|c| site_model::farm_folder(&c.study)).unwrap_or("");
    let used = parts_of(&b.name);
    if used.is_empty() || farm.is_empty() {
        return String::new();
    }
    let items: String = PARTS
        .iter()
        .flat_map(|(p, title, _)| used.iter().filter(move |m| m.part == *p).map(move |m| (title, m)))
        .map(|(title, m)| format!("<li><b>{}</b>: <a href=\"{prefix}{}#{}\">{}</a></li>", esc(title), esc(&part_page_path(farm, &m.part)), esc(m.stem()), esc(&m.name)))
        .collect();
    format!("<section><h2>What it is made of</h2><ul class=\"parts\">{items}</ul></section>\n")
}

/// The builds of a farm study, the variations included.
fn farm_builds<'a>(site: &'a Site, study: &str) -> Vec<&'a Build> {
    site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.study == study)).collect()
}

/// A farm build's iron an hour (for the order under its stage), if measured.
fn rate(b: &Build) -> f64 {
    b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "iron per hour")).and_then(|(_, v)| v.split_whitespace().next()?.parse().ok()).unwrap_or(0.0)
}

/// `farms/index.html`: the farm types, each a tile with a picture of one of its farms.
pub fn farms_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let mut tiles: Vec<String> = Vec::new();
    for (study, folder) in site_model::FARM_STUDIES {
        let list = farm_builds(site, study);
        // (the picture: the iron farm's first stage pick, a crop farm's smallest full design)
        let Some(first) = list.iter().find(|b| b.flags.iter().any(|f| f == "pick")).or_else(|| list.iter().find(|b| !is_variation(b))).or(list.first()) else { continue };
        tiles.push(format!(
            "<a class=\"card study\" href=\"{prefix}farms/{folder}/index.html\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{}</div></div></a>",
            esc(farm_title(folder)),
            site_model::html::thumb(prefix, site_model::html::card_src(first), first.trace.as_deref(), first.still.as_deref(), first.picture.as_deref(), &first.kind, "", ""),
            esc(farm_title(folder)),
            esc(crate::explain::study_blurb(study))
        ));
    }
    let body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Farms</p>\n<h1>Farms</h1>\n<p class=\"lede\">Farms make resources by themselves: build one, stay near it and empty its chests. Each page says what it makes, what it costs and how to build it, and lets you watch it work in 3D.</p>\n{}",
        pages::grid(&tiles)
    );
    pages::shell_full("Farms", "Redstone farms you can download: iron farms for every stage of the game, and crop farms.", prefix, &body, scripts, site, None, Some("farm"))
}

/// `farms/iron/index.html`: the iron farm's full farms by stage, every variation, each stage's pick
/// first; then its components.
pub fn iron_hub(site: &Site, parts: &[Member], scripts: Scripts) -> String {
    let prefix = "../../";
    let list = farm_builds(site, "mobfarm");
    let mut stages = String::new();
    for (k, (_, title, _, line)) in STAGES.iter().enumerate() {
        let mut of: Vec<&&Build> = list.iter().filter(|b| stage_of(b) == Some(k)).collect();
        if of.is_empty() {
            continue;
        }
        // (the stage's pick first: a variation picked in its variations.tsv, else the archived
        // design, the one live before the variations; then the most iron an hour)
        let key = |b: &Build| (!b.flags.iter().any(|f| f == "pick"), is_variation(b), -rate(b));
        of.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal));
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
        esc(farm_title(farm)),
        esc(title),
        esc(title),
        esc(what)
    );
    for m in members {
        let design = member_design(farm, m);
        let rows: String = m.metrics.iter().map(|(k, v)| format!("<tr><th scope=\"row\">{}</th><td>{}</td></tr>", esc(k), esc(v))).collect();
        let stage = STAGES.iter().find(|s| s.0 == m.stage).map(|s| format!("<tr><th scope=\"row\">stage</th><td>{}</td></tr>", esc(s.1))).unwrap_or_default();
        let used: Vec<String> = m
            .used_by
            .iter()
            .filter_map(|u| site.builds.iter().find(|b| &b.name == u && site_model::is_farm(b)))
            .map(|b| format!("<a href=\"{prefix}{}\">{}</a>", esc(&b.page), esc(&pages::page_title(b))))
            .collect();
        body.push_str(&format!(
            "<section class=\"member\" id=\"{}\"><h2>{}</h2><div class=\"grid\">{}</div><table class=\"props\">{stage}{rows}</table>{}<p><a href=\"{prefix}{}\" download>Download it as text</a></p></section>\n",
            esc(m.stem()),
            esc(&m.name),
            site_model::html::thumb(prefix, &design, None, None, None, "circuit", "", ""),
            if used.is_empty() { String::new() } else { format!("<p>Built into: {}</p>", used.join(", ")) },
            esc(&design)
        ));
    }
    pages::shell_full(title, what, prefix, &body, scripts, site, None, Some("farm"))
}

/// `mctas site variation`: a design joins its farm stage's variations (judged on redsim first: its
/// numbers on the site are its judge's), or a variation's row changes (`--pick`, `--name`).
pub fn variation(a: &crate::VariationArgs) -> Result<()> {
    let repo = crate::published::repo_root();
    if !STAGES.iter().any(|s| s.0 == a.stage) {
        bail!("--stage {}: one of {}", a.stage, STAGES.iter().map(|s| s.0).collect::<Vec<_>>().join(", "));
    }
    let study = site_model::FARM_STUDIES.iter().find(|(_, f)| *f == a.farm).map(|(s, _)| *s).with_context(|| format!("--farm {}: no such farm", a.farm))?;
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
        let j = crate::preview::judge(&text);
        if !j.pass {
            bail!("{}: its judge does not pass it on redsim: {}", path.display(), if j.why.is_empty() { j.summary.clone() } else { j.why.clone() });
        }
        // the design with its judge's line on top, as the archive writes it
        let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.join(&file), format!("# {}\n{body}", j.summary.trim_start_matches("# ")))?;
        let (lo, hi) = c.bounds();
        let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let kind_name = if j.kind.is_empty() { kind.name() } else { j.kind.clone() };
        let row = circuit::archive::Row {
            archived: clock::catalog::now_utc(),
            file: file.clone(),
            kind: stage_kind(&kind_name, &a.stage),
            axis: j.axis.clone(),
            metrics: j.metrics.clone(),
            blocks: c.blocks(),
            dims: if j.dims.is_empty() { format!("{}x{}x{}", d[0], d[1], d[2]) } else { j.dims.clone() },
            volume: if j.volume > 0 { j.volume } else { (d[0] * d[1] * d[2]) as i64 },
            family: j.family.clone(),
            robustness: j.robustness.clone(),
            kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
            type_key: circuit::archive::type_key(&c),
            hash: String::new(),
            summary: j.summary.clone(),
        };
        rows.retain(|r| r.file != file);
        rows.push(row);
        match vars.iter_mut().find(|v| v.file == file) {
            Some(v) => {
                if let Some(n) = &a.name {
                    v.name = n.clone();
                }
            }
            None => vars.push(Variation { file: file.clone(), pick: false, name: a.name.clone().unwrap_or_default() }),
        }
        if a.pick {
            for v in vars.iter_mut() {
                v.pick = v.file == file;
            }
        }
        println!("{}: {} ({})", dir.join(&file).display(), j.metrics.split(", ").take(3).collect::<Vec<_>>().join(", "), if a.pick { "the stage's pick" } else { "a variation" });
    }
    let mut out = format!("{}\n", crate::preview::VERDICTS_HEADER);
    for r in &rows {
        out.push_str(&crate::preview::verdicts_line(r));
        out.push('\n');
    }
    std::fs::write(dir.join(circuit::archive::VERDICTS), out)?;
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
}
