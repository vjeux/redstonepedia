//! Every page path the site ever published, so a design that leaves the site (rejudged out of the
//! archive, no longer its kind's pick, renamed) never leaves a dead link (the coordinator
//! 2026-10-21 11:05Z: vjeux hit circuits/shulker/shulker-loader-1wide-28b.html, a 404 since the
//! 28-block loader was rejudged out).
//!
//! [`FILE`] lists every page ever deployed with what it was (a circuit's kind, a clock's period, a
//! door's board, a family's card). `site build` writes a small redirect page at each listed path it
//! does not build, to the current page of the same kind (a meta refresh and a link, with a
//! one-line note saying why: the archive's rejudged/ entry when it has one), and carries the whole
//! list in the site ([`SITE_FILE`]). `site deploy` refuses to drop a live page and adds what it
//! published to the list; `site publish` first merges the live site's list (so a list the
//! publisher forgot to commit is never lost); `site smoke` checks every listed path resolves.
//! `site published --recover` rebuilt the list from every deploy GitHub Pages kept.

use crate::pages;
use crate::{deploy, kinds};
use anyhow::{Context, Result, bail};
use site_model::html::esc;
use site_model::{Build, Site};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The list, in the repository (append-only: git merges it by union, .gitattributes).
pub const FILE: &str = "crates/site/published.tsv";
/// The list as the deployed site carries it (every page it ever had, its own included).
pub const SITE_FILE: &str = "data/published.tsv";
/// Seconds a redirect page with a note waits before moving on (time to read one line).
const READ_SECONDS: u32 = 4;

/// One page path the site published.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// `circuits/shulker/shulker-loader-1wide-28b.html`.
    pub path: String,
    /// `circuit`, `clock`, `door`, `family`, `page`, or `deleted` (a page vjeux deleted: no
    /// redirect, the site's 404 page).
    pub what: String,
    /// What replaces it: a circuit's `study/kind` ([`kinds::kind_key`]), a clock's period in gt
    /// (as the Clocks tab groups them), a door's board, a family's card id; empty for a page.
    pub key: String,
    /// A parametric circuit kind's row (its size), else empty.
    pub row: String,
    /// What it was, in words (its page title: `Shulker loader: 28 blocks`).
    pub title: String,
    /// When it was first published (`2026-10-21 07:29Z`: the deploy, or the build that made it).
    pub first: String,
}

impl Entry {
    fn line(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        [&self.path, &self.what, &self.key, &self.row, &self.title, &self.first].iter().map(|s| clean(s)).collect::<Vec<_>>().join("\t")
    }
}

/// The entries of a list's text (comments and blank lines skipped; a path that could leave the
/// site's folder is dropped).
pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let path = f.first()?.trim();
            if path.is_empty() || path.starts_with('/') || path.split('/').any(|c| c == ".." || c.is_empty()) {
                return None;
            }
            let get = |i: usize| f.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
            Some(Entry { path: path.to_string(), what: get(1), key: get(2), row: get(3), title: get(4), first: get(5) })
        })
        .collect()
}

/// Merge `new` into `map`: a path keeps its earliest first-published time, and a field it lacks
/// is filled from the newer entry. Returns how many paths were new.
pub fn merge(map: &mut BTreeMap<String, Entry>, new: impl IntoIterator<Item = Entry>) -> usize {
    let mut added = 0;
    for e in new {
        match map.get_mut(&e.path) {
            None => {
                map.insert(e.path.clone(), e);
                added += 1;
            }
            Some(old) => {
                if old.first.is_empty() || (!e.first.is_empty() && e.first < old.first) {
                    old.first = e.first.clone();
                }
                // (what a page was: the first record that knows it)
                if old.what.is_empty() || (old.key.is_empty() && !e.key.is_empty() && old.what == e.what) {
                    old.what = e.what.clone();
                    old.key = e.key.clone();
                    old.row = e.row.clone();
                }
                if old.title.is_empty() {
                    old.title = e.title.clone();
                }
            }
        }
    }
    added
}

/// The list at `path` (none yet: empty).
pub fn read(path: &Path) -> BTreeMap<String, Entry> {
    let mut map = BTreeMap::new();
    merge(&mut map, parse(&std::fs::read_to_string(path).unwrap_or_default()));
    map
}

/// The list's text: a header, then one line per path, sorted.
pub fn render(map: &BTreeMap<String, Entry>) -> String {
    let mut s = String::from(
        "# Every page path REDSTONEPEDIA ever published (crates/site/src/published.rs). `site build` writes\n\
         # a redirect page at each one it does not build (to the current page of the same kind), `site\n\
         # deploy` adds what it publishes, `site smoke` checks each one resolves. Append-only: git merges\n\
         # it by union (.gitattributes). Columns: path, what (circuit, clock, door, family, page, deleted:\n\
         # a page vjeux deleted), key (a circuit's study/kind, a clock's period in gt, a door's board, a\n\
         # family's card), row (a parametric circuit's size), title (what it was), first (first published).\n",
    );
    for e in map.values() {
        s.push_str(&e.line());
        s.push('\n');
    }
    s
}

pub fn write_list(path: &Path, map: &BTreeMap<String, Entry>) -> Result<()> {
    std::fs::write(path, render(map)).with_context(|| format!("writing {}", path.display()))
}

/// What a build's page is (no first-published time).
pub fn of_build(b: &Build) -> Entry {
    let mut e = Entry { path: b.page.clone(), what: b.kind.clone(), title: pages::page_title(b), ..Default::default() };
    if let Some(c) = &b.circuit {
        let k = kinds::kind_key(b);
        if kinds::parametric(&k)
            && let Some((r, _)) = kinds::row_of(b)
        {
            e.row = r.to_string();
        }
        e.key = format!("{}/{k}", c.study);
        e.what = "circuit".into();
    } else if let Some(c) = &b.clock {
        e.key = c.every().to_string();
        e.what = "clock".into();
    } else if b.door.is_some() || b.kind == "door" {
        // its main board (`Solid (main ranking)`, `Sculk`, `Gravity`...): where its replacement is
        e.key = b.door.as_ref().and_then(|d| d.boards.first().cloned()).unwrap_or_else(|| b.group.clone());
        e.what = "door".into();
    }
    e
}

/// What a page path is when nothing else says it: its folder (a family page's card is its name).
fn of_path(path: &str) -> Entry {
    let stem = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".html").to_string();
    let mut e = Entry { path: path.to_string(), what: "page".into(), ..Default::default() };
    if crate::generate::DELETED_PAGES.contains(&path) {
        e.what = "deleted".into();
    } else if let Some(rest) = path.strip_prefix("families/") {
        if !rest.contains('/') && stem != "index" {
            e.what = "family".into();
            e.key = stem;
        }
    } else if let Some(rest) = path.strip_prefix("circuits/") {
        // circuits/<study>/<design>.html (a study's index is a page)
        if let Some((study, file)) = rest.split_once('/')
            && file != "index.html"
            && !file.contains('/')
            && study != "calibration"
        {
            e.what = "circuit".into();
            e.key = format!("{study}/");
        }
    } else if path.starts_with("clocks/") && stem != "index" {
        e.what = "clock".into();
    } else if path.starts_with("doors/") && stem != "index" {
        e.what = "door".into();
    }
    e
}

/// The pages `site build` writes for `site` (generate.rs): every build's and card's, the index,
/// the 404 page, the per-kind folders and the circuit studies, with what each is.
pub fn built_pages(site: &Site) -> Vec<Entry> {
    let mut v: Vec<Entry> = site.builds.iter().map(of_build).collect();
    v.extend(site.cards.iter().map(|c| Entry { path: c.page.clone(), what: "family".into(), key: c.id.clone(), title: c.title.clone(), ..Default::default() }));
    let mut studies: BTreeSet<String> = BTreeSet::new();
    for b in &site.builds {
        if let Some((sub, _)) = b.page.strip_prefix("circuits/").and_then(|r| r.split_once('/')) {
            studies.insert(sub.to_string());
        }
    }
    let fixed = ["index.html", "404.html", "doors/index.html", "clocks/index.html", "circuits/index.html"].into_iter().map(String::from);
    for p in fixed.chain(studies.iter().map(|s| format!("circuits/{s}/index.html"))) {
        v.push(Entry { path: p, what: "page".into(), ..Default::default() });
    }
    v
}

/// `git` in a repository (`dir`: a bare one; none: the current checkout), with GitHub's token when
/// it has one (a partial clone fetches the blobs it reads from GitHub).
pub struct Git {
    pub dir: Option<PathBuf>,
    pub header: Option<String>,
}

impl Git {
    pub fn here() -> Git {
        Git { dir: None, header: None }
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        let mut c = Command::new("git");
        if let Some(d) = &self.dir {
            c.arg("--git-dir").arg(d);
        }
        if let Some(h) = &self.header {
            c.args(["-c", h]);
        }
        let o = c.args(args).output().with_context(|| format!("git {}", args.join(" ")))?;
        if !o.status.success() {
            bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    }
}

/// The page paths of a deployed site (a gh-pages commit).
pub fn tree_pages(git: &Git, commit: &str) -> Result<Vec<String>> {
    Ok(git.run(&["ls-tree", "-r", "--name-only", commit])?.lines().filter(|p| p.ends_with(".html")).map(String::from).collect())
}

/// `2026-10-21 07:29Z` for Unix seconds.
pub fn fmt_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}Z", rem / 3600, rem % 3600 / 60)
}

/// What each page of a deployed site was: the site's own list when it carries one, else its data
/// (every build's page: data/site.json), else its path. `when`: its deploy's time (for pages its
/// list does not date).
pub fn of_tree(git: &Git, commit: &str, when: &str) -> Result<Vec<Entry>> {
    let paths = tree_pages(git, commit)?;
    let mut known: BTreeMap<String, Entry> = BTreeMap::new();
    if let Ok(text) = git.run(&["show", &format!("{commit}:{SITE_FILE}")]) {
        merge(&mut known, parse(&text));
    } else if let Ok(text) = git.run(&["show", &format!("{commit}:data/site.json")]) {
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        for b in v["builds"].as_array().into_iter().flatten() {
            // (an older data format: what the path says, below)
            if let Ok(b) = serde_json::from_value::<Build>(b.clone())
                && !b.page.is_empty()
            {
                known.insert(b.page.clone(), of_build(&b));
            }
        }
        for c in v["cards"].as_array().into_iter().flatten() {
            if let (Some(page), Some(id)) = (c["page"].as_str(), c["id"].as_str()) {
                let title = c["title"].as_str().unwrap_or("").to_string();
                known.insert(page.to_string(), Entry { path: page.to_string(), what: "family".into(), key: id.to_string(), title, ..Default::default() });
            }
        }
    }
    Ok(paths
        .iter()
        .map(|p| {
            let mut e = known.get(p).cloned().unwrap_or_else(|| of_path(p));
            if crate::generate::DELETED_PAGES.contains(&p.as_str()) {
                e.what = "deleted".into();
            }
            if e.first.is_empty() {
                e.first = when.to_string();
            }
            e
        })
        .collect())
}

/// Where a page that is no longer built now leads: a page of the site (a path from its root,
/// maybe with a `#fragment` for the index) and its title.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub path: String,
    pub title: String,
}

fn card_first<'a>(v: &[&'a Build]) -> Option<&'a Build> {
    v.iter().copied().min_by_key(|b| (!b.flags.iter().any(|f| f == "tunable"), b.flags.iter().any(|f| f == "generator"), b.blocks, b.name.clone()))
}

/// The current page for `e` in `site` (None: a page vjeux deleted, no redirect).
pub fn resolve(e: &Entry, site: &Site) -> Option<Target> {
    let to = |b: &Build| Target { path: b.page.clone(), title: pages::page_title(b) };
    let index = |frag: &str, title: &str| Target { path: if frag.is_empty() { "index.html".into() } else { format!("index.html#{frag}") }, title: title.to_string() };
    match e.what.as_str() {
        "deleted" => None,
        "circuit" => {
            let circuits: Vec<&Build> = site.builds.iter().filter(|b| b.circuit.is_some()).collect();
            let key_of = |b: &Build| format!("{}/{}", b.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or(""), kinds::kind_key(b));
            let (study, kind) = e.key.split_once('/').unwrap_or((e.key.as_str(), ""));
            // the same kind: the same size when it has one, else its card
            let same: Vec<&Build> = circuits.iter().copied().filter(|b| key_of(b) == e.key).collect();
            if !kind.is_empty() && !same.is_empty() {
                if !e.row.is_empty()
                    && let Some(b) = same.iter().find(|b| kinds::row_of(b).map(|r| r.0.to_string()) == Some(e.row.clone()))
                {
                    return Some(to(b));
                }
                return card_first(&same).map(to);
            }
            // the same kind of machine (`flying-machine` before the engine shapes), in its study,
            // then anywhere
            let base = kind.split_whitespace().next().unwrap_or("");
            let base_of = |b: &Build| kinds::kind_key(b).split_whitespace().next().unwrap_or("").to_string();
            if !base.is_empty() {
                let near: Vec<&Build> = circuits.iter().copied().filter(|b| base_of(b) == base && b.circuit.as_ref().is_some_and(|c| c.study == study)).collect();
                let far: Vec<&Build> = circuits.iter().copied().filter(|b| base_of(b) == base).collect();
                if let Some(b) = card_first(&near).or_else(|| card_first(&far)) {
                    return Some(to(b));
                }
            }
            // its study's page, else the circuits' overview
            if circuits.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.study == study)) {
                let title = crate::explain::studies().iter().find(|s| s.0 == study).map(|s| format!("the {} page", s.1)).unwrap_or_else(|| format!("the {study} page"));
                return Some(Target { path: format!("circuits/{study}/index.html"), title });
            }
            Some(Target { path: "circuits/index.html".into(), title: "the circuits page".into() })
        }
        "clock" => {
            let period: Option<u32> = e.key.parse().ok();
            let same: Vec<&Build> = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| Some(c.every()) == period)).collect();
            match same.iter().copied().min_by_key(|b| (crate::generate::is_member(b), b.blocks, b.name.clone())) {
                Some(b) => Some(to(b)),
                None => match period {
                    Some(p) => Some(index(&format!("kind=clock&time={p}"), &format!("the clocks nearest to {}", site_model::gt_time(p)))),
                    None => Some(index("kind=clock", "the clocks")),
                },
            }
        }
        "door" => {
            // the record of its main board (the door card first, then the door that leads that
            // board, then the smallest), else the door card
            let doors: Vec<&Build> = site.builds.iter().filter(|b| b.kind == "door").collect();
            let main = |b: &Build| b.door.as_ref().and_then(|d| d.boards.first().cloned()).unwrap_or_else(|| b.group.clone());
            let word = e.key.split([' ', ',']).next().unwrap_or("").to_lowercase();
            let leads = |b: &Build| b.door.as_ref().is_some_and(|d| d.leads.iter().any(|l| l.to_lowercase() == word));
            let is_card = |b: &Build| b.door.as_ref().is_some_and(|d| d.records > 0);
            let board = doors.iter().copied().filter(|b| !e.key.is_empty() && main(b) == e.key).min_by_key(|b| (!is_card(b), !leads(b), b.volume.unwrap_or(u32::MAX), b.blocks));
            // (a board no record leads now, `Sculk, not rotation-safe`: its material's record)
            let first_word = |s: &str| s.split([' ', ',']).next().unwrap_or("").to_lowercase();
            let material = doors.iter().copied().filter(|b| !word.is_empty() && first_word(&main(b)) == word).min_by_key(|b| (!is_card(b), !leads(b), b.volume.unwrap_or(u32::MAX), b.blocks));
            let card = doors.iter().copied().find(|b| is_card(b));
            match board.or(material).or(card) {
                Some(b) => Some(to(b)),
                None => Some(index("kind=door", "the doors")),
            }
        }
        "family" => {
            let base = site.builds.iter().filter(|b| b.clock.as_ref().and_then(|c| c.card.as_deref()) == Some(e.key.as_str()) || b.circuit.as_ref().is_some_and(|c| c.card == e.key)).min_by_key(|b| (crate::generate::is_member(b), b.blocks));
            match base {
                Some(b) => Some(to(b)),
                None => Some(index("kind=clock", "the clocks")),
            }
        }
        _ => {
            if e.path.starts_with("circuits/") {
                Some(Target { path: "circuits/index.html".into(), title: "the circuits page".into() })
            } else {
                Some(index("", "the index"))
            }
        }
    }
}

/// Why a circuit design left the archive, from its rejudged/ (or update-less/) entry: the first
/// line `mctas circuit archive --rejudge` writes (`# rejudged (<date>, ...): <why>`), in words.
pub fn rejudged_reason(repo: &Path, study: &str, stem: &str) -> Option<String> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut stack = vec![repo.join("designs/circuits").join(study).join("tracks")];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n.to_string_lossy() == format!("{stem}.txt")) && p.parent().and_then(|q| q.file_name()).is_some_and(|n| n == "rejudged" || n == "update-less") {
                found.push(p);
            }
        }
    }
    found.sort();
    let text = std::fs::read_to_string(found.first()?).ok()?;
    // (`# rejudged (<date>, ...): <why>`, or `# rejudged <date>: <why>`: the display archive's)
    let first = text.lines().find(|l| l.starts_with("# rejudged ") || l.starts_with("# update-less "))?;
    let why = first.split_once("): ").or_else(|| first.split_once(": "))?.1;
    Some(reason_words(why, stem))
}

/// A judge's failure line in words: the R7 note when there is one (`needs the judge's block
/// placement (...)`), else the failure after `FAIL: <name> <kind>: `, without the run's
/// bookkeeping (`rotation 0: `, `(the cap)`, coordinates); one line.
pub fn reason_words(why: &str, stem: &str) -> String {
    let (pre, fail) = match why.split_once("FAIL: ") {
        Some((p, f)) => (p.trim().trim_end_matches(':').trim(), f),
        None => ("", why),
    };
    let mut d = if !pre.is_empty() {
        pre.to_string()
    } else {
        let f = fail.strip_prefix(stem).map(str::trim_start).unwrap_or(fail);
        f.split_once(": ").map(|(_, rest)| rest).unwrap_or(f).to_string()
    };
    d = d.replace(" (the cap)", "").replace("R7: ", "");
    // (who asked for the rule is ours, not the visitor's: `..., vjeux 11:43Z)`)
    while let Some(i) = d.find(", vjeux ") {
        let rest = &d[i + ", vjeux ".len()..];
        let n = rest.find(|ch: char| !(ch.is_ascii_digit() || ch == ':' || ch == 'Z')).unwrap_or(rest.len());
        if n == 0 {
            break;
        }
        d.replace_range(i..i + ", vjeux ".len() + n, "");
    }
    while let Some(i) = d.find("rotation ") {
        let rest = &d[i + "rotation ".len()..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || !rest[digits..].starts_with(": ") {
            break;
        }
        d.replace_range(i..i + "rotation ".len() + digits + 2, "");
    }
    let d = crate::explain::plain_reason(&d);
    let d = d.trim().trim_end_matches('.').to_string();
    if d.chars().count() <= 200 {
        return d;
    }
    let cut: String = d.chars().take(200).collect();
    format!("{}…", cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut).trim_end_matches([',', ';', ':']))
}

/// The one-line note of a redirect page: why the page is gone and what it leads to (None: no
/// note, a plain move).
pub fn note(e: &Entry, target: &Target, repo: &Path) -> Option<String> {
    let stem = e.path.rsplit('/').next().unwrap_or("").trim_end_matches(".html");
    // a page by its title, quoted (`“Shulker box loader: 2.50 items a second · 31 blocks”`); the
    // index's views and the overviews by their words
    let to = if target.path.ends_with(".html") && !target.path.ends_with("index.html") { format!("“{}”", target.title) } else { target.title.clone() };
    match e.what.as_str() {
        "circuit" => {
            let study = e.path.strip_prefix("circuits/").and_then(|r| r.split_once('/')).map(|(s, _)| s).unwrap_or("");
            // (no design of its kind left, only an overview to send to: "replaced by the circuits"
            // said nothing; the 7-segment decoders, all rejudged out by the display-face rule)
            let overview = target.path.ends_with("index.html");
            if let Some(_why) = rejudged_reason(repo, study, stem) {
                // (why it failed is our retest's business, not the reader's: vjeux 2026-11-08 10:10)
                if overview {
                    return Some(format!("This design is no longer on the site, and no design of its kind is listed now: {to} lists the others."));
                }
                return Some(format!("This design was replaced by {to}."));
            }
            if overview {
                return Some(format!("This design is no longer in the archive, and no design of its kind is listed now: {to} lists the others."));
            }
            if repo.join("designs/circuits").join(study).join("archive").join(format!("{stem}.txt")).exists() {
                return Some(format!("This design is no longer listed: the site shows the best design of each kind, and {to} is the one for this kind now."));
            }
            Some(format!("This design is no longer in the archive: {to} is its kind's design now."))
        }
        "clock" if target.path.starts_with("clocks/") => Some(format!("This clock is no longer listed: {to} has the same period.")),
        "clock" => Some("No listed clock has this period any more: the Clocks tab shows the nearest ones.".to_string()),
        "door" => Some(format!("This door is no longer listed: the site shows each board's record, and {to} is the one for its board ({}) now.", e.key)),
        "family" => Some(format!("The family pages were removed: {to}, its base design, says every size it comes in.")),
        // a collection's page with no design left on the site (the 7-segment decoders')
        _ if e.path.starts_with("circuits/") && e.path.ends_with("/index.html") && e.path.matches('/').count() == 2 => {
            Some(format!("No design of this collection is on the site right now: {to} lists the others."))
        }
        _ => Some(format!("This page was removed: {to} has what it showed.")),
    }
}

/// A redirect page for `e` (a path the build does not write): small and standalone (no header,
/// no footer: its bytes change only with its target), a meta refresh to the target (after a few
/// seconds when a note says why), a link, the note; never indexed (its canonical URL is the
/// target's).
pub fn redirect_page(_site: &Site, e: &Entry, target: &Target, note: Option<&str>) -> String {
    let prefix = "../".repeat(e.path.matches('/').count());
    let url = format!("{prefix}{}", target.path);
    let delay = if note.is_some() { READ_SECONDS } else { 0 };
    let title = if e.title.is_empty() { "This page moved".to_string() } else { e.title.clone() };
    let canonical = format!("{}{}", deploy::PAGES_URL, target.path.split('#').next().unwrap_or(""));
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Moved: {} · {}</title>\n<meta http-equiv=\"refresh\" content=\"{delay}; url={}\">\n<link rel=\"canonical\" href=\"{}\">\n<meta name=\"robots\" content=\"noindex\">\n{}</head>\n<body>\n<main class=\"moved\">\n<p class=\"crumbs\"><a href=\"{prefix}index.html\">{}</a></p>\n<h1>{}</h1>\n<p>{}</p>\n<p><a href=\"{}\">Go to {}</a>{}</p>\n</main>\n</body>\n</html>\n",
        esc(&title),
        pages::SITE_NAME,
        esc(&url),
        esc(&canonical),
        pages::stylesheet(&prefix),
        pages::SITE_NAME,
        esc(&title),
        esc(note.unwrap_or("This page moved.")),
        esc(&url),
        esc(&target.title),
        if delay > 0 { format!(" <span class=\"hint\">(taking you there in {delay} s)</span>") } else { String::new() }
    )
}

/// A page is a redirect (its meta refresh).
pub fn is_redirect(html: &str) -> bool {
    html.contains("<meta http-equiv=\"refresh\"")
}

/// The page a redirect leads to, as a path from the site's root (its `#fragment` dropped).
pub fn redirect_target(from: &str, html: &str) -> Option<String> {
    let url = html.split("<meta http-equiv=\"refresh\" content=\"").nth(1)?.split('"').next()?.split_once("url=")?.1;
    let url = url.replace("&amp;", "&");
    let url = url.split(['#', '?']).next().unwrap_or("");
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for c in url.split('/') {
        match c {
            ".." => {
                parts.pop()?;
            }
            "." | "" => {}
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// Write the redirect pages of `listed` the build does not write (`built`): returns how many, and
/// a note per kind of page.
pub fn write_redirects(site: &Site, repo: &Path, out: &Path, listed: &BTreeMap<String, Entry>, built: &BTreeSet<String>) -> Result<(usize, Vec<String>)> {
    let mut n = 0;
    let mut by_what: BTreeMap<String, usize> = BTreeMap::new();
    for e in listed.values().filter(|e| !built.contains(&e.path)) {
        let file = out.join(&e.path);
        let Some(target) = resolve(e, site) else {
            // (a page vjeux deleted: gone from a folder an earlier build left it in)
            let _ = std::fs::remove_file(&file);
            continue;
        };
        let note = note(e, &target, repo);
        if let Some(p) = file.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&file, redirect_page(site, e, &target, note.as_deref())).with_context(|| format!("writing {}", file.display()))?;
        n += 1;
        *by_what.entry(e.what.clone()).or_default() += 1;
    }
    let notes = by_what.into_iter().map(|(w, k)| format!("{k} {w}")).collect();
    Ok((n, notes))
}

/// Every listed path resolves in a generated site: a page, or a redirect to an existing page that
/// is not a redirect itself; a page vjeux deleted stays deleted. Returns (paths that resolve,
/// what does not).
pub fn check(dir: &Path, listed: &BTreeMap<String, Entry>) -> (usize, Vec<String>) {
    let mut ok = 0;
    let mut bad = Vec::new();
    for e in listed.values() {
        let file = dir.join(&e.path);
        if e.what == "deleted" {
            if file.exists() {
                bad.push(format!("{} (deleted by vjeux) is written", e.path));
            } else {
                ok += 1;
            }
            continue;
        }
        let Ok(html) = std::fs::read_to_string(&file) else {
            bad.push(format!("{}: 404", e.path));
            continue;
        };
        if is_redirect(&html) {
            match redirect_target(&e.path, &html) {
                Some(t) => match std::fs::read_to_string(dir.join(&t)) {
                    Ok(h) if !is_redirect(&h) || t == "index.html" => ok += 1,
                    Ok(_) => bad.push(format!("{} -> {t}: a redirect to a redirect", e.path)),
                    Err(_) => bad.push(format!("{} -> {t}: 404", e.path)),
                },
                None => bad.push(format!("{}: a redirect without a target", e.path)),
            }
        } else {
            ok += 1;
        }
    }
    (ok, bad)
}

/// The repository's root (the checkout `site deploy` and `site publish` run in).
pub fn repo_root() -> PathBuf {
    Git::here().run(&["rev-parse", "--show-toplevel"]).map(|s| PathBuf::from(s.trim())).unwrap_or_else(|_| PathBuf::from("."))
}

/// Merge a deployed site's pages (`commit` in the current repository: the live gh-pages) into the
/// repository's list: returns how many were new.
pub fn merge_tree(repo: &Path, commit: &str) -> Result<usize> {
    let git = Git::here();
    let when = git.run(&["log", "-1", "--format=%ct", commit]).ok().and_then(|s| s.trim().parse::<i64>().ok()).map(fmt_utc).unwrap_or_default();
    let entries = of_tree(&git, commit, &when)?;
    let path = repo.join(FILE);
    let mut map = read(&path);
    let n = merge(&mut map, entries);
    if n > 0 {
        write_list(&path, &map)?;
    }
    Ok(n)
}

/// Merge a generated site's list (`<dir>/data/published.tsv`) into the repository's: returns how
/// many paths were new.
pub fn merge_site(repo: &Path, dir: &Path) -> Result<usize> {
    let Ok(text) = std::fs::read_to_string(dir.join(SITE_FILE)) else { return Ok(0) };
    let path = repo.join(FILE);
    let mut map = read(&path);
    let n = merge(&mut map, parse(&text));
    if n > 0 {
        write_list(&path, &map)?;
    }
    Ok(n)
}

/// `site published --recover`: the list rebuilt from every deploy GitHub Pages kept (its Pages
/// builds' commits, fetched by hash into a partial clone: trees for the paths, a deploy's data for
/// what each of its pages was), merged into the repository's.
pub fn recover(repo: &Path) -> Result<()> {
    let mut deploys: Vec<(String, String)> = Vec::new();
    for page in 1..=50 {
        let v = deploy::gh_json(&format!("repos/{}/pages/builds?per_page=100&page={page}", deploy::REPO))?;
        let a = v.as_array().cloned().unwrap_or_default();
        if a.is_empty() {
            break;
        }
        for b in a {
            if let (Some(t), Some(c)) = (b["created_at"].as_str(), b["commit"].as_str()) {
                deploys.push((t.to_string(), c.to_string()));
            }
        }
    }
    deploys.sort();
    deploys.dedup_by(|a, b| a.1 == b.1);
    if deploys.is_empty() {
        bail!("no Pages builds listed (gh api repos/{}/pages/builds)", deploy::REPO);
    }
    let cache = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(".cache/mctas/site/pages.git");
    let token = Command::new("gh").args(["auth", "token"]).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let header = (!token.is_empty()).then(|| format!("http.extraHeader=Authorization: Basic {}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("vjeux:{token}"))));
    if !cache.join("HEAD").exists() {
        std::fs::create_dir_all(&cache)?;
        Git { dir: None, header: None }.run(&["init", "-q", "--bare", &cache.to_string_lossy()])?;
    }
    let git = Git { dir: Some(cache.clone()), header };
    let _ = git.run(&["remote", "add", "origin", &format!("https://github.com/{}", deploy::REPO)]);
    let missing: Vec<&str> = deploys.iter().map(|(_, c)| c.as_str()).filter(|c| git.run(&["cat-file", "-e", &format!("{c}^{{commit}}")]).is_err()).collect();
    if !missing.is_empty() {
        eprintln!("fetching {} deploys (commits and trees) into {}", missing.len(), cache.display());
        let mut args = vec!["fetch", "-q", "--filter=blob:none", "--no-tags", "origin"];
        args.extend(missing.iter().copied());
        git.run(&args)?;
    }
    // every page of every deploy: the first deploy that had it, the last
    let mut first: BTreeMap<String, String> = BTreeMap::new();
    let mut last: BTreeMap<String, String> = BTreeMap::new();
    for (t, c) in &deploys {
        for p in tree_pages(&git, c)? {
            first.entry(p.clone()).or_insert_with(|| t.clone());
            last.insert(p, c.clone());
        }
    }
    // what each was: the last deploy that had it says (its data read once per deploy)
    let mut by_commit: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (p, c) in &last {
        by_commit.entry(c.clone()).or_default().push(p.clone());
    }
    let mut entries: Vec<Entry> = Vec::new();
    for (c, paths) in &by_commit {
        let want: BTreeSet<&String> = paths.iter().collect();
        let es = of_tree(&git, c, "")?;
        eprintln!("deploy {}: {} pages last seen there", &c[..9.min(c.len())], want.len());
        entries.extend(es.into_iter().filter(|e| want.contains(&e.path)));
    }
    for e in &mut entries {
        let t = first.get(&e.path).cloned().unwrap_or_default();
        e.first = deploy::parse_utc(&t).map(fmt_utc).unwrap_or(t);
    }
    let path = repo.join(FILE);
    let mut map = read(&path);
    let n = merge(&mut map, entries);
    write_list(&path, &map)?;
    let mut by_what: BTreeMap<&str, usize> = BTreeMap::new();
    for e in map.values() {
        *by_what.entry(e.what.as_str()).or_default() += 1;
    }
    println!("{}: {} paths from {} deploys ({n} new): {}", path.display(), map.len(), deploys.len(), by_what.iter().map(|(w, k)| format!("{k} {w}")).collect::<Vec<_>>().join(", "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_round_trips_and_merges_by_path() {
        let a = Entry { path: "circuits/shulker/shulker-loader-1wide-28b.html".into(), what: "circuit".into(), key: "shulker/shulker-loader".into(), title: "Shulker loader: 28 blocks".into(), first: "2026-10-21 07:44Z".into(), ..Default::default() };
        let mut map = BTreeMap::new();
        assert_eq!(merge(&mut map, [a.clone()]), 1);
        assert_eq!(parse(&render(&map)), vec![a.clone()]);
        // a later record keeps the earliest time; a path that leaves the folder is dropped
        let later = Entry { first: "2026-10-21 09:00Z".into(), ..a.clone() };
        assert_eq!(merge(&mut map, [later]), 0);
        assert_eq!(map[&a.path].first, "2026-10-21 07:44Z");
        assert!(parse("../etc/passwd\tpage\n/abs\tpage\n").is_empty());
    }

    #[test]
    fn a_rejudged_line_in_words() {
        let why = "FAIL: shulker-loader-1wide-28b shulker-loader: jams with a full chest: rotation 0: 0 of 26 boxes after 22020 gt (the cap)";
        assert_eq!(reason_words(why, "shulker-loader-1wide-28b"), "jams with a full chest: 0 of 26 boxes after 22020 gt");
        let r7 = "needs the judge's block placement (R7: passes with a redstone block written into its input cell, not with a real input): FAIL: counter7-ring7 counter 7: input in at 3";
        assert_eq!(reason_words(r7, "counter7-ring7"), "needs the judge's block placement (passes with a redstone block written into its input cell, not with a real input)");
        // main's newer line (`# rejudged <date>: <why>`): who asked for the rule stays ours
        let face = "refused by the display-face rule (the seven lamps must form the 3x5 figure-8 on one exterior face, vjeux 11:43Z); its lamps sit inside the stack";
        assert_eq!(reason_words(face, "seven-segment-analog"), "refused by the display-face rule (the seven lamps must form the 3x5 figure-8 on one exterior face); its lamps sit inside the stack");
    }

    #[test]
    fn utc_minutes() {
        assert_eq!(fmt_utc(0), "1970-01-01 00:00Z");
        assert_eq!(fmt_utc(1_792_440_420), "2026-10-19 20:07Z");
        assert_eq!(fmt_utc(1_709_251_140), "2024-02-29 23:59Z");
    }

    #[test]
    fn a_redirect_resolves_relative_to_its_page() {
        let html = "<meta http-equiv=\"refresh\" content=\"4; url=../../circuits/shulker/shulker-loader-1wide.html\">";
        assert_eq!(redirect_target("circuits/shulker/shulker-loader-1wide-28b.html", html).as_deref(), Some("circuits/shulker/shulker-loader-1wide.html"));
        let idx = "<meta http-equiv=\"refresh\" content=\"4; url=../index.html#kind=clock&amp;time=20\">";
        assert_eq!(redirect_target("clocks/x.html", idx).as_deref(), Some("index.html"));
    }

    #[test]
    fn the_check_finds_dead_links_and_redirect_chains() {
        let dir = std::env::temp_dir().join(format!("mctas-published-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("circuits/a")).unwrap();
        let page = |p: &str, html: &str| std::fs::write(dir.join(p), html).unwrap();
        page("circuits/a/live.html", "<html>a page</html>");
        page("circuits/a/moved.html", "<meta http-equiv=\"refresh\" content=\"4; url=../../circuits/a/live.html\">");
        page("circuits/a/chain.html", "<meta http-equiv=\"refresh\" content=\"0; url=../../circuits/a/moved.html\">");
        page("circuits/a/dead.html", "<meta http-equiv=\"refresh\" content=\"0; url=../../circuits/a/nothing.html\">");
        page("about.html", "vjeux deleted it");
        let mut listed = BTreeMap::new();
        let e = |p: &str, what: &str| Entry { path: p.into(), what: what.into(), ..Default::default() };
        merge(&mut listed, [e("circuits/a/live.html", "circuit"), e("circuits/a/moved.html", "circuit"), e("circuits/a/chain.html", "circuit"), e("circuits/a/dead.html", "circuit"), e("circuits/a/gone.html", "circuit"), e("about.html", "deleted")]);
        let (ok, bad) = check(&dir, &listed);
        assert_eq!(ok, 2, "{bad:?}");
        assert_eq!(bad.len(), 4, "{bad:?}");
        assert!(bad.iter().any(|b| b.contains("gone.html: 404")) && bad.iter().any(|b| b.contains("a redirect to a redirect")) && bad.iter().any(|b| b.contains("deleted by vjeux")), "{bad:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
