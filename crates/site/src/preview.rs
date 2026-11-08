//! `mctas site preview <design>... [--name <slug>]`: any design an agent is working on as a live
//! page in under a minute, so the owner can look at it and iterate (vjeux 2026-11-08: "push farms
//! to the site in less than a minute. I want to be able to see them in order to iterate with
//! you"). docs/site.md#previews.
//!
//! - The page is the site's own build page (`pages::build_page`) for the design as it stands in
//!   the checkout, archived or not, judged or not: the 3D view running it (the checkout's viewer
//!   bundle), its card's numbers, the design file and its Litematica schematic. Its numbers: the
//!   archive's verdict when the design is archived (the same structure, any file name), else the
//!   kind's own judge on redsim, exactly what `mctas circuit sim` runs (src/simulate/preview.rs,
//!   cached), else none. A judge slower than `--judge-timeout` does not hold the page: it goes up
//!   without its numbers, and again with them once the judge is done.
//! - It goes to its own repository, vjeux/redstonepedia-preview, whose GitHub Pages an Actions
//!   workflow deploys on every push (about 30 s, no 10-builds-an-hour cap): never the site's
//!   gh-pages, never crates/site/published.tsv, never the site's Pages build budget. One commit on
//!   top of the repository's head (another lane's push in between: fetched again and redone),
//!   built by `git fast-import` from memory. A preview lives at `<slug>/<stem>.html`; the shared
//!   files are content-addressed (`viewer/<hash>/`, `textures/<hash>/`, `css/<hash>/`), so each
//!   preview keeps the viewer it was made with; `index.html` lists every preview from
//!   `previews.tsv` (its time, its branch and commit, its design's hash).
//! - Every preview page carries the status label "Preview: not judged / not verified" and its
//!   card's status, `noindex, nofollow` (robots.txt says the same), no sitemap, and links back to
//!   the site; the site never links to it. No claim of a test in the real game unless the design
//!   was archived (`pages::Preview`). Nothing internal is published: every text file and the
//!   commit message pass [`internal_names`] first.
//! - A new preview under a name replaces the old one; previews older than `--older-than` days are
//!   removed by every preview (`--clean` alone just prunes). The command ends when the page answers
//!   with its new content and prints the seconds from its start.
//! - Previews need no merge, no review and no coordinator GO: any lane runs it from its own branch.
//!   They run no tests. Publishing to the site itself stays `mctas site publish`.

use crate::pages::{self, Scripts};
use crate::{PreviewArgs, circuits, generate};
use anyhow::{Context, Result, bail};
use circuit::archive::Row;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The manifest of every preview, one row per page.
const MANIFEST: &str = "previews.tsv";
const MANIFEST_HEADER: &str = "slug\tpage\ttitle\tstatus\ttime\tbranch\tcommit\tdesign\tviewer\ttextures\tcss";
/// The commits' identity (AGENTS.md).
const AUTHOR: &str = "Christopher Chedeau <vjeuxx@gmail.com>";
/// The status label every preview page carries (the coordinator's condition, 2026-11-08).
pub const LABEL: &str = "Preview: not judged / not verified";

/// The Pages URL of a repository `owner/name`: `https://owner.github.io/name/`.
pub fn pages_url(repo: &str) -> String {
    let (owner, name) = repo.split_once('/').unwrap_or(("vjeux", repo));
    format!("https://{}.github.io/{name}/", owner.to_lowercase())
}

/// What the preview's judge said, for the card and the label.
#[derive(Clone, Debug)]
enum Verdict {
    /// The same structure is archived: its study's judge's row (as the site reads it).
    Archived(Row),
    /// The kind's judge on redsim, run for the preview.
    Judged(crate::preview_judge::Judged),
    /// No numbers: why (`pending`: the judge is still running).
    None { why: String, pending: bool },
}

/// One rendered page and its files.
struct Page {
    stem: String,
    title: String,
    status: String,
    design_hash: String,
    /// (path in the preview repository, bytes)
    files: Vec<(String, Vec<u8>)>,
    pending: bool,
}

/// Where the preview comes from: the checkout's branch and commit, now.
struct From {
    repo: PathBuf,
    branch: String,
    commit: String,
    /// The checkout has changes the commit does not hold (the design files are what is rendered).
    dirty: bool,
    time: String,
    build_id: String,
}

/// The shared files of a preview, by content hash.
struct Assets {
    viewer: String,
    textures: String,
    css: String,
    viewer_dir: PathBuf,
    textures_dir: PathBuf,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The first 12 hex digits of the sha256 of every file of a folder (names and bytes, in order).
fn tree_hash(dir: &Path) -> Result<String> {
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for f in &files {
        h.update(f.strip_prefix(dir).unwrap_or(f).to_string_lossy().as_bytes());
        h.update(std::fs::read(f)?);
    }
    Ok(hex(&h.finalize()[..6]))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

fn short_hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes)[..4])
}

fn git_out(repo: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// A name usable in a URL: lowercase letters, digits, `-` and `_`.
fn slug_of(s: &str) -> String {
    let s: String = s.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "preview".to_string() } else { s }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The words that name places inside a company (hosts, tools, sites), as the first 8 hex digits of
/// the sha256 of the lowercase word with its trailing digits dropped (a host's number): a public
/// preview never carries one (2026-11-08: pages, index, data and commit messages are checked
/// before the push), and this public file does not spell them out. [`internal_names`] adds the
/// patterns.
const INTERNAL_WORDS: [&str; 17] = [
    "773f0ccd", "381132e9", "692c61b7", "45b36efe", "a7796a03", "caeb12e4", "dd0533d0", "3d59f754", "f4a38066", "745cf64d", "cff53eb1", "2ad72d28", "921de51c", "8c779db4", "83366499", "56c7cb2f", "962c206a",
];

/// The internal names in `text` (each once): a word of [`INTERNAL_WORDS`], the word "Meta", the
/// word "fb" (not the viewer's `fb-` class names), a session-like UUID, a diff number (D and 7 to
/// 10 digits).
pub fn internal_names(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut hits: BTreeSet<String> = BTreeSet::new();
    for w in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        let base = w.trim_end_matches(|c: char| c.is_ascii_digit());
        if base.len() >= 3 && INTERNAL_WORDS.contains(&short_hash(base.as_bytes()).as_str()) {
            hits.insert(w.to_string());
        }
    }
    let b = text.as_bytes();
    let word = |i: usize| b.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
    for (i, _) in text.match_indices("Meta") {
        if (i == 0 || !word(i - 1)) && !word(i + 4) {
            hits.insert("Meta".into());
        }
    }
    for (i, _) in lower.match_indices("fb") {
        if (i == 0 || !word(i - 1)) && !word(i + 2) && b.get(i + 2) != Some(&b'-') {
            hits.insert("fb".into());
        }
    }
    let lb = lower.as_bytes();
    let hexd = |i: usize, n: usize| (i..i + n).all(|k| lb.get(k).is_some_and(|c| c.is_ascii_hexdigit()));
    for i in 0..lb.len() {
        // a UUID: 8-4-4-4-12 hex digits
        if (i == 0 || !lb[i - 1].is_ascii_hexdigit()) && hexd(i, 8) && lb.get(i + 8) == Some(&b'-') && hexd(i + 9, 4) && lb.get(i + 13) == Some(&b'-') && hexd(i + 14, 4) && lb.get(i + 18) == Some(&b'-') && hexd(i + 19, 4) && lb.get(i + 23) == Some(&b'-') && hexd(i + 24, 12) {
            hits.insert(text[i..i + 36].to_string());
        }
        // a diff number
        if b[i] == b'D' && (i == 0 || !word(i - 1)) {
            let n = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if (7..=10).contains(&n) && !word(i + 1 + n) {
                hits.insert(text[i..i + 1 + n].to_string());
            }
        }
    }
    hits.into_iter().collect()
}

/// The archive row of the same structure as `c` (the archive's own key: its content hash and
/// kind), if its study archived it.
fn archived_row(root: &Path, c: &circuit::Circuit) -> Option<Row> {
    let kind = c.kind.as_ref()?;
    let hash = format!("{}-{}", clock::catalog::content_hash(&c.clock), kind.name().replace(' ', "_"));
    circuit::archive::read_rows(&circuit::archive::archive_dir(root, kind.study())).ok()?.into_iter().find(|r| r.hash == hash)
}

/// The judge's verdict on `text` (cached: src/cache.rs, the preview domain).
fn judge(text: &str) -> crate::preview_judge::Judged {
    crate::cache::get_or(crate::cache::Domain::Preview, &[b"preview-judge", text.as_bytes()], || crate::preview_judge::preview_judge(text))
}

/// The card's status in words, and the preview's sentence on where its numbers come from.
fn status_words(v: &Verdict) -> (String, String, bool) {
    match v {
        Verdict::Archived(r) => {
            let how = if r.robustness.is_empty() { String::new() } else { format!(", {}", r.robustness) };
            (format!("archived {} by its study's judge{how}", site_model::normalize_time(&r.archived)), String::new(), true)
        }
        Verdict::Judged(j) if j.pass => {
            let how = if j.every_build { format!("every build, {}", if j.robustness.is_empty() { "no robustness verdict" } else { j.robustness.as_str() }) } else { "one build, no robustness verdict".to_string() };
            (
                format!("passes its kind's judge on our simulation of the game ({how}); not archived, not tested in the real game"),
                format!("Its numbers come from its kind's judge, run for this preview on our simulation of the game ({how}); it has not been tested in the real game."),
                false,
            )
        }
        Verdict::Judged(j) if j.ran => (
            format!("FAILS its kind's judge on our simulation of the game: {}", j.why),
            format!("Its kind's judge, run on our simulation of the game for this preview, fails it ({}): it has no numbers.", j.why),
            false,
        ),
        Verdict::Judged(j) => (format!("not judged: {}", j.why), format!("It has not been judged: {}.", j.why.trim_end_matches('.')), false),
        Verdict::None { why, .. } => (format!("not judged: {why}"), format!("It has not been judged yet: {}.", why.trim_end_matches('.')), false),
    }
}

/// The verdicts-row the page is built from: the archive's, the preview judge's, or the design's
/// own block count and box with no numbers.
fn row_for(c: &circuit::Circuit, file: &str, v: &Verdict) -> Row {
    let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut r = Row {
        archived: String::new(),
        file: file.to_string(),
        kind,
        axis: String::new(),
        metrics: String::new(),
        blocks: c.blocks(),
        dims: format!("{}x{}x{}", d[0], d[1], d[2]),
        volume: (d[0] * d[1] * d[2]) as i64,
        family: String::new(),
        robustness: String::new(),
        kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
        type_key: circuit::archive::type_key(c),
        hash: String::new(),
        summary: String::new(),
    };
    match v {
        Verdict::Archived(a) => {
            r = Row { file: file.to_string(), ..a.clone() };
        }
        Verdict::Judged(j) if j.pass => {
            r.axis = j.axis.clone();
            r.metrics = j.metrics.clone();
            r.dims = j.dims.clone();
            r.volume = j.volume;
            r.family = j.family.clone();
            r.robustness = j.robustness.clone();
            r.summary = j.summary.clone();
        }
        _ => {}
    }
    r
}

/// The verdicts.tsv header (circuit::archive's columns, in its order).
const VERDICTS_HEADER: &str = "archived\tfile\tkind\taxis\tmetrics\tblocks\tdims\tvolume\tfamily\trobustness\tkinds\ttype_key\thash\tsummary";

/// One verdicts.tsv line of `r` (tabs and line breaks inside a field become spaces, as the archive
/// writes them).
fn verdicts_line(r: &Row) -> String {
    [r.archived.clone(), r.file.clone(), r.kind.clone(), r.axis.clone(), r.metrics.clone(), r.blocks.to_string(), r.dims.clone(), r.volume.to_string(), r.family.clone(), r.robustness.clone(), r.kinds.clone(), r.type_key.clone(), r.hash.clone(), r.summary.clone()]
        .map(|s| s.replace(['\t', '\n'], " "))
        .join("\t")
}

/// The site's own build of one design with the row `row`: the site's archive reader
/// (`circuits::archived`) on a folder of its own holding just this design, its row and its
/// caption, so the page is exactly what the site would write for it.
fn build_of(study: &str, file: &str, text: &str, row: &Row, caption: Option<&str>, notes: &mut Vec<String>) -> Result<site_model::Build> {
    let root = std::env::temp_dir().join(format!("mctas-preview-{}-{}", std::process::id(), short_hash(format!("{file}{}", now_secs()).as_bytes())));
    let dir = circuit::archive::archive_dir(&root, study);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(file), text)?;
    std::fs::write(dir.join("verdicts.tsv"), format!("{VERDICTS_HEADER}\n{}\n", verdicts_line(row)))?;
    if let Some(c) = caption.filter(|c| !c.is_empty()) {
        std::fs::write(dir.join("captions.tsv"), format!("file\tcaption\n{file}\t{}\n", c.replace(['\t', '\n'], " ")))?;
    }
    let mut builds = circuits::archived(&root, notes, true);
    let _ = std::fs::remove_dir_all(&root);
    match builds.len() {
        1 => Ok(builds.remove(0)),
        n => bail!("{file}: the site's archive reader made {n} builds of it"),
    }
}

/// Render one design's page under `slug` with the verdict `v`.
fn render(path: &Path, slug: &str, from: &From, assets: &Assets, v: &Verdict, site_url: &str, preview_url: &str) -> Result<Page> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let c = circuit::Circuit::load(path)?;
    let Some(kind) = c.kind.clone() else { bail!("{}: no `circuit: <kind>` line (`site preview` renders circuit designs)", path.display()) };
    let study = kind.study();
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let stem = slug_of(file.trim_end_matches(".txt"));
    let row = row_for(&c, &file, v);
    // its caption: the archive's for an archived design, else its own comment lines (the site's
    // reader falls back to them)
    let caption = match v {
        Verdict::Archived(a) => circuit::archive::read_captions(&circuit::archive::archive_dir(&from.repo.join(circuits::ROOT), study)).get(&a.file).cloned(),
        _ => None,
    };
    let mut notes = Vec::new();
    let mut b = build_of(study, &file, &text, &row, caption.as_deref(), &mut notes)?;
    b.caption = crate::explain::scrub_attribution(&b.caption);
    // its place in the preview repository: the page, its design and schematic, its stills
    b.id = format!("preview/{slug}/{stem}");
    b.page = format!("{slug}/{stem}.html");
    b.design = format!("{slug}/{stem}.txt");
    let mut files: Vec<(String, Vec<u8>)> = vec![(b.design.clone(), text.clone().into_bytes())];
    if let Some(cc) = b.circuit.as_mut() {
        if !cc.view_src.is_empty() {
            cc.view_src = format!("{slug}/stills/{stem}.txt");
            files.push((cc.view_src.clone(), cc.view_text.clone().into_bytes()));
        }
        if !cc.card_src.is_empty() {
            cc.card_src = format!("{slug}/stills/{stem}-card.txt");
            files.push((cc.card_src.clone(), cc.card_text.clone().into_bytes()));
        }
    }
    let design_hash = short_hash(text.as_bytes());
    let repo_path = path.canonicalize().ok().and_then(|p| from.repo.canonicalize().ok().and_then(|r| p.strip_prefix(r).ok().map(|x| x.to_string_lossy().to_string()))).unwrap_or_default();
    // the design's own commit: the last one that changed its file, when the file is committed as
    // it is (else its hash alone says which version this is)
    let design_commit = (!repo_path.is_empty())
        .then(|| (git_out(&from.repo, &["ls-files", "--error-unmatch", "--", &repo_path]).is_some() && git_out(&from.repo, &["status", "--porcelain", "--", &repo_path]).is_some_and(|s| s.is_empty())).then(|| git_out(&from.repo, &["log", "-1", "--format=%H", "--", &repo_path])))
        .flatten()
        .flatten()
        .filter(|c| !c.is_empty());
    let design_id = match &design_commit {
        Some(c) => format!("{design_hash} (committed in {})", &c[..c.len().min(9)]),
        None => format!("{design_hash} (not committed)"),
    };
    let (status, numbers, game_tested) = status_words(v);
    let pending = matches!(v, Verdict::None { pending: true, .. });
    let from_line = format!(
        "{}@{}{}",
        from.branch,
        &from.commit[..from.commit.len().min(9)],
        if from.dirty { " with uncommitted changes" } else { "" }
    );
    let esc = site_model::html::esc;
    let when = format!("{} UTC", from.time.replacen('T', " ", 1).trim_end_matches('Z'));
    let banner = format!(
        "<div class=\"preview-banner\" role=\"status\" data-build=\"{}\" style=\"border:2px solid #c2410c;background:#fff7ed;color:#431407;border-radius:8px;padding:10px 14px;margin:12px 0\"><p style=\"margin:0 0 6px\"><b style=\"background:#c2410c;color:#fff;border-radius:4px;padding:2px 8px\">{LABEL}</b> <span>Card: {}</span></p><p style=\"margin:0;font-size:0.9em\">A work in progress shown to its designer: not a page of <a href=\"{site_url}\">REDSTONEPEDIA</a>, which publishes one tested design per kind. From <code>{}</code>, {}, design <code>{}</code>. <a href=\"{preview_url}\">Every preview</a>.</p></div>\n",
        esc(&from.build_id),
        esc(&status),
        esc(&from_line),
        esc(&when),
        esc(&design_id)
    );
    let footer = format!(
        "A preview of a work in progress (<code>{}</code>, {}), not part of <a href=\"{site_url}\">REDSTONEPEDIA</a>: see the status at the top. Previews are replaced by the next one under their name and removed after a while.",
        esc(&from_line),
        esc(&when)
    );
    pages::set_preview(Some(pages::Preview { banner, footer, game_tested, numbers, commit: design_commit.clone().unwrap_or_else(|| from.commit.clone()) }));
    let site = site_model::Site { version: site_model::DATA_VERSION, generated: from.time.clone(), commit: from.commit.clone(), builds: vec![b.clone()], cards: Vec::new(), members: Vec::new() };
    let html = pages::build_page(&site, &b, &text, &repo_path, Scripts { viewer: true, app: false, ui: false });
    pages::set_preview(None);
    let html = relink(&html, slug, assets, site_url)?;
    files.push((b.page.clone(), html.into_bytes()));
    // the Litematica schematic next to the design
    let d = mctas::design::Design::parse(&text).with_context(|| format!("parsing {}", path.display()))?;
    let desc = format!("{} REDSTONEPEDIA preview (not verified): {preview_url}{}", crate::explain::caption_text(&b.caption), b.page);
    let bytes = litematic::export(&d, &litematic::Meta::new(&stem, desc.trim(), litematic::now_ms())).with_context(|| format!("{}: litematic", path.display()))?;
    files.push((generate::litematic_path(&b.design), bytes));
    for n in notes {
        eprintln!("  note: {n}");
    }
    Ok(Page { stem, title: pages::page_title(&b), status, design_hash: design_id, files, pending })
}

/// A page written for the site, moved to the preview repository: its shared files by content
/// hash, every other link of the site to the live site (the header, the crumbs, the chips). The
/// shared files' URLs must be found (a template change that moves them fails here, not on the
/// page).
fn relink(html: &str, slug: &str, a: &Assets, site_url: &str) -> Result<String> {
    let mut s = html.to_string();
    for (from, to) in [
        (format!("data-mcview-textures=\"../{}\"", mcview::atlas::vanilla::SITE_DIR), format!("data-mcview-textures=\"../textures/{}\"", a.textures)),
        ("\"../viewer/mcview.js?v=".to_string(), format!("\"../viewer/{}/mcview.js?v=", a.viewer)),
        ("\"../viewer/mcview_bg.wasm?v=".to_string(), format!("\"../viewer/{}/mcview_bg.wasm?v=", a.viewer)),
        ("href=\"../style.css".to_string(), format!("href=\"../css/{}/style.css", a.css)),
    ] {
        if !s.contains(&from) {
            bail!("the page template has no {from:?}: crates/site/src/preview.rs::relink needs updating");
        }
        s = s.replace(&from, &to);
    }
    // every other relative link out of the preview's folder goes to the site
    let keep = [format!("{slug}/"), "viewer/".into(), "textures/".into(), "css/".into(), "favicon.svg".into()];
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(i) = rest.find("=\"../") {
        let attr_start = rest[..i].rfind(' ').map(|k| k + 1).unwrap_or(0);
        let attr = &rest[attr_start..i];
        let after = &rest[i + 5..];
        out.push_str(&rest[..i + 2]);
        if matches!(attr, "href" | "action") && !keep.iter().any(|k| after.starts_with(k.as_str())) {
            out.push_str(site_url);
        } else {
            out.push_str("../");
        }
        rest = after;
    }
    out.push_str(rest);
    // the tabs' counts are the preview's own (one build): left out
    let mut cleaned = String::with_capacity(out.len());
    let mut r = out.as_str();
    while let Some(i) = r.find(" <span class=\"count\">") {
        cleaned.push_str(&r[..i]);
        match r[i..].find("</span>") {
            Some(j) => r = &r[i + j + 7..],
            None => {
                r = &r[i..];
                break;
            }
        }
    }
    cleaned.push_str(r);
    Ok(cleaned)
}

/// The previews' index page (every preview, newest first), from the manifest's rows.
fn index_page(rows: &[Vec<String>], css: &str, site_url: &str, older_than: u32) -> String {
    let esc = site_model::html::esc;
    let mut rows: Vec<&Vec<String>> = rows.iter().collect();
    rows.sort_by(|a, b| b[4].cmp(&a[4]).then(a[0].cmp(&b[0])).then(a[1].cmp(&b[1])));
    let body: String = rows
        .iter()
        .map(|r| {
            format!(
                "<tr><td>{}</td><td><a href=\"{}\">{}</a></td><td>{}</td><td class=\"nowrap\">{}</td><td><code>{}@{}</code></td><td><code>{}</code></td></tr>\n",
                esc(&r[0]),
                esc(&r[1]),
                esc(&r[2]),
                esc(&r[3]),
                esc(&r[4].replacen('T', " ", 1).trim_end_matches('Z').to_string()),
                esc(&r[5]),
                esc(&r[6]),
                esc(&r[7])
            )
        })
        .collect();
    let css_link = if css.is_empty() { String::new() } else { format!("<link rel=\"stylesheet\" href=\"css/{css}/style.css\">\n") };
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"robots\" content=\"noindex, nofollow\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Previews · REDSTONEPEDIA</title>\n{css_link}<link rel=\"icon\" href=\"favicon.svg\" type=\"image/svg+xml\">\n</head>\n<body>\n<main>\n<h1>Previews</h1>\n<p><b>{LABEL}.</b> Work in progress shown to its designer: these pages are not part of <a href=\"{site_url}\">REDSTONEPEDIA</a>, which publishes one tested design per kind. A preview is replaced by the next one under its name and removed after {older_than} days.</p>\n<div class=\"scroll-x\"><table class=\"list\"><thead><tr><th>preview</th><th>page</th><th>card</th><th>when (UTC)</th><th>from</th><th>design</th></tr></thead><tbody>\n{body}</tbody></table></div>\n</main>\n</body>\n</html>\n"
    )
}

/// The files every push writes at the repository's root (the same bytes each time).
fn base_files(site_url: &str, preview_url: &str, repo: &str) -> Vec<(String, Vec<u8>)> {
    let readme = format!(
        "# REDSTONEPEDIA previews\n\nWork-in-progress designs of [github.com/vjeux/redstonepedia](https://github.com/vjeux/redstonepedia), each on a page of its own so its designer can look at it: {preview_url}\n\nNot judged / not verified: nothing here is part of [REDSTONEPEDIA]({site_url}). Written by `mctas site preview` (docs/site.md in that repository); every push deploys GitHub Pages through `.github/workflows/pages.yml`. Do not edit by hand ({repo}).\n"
    );
    let workflow = "# Deploys this repository to GitHub Pages on every push (`mctas site preview`, docs/site.md of\n# github.com/vjeux/redstonepedia): a custom workflow has no 10-builds-an-hour cap. No shell steps.\nname: pages\non:\n  push:\n    branches: [main]\n  workflow_dispatch:\npermissions:\n  contents: read\n  pages: write\n  id-token: write\n# a newer push replaces a deploy in progress: it holds every preview too\nconcurrency:\n  group: pages\n  cancel-in-progress: true\njobs:\n  deploy:\n    runs-on: ubuntu-latest\n    environment:\n      name: github-pages\n      url: ${{ steps.deployment.outputs.page_url }}\n    steps:\n      - uses: actions/checkout@v7\n      - uses: actions/upload-pages-artifact@v5\n        with:\n          path: .\n      - id: deployment\n        uses: actions/deploy-pages@v5\n";
    vec![
        ("README.md".into(), readme.into_bytes()),
        ("robots.txt".into(), b"User-agent: *\nDisallow: /\n".to_vec()),
        (".nojekyll".into(), Vec::new()),
        (".github/workflows/pages.yml".into(), workflow.as_bytes().to_vec()),
        ("favicon.svg".into(), generate::FAVICON.as_bytes().to_vec()),
    ]
}

/// The local git repository previews are committed in (no working tree: `git fast-import`).
fn store() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).context("no HOME")?;
    let dir = home.join(".cache/mctas/preview.git");
    if !dir.join("HEAD").exists() {
        std::fs::create_dir_all(&dir)?;
        let o = Command::new("git").args(["init", "-q", "--bare"]).arg(&dir).output()?;
        if !o.status.success() {
            bail!("git init {}: {}", dir.display(), String::from_utf8_lossy(&o.stderr).trim());
        }
    }
    Ok(dir)
}

fn git(store: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git").arg("--git-dir").arg(store).args(args).output().with_context(|| format!("git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The remote's head (None: the repository has no commit yet), fetched into the store.
fn fetch_head(store: &Path, remote: &str, header: &str) -> Result<Option<String>> {
    let o = Command::new("git").arg("--git-dir").arg(store).args(["-c", header, "fetch", "-q", remote, "+refs/heads/main:refs/remotes/origin/main"]).output()?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        if err.contains("couldn't find remote ref") || err.contains("Couldn't find remote ref") {
            return Ok(None);
        }
        bail!("git fetch {remote}: {}", err.trim());
    }
    Ok(git(store, &["rev-parse", "refs/remotes/origin/main"]).ok())
}

/// The manifest's rows at `head`.
fn read_manifest(store: &Path, head: Option<&str>) -> Vec<Vec<String>> {
    let Some(h) = head else { return Vec::new() };
    git(store, &["show", &format!("{h}:{MANIFEST}")])
        .unwrap_or_default()
        .lines()
        .skip(1)
        .map(|l| l.split('\t').map(String::from).collect::<Vec<_>>())
        .filter(|r| r.len() >= 11)
        .collect()
}

/// The paths a tree holds under `prefix/` (empty: none).
fn has_path(store: &Path, head: Option<&str>, path: &str) -> bool {
    head.is_some_and(|h| git(store, &["cat-file", "-e", &format!("{h}:{path}")]).is_ok())
}

/// One commit on top of `head` (built by `git fast-import` from memory): delete `dirs`, write
/// `files`. Returns the commit, or None when it would change nothing.
fn commit(store: &Path, head: Option<&str>, dirs: &[String], files: &[(String, Vec<u8>)], msg: &str, attempt: u32) -> Result<Option<String>> {
    let r = format!("refs/mctas/preview-{}-{attempt}", std::process::id());
    let now = now_secs();
    let mut stream: Vec<u8> = Vec::new();
    writeln!(stream, "commit {r}")?;
    writeln!(stream, "author {AUTHOR} {now} +0000")?;
    writeln!(stream, "committer {AUTHOR} {now} +0000")?;
    writeln!(stream, "data {}", msg.len())?;
    stream.extend_from_slice(msg.as_bytes());
    stream.push(b'\n');
    if let Some(h) = head {
        writeln!(stream, "from {h}")?;
    }
    for d in dirs {
        writeln!(stream, "D {d}")?;
    }
    for (path, bytes) in files {
        writeln!(stream, "M 100644 inline {path}")?;
        writeln!(stream, "data {}", bytes.len())?;
        stream.extend_from_slice(bytes);
        stream.push(b'\n');
    }
    stream.extend_from_slice(b"done\n");
    let mut child = Command::new("git").arg("--git-dir").arg(store).args(["fast-import", "--quiet", "--done", "--force"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().context("git fast-import")?;
    child.stdin.take().context("fast-import stdin")?.write_all(&stream)?;
    let o = child.wait_with_output()?;
    if !o.status.success() {
        bail!("git fast-import: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    let c = git(store, &["rev-parse", &r])?;
    let _ = git(store, &["update-ref", "-d", &r]);
    if let Some(h) = head
        && git(store, &["rev-parse", &format!("{c}^{{tree}}")])? == git(store, &["rev-parse", &format!("{h}^{{tree}}")])?
    {
        return Ok(None);
    }
    Ok(Some(c))
}

/// What one push changes: the pages of one preview (replacing the name's old ones), the prune.
struct Change {
    slug: Option<String>,
    rows: Vec<Vec<String>>,
    files: Vec<(String, Vec<u8>)>,
    assets: Option<Assets>,
    older_than: u32,
    msg: String,
}

/// Commit `ch` on top of the remote's head and push it, again from a fresh head when another push
/// got in between. Returns the pushed commit (None: nothing to change).
fn publish(repo: &str, ch: &Change) -> Result<Option<String>> {
    let store = store()?;
    let remote = format!("https://github.com/{repo}.git");
    let header = crate::deploy::auth_header(true)?;
    let site_url = crate::deploy::PAGES_URL;
    let preview_url = pages_url(repo);
    for attempt in 1..=6 {
        let head = fetch_head(&store, &remote, &header)?;
        let old = read_manifest(&store, head.as_deref());
        let cutoff = now_secs() - ch.older_than as i64 * 86_400;
        // a slug's newest page decides its age: a preview older than the cutoff goes, and the
        // name being previewed now is replaced
        let mut newest: BTreeMap<String, i64> = BTreeMap::new();
        for r in &old {
            let t = crate::deploy::parse_utc(&r[4]).unwrap_or(0);
            let e = newest.entry(r[0].clone()).or_insert(t);
            *e = (*e).max(t);
        }
        let gone: BTreeSet<String> = newest.iter().filter(|(s, t)| **t < cutoff || ch.slug.as_deref() == Some(s.as_str())).map(|(s, _)| s.clone()).collect();
        let mut rows: Vec<Vec<String>> = old.into_iter().filter(|r| !gone.contains(&r[0])).collect();
        rows.extend(ch.rows.iter().cloned());
        // the shared files nobody uses any more go too
        let used = |col: usize| -> BTreeSet<String> { rows.iter().map(|r| r[col].clone()).collect() };
        let (viewers, textures, csses) = (used(8), used(9), used(10));
        let mut dirs: Vec<String> = gone.iter().cloned().collect();
        for (top, keep) in [("viewer", &viewers), ("textures", &textures), ("css", &csses)] {
            if let Some(h) = head.as_deref() {
                for name in git(&store, &["ls-tree", "--name-only", &format!("{h}:{top}")]).unwrap_or_default().lines() {
                    if !keep.contains(name) {
                        dirs.push(format!("{top}/{name}"));
                    }
                }
            }
        }
        let mut files: Vec<(String, Vec<u8>)> = base_files(site_url, &preview_url, repo);
        // the shared files this preview needs that the repository lacks
        if let Some(a) = &ch.assets {
            if !has_path(&store, head.as_deref(), &format!("viewer/{}/mcview.js", a.viewer)) {
                let mut list = Vec::new();
                walk(&a.viewer_dir, &mut list)?;
                for f in list {
                    let rel = f.strip_prefix(&a.viewer_dir).unwrap_or(&f).to_string_lossy().to_string();
                    files.push((format!("viewer/{}/{rel}", a.viewer), std::fs::read(&f)?));
                }
            }
            if !has_path(&store, head.as_deref(), &format!("textures/{}/{}", a.textures, mcview::atlas::vanilla::ATLAS)) {
                let mut list = Vec::new();
                walk(&a.textures_dir, &mut list)?;
                for f in list {
                    let rel = f.strip_prefix(&a.textures_dir).unwrap_or(&f).to_string_lossy().to_string();
                    files.push((format!("textures/{}/{rel}", a.textures), std::fs::read(&f)?));
                }
            }
            if !has_path(&store, head.as_deref(), &format!("css/{}/style.css", a.css)) {
                files.push((format!("css/{}/style.css", a.css), generate::STYLE.as_bytes().to_vec()));
            }
        }
        files.extend(ch.files.iter().cloned());
        let mut manifest = format!("{MANIFEST_HEADER}\n");
        for r in &rows {
            manifest.push_str(&r.join("\t"));
            manifest.push('\n');
        }
        let newest_css = rows.iter().max_by(|a, b| a[4].cmp(&b[4])).map(|r| r[10].clone()).unwrap_or_default();
        files.push((MANIFEST.into(), manifest.into_bytes()));
        files.push(("index.html".into(), index_page(&rows, &newest_css, site_url, ch.older_than).into_bytes()));
        // nothing internal goes out (the repository is public): every text file this push writes
        // and its message (the viewer's and the textures' files are the site's own)
        let mut found: Vec<String> = Vec::new();
        for (p, bytes) in files.iter().filter(|(p, _)| !p.starts_with("viewer/") && !p.starts_with("textures/") && !p.ends_with(".litematic")) {
            for w in internal_names(&String::from_utf8_lossy(bytes)) {
                found.push(format!("{p}: {w}"));
            }
        }
        for w in internal_names(&ch.msg) {
            found.push(format!("the commit message: {w}"));
        }
        if !found.is_empty() {
            bail!("not published: internal names in what would be public ({}); take them out of the design's comments or the branch name", found.join("; "));
        }
        let Some(c) = commit(&store, head.as_deref(), &dirs, &files, &ch.msg, attempt)? else {
            println!("preview repository already up to date");
            return Ok(None);
        };
        let o = Command::new("git").arg("--git-dir").arg(&store).args(["-c", &header, "push", "-q", &remote, &format!("{c}:refs/heads/main")]).output()?;
        if o.status.success() {
            let _ = git(&store, &["update-ref", "refs/remotes/origin/main", &c]);
            return Ok(Some(c));
        }
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        let raced = err.contains("non-fast-forward") || err.contains("fetch first") || err.contains("rejected");
        eprintln!("push attempt {attempt} failed{}: {}", if raced { " (another push got in first: again from its head)" } else { "" }, err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim());
        if !raced {
            std::thread::sleep(Duration::from_secs(2));
        }
    }
    bail!("could not push to {repo} after 6 attempts")
}

/// Poll `url` until it answers 200 with `token` in it; the seconds since `t0`.
fn wait_live(url: &str, token: &str, t0: Instant, limit: Duration, repo: &str) -> Result<f64> {
    let mut n = 0u32;
    loop {
        n += 1;
        // (a fresh query each time: a cached 404 from before the deploy never answers)
        let o = Command::new("curl").args(["-sS", "--max-time", "10", "-H", "Cache-Control: no-cache", &format!("{url}?poll={n}")]).output().context("running curl")?;
        if o.status.success() && String::from_utf8_lossy(&o.stdout).contains(token) {
            return Ok(t0.elapsed().as_secs_f64());
        }
        if t0.elapsed() > limit {
            bail!("{url} did not answer with the new page within {} s (check https://github.com/{repo}/actions)", limit.as_secs());
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

/// `--setup`: the preview repository (public) and its GitHub Pages deployed by the workflow; both
/// once (an existing one is kept as it is).
fn setup(repo: &str) -> Result<()> {
    let exists = Command::new("gh").args(["api", &format!("repos/{repo}")]).output()?.status.success();
    if exists {
        println!("{repo}: exists");
    } else {
        let o = Command::new("gh")
            .args(["repo", "create", repo, "--public", "--description", "Work-in-progress previews of REDSTONEPEDIA designs (not judged / not verified): mctas site preview", "--homepage", &pages_url(repo)])
            .output()?;
        if !o.status.success() {
            bail!("gh repo create {repo}: {}", String::from_utf8_lossy(&o.stderr).trim());
        }
        println!("{repo}: created");
    }
    let pages = Command::new("gh").args(["api", &format!("repos/{repo}/pages")]).output()?;
    if pages.status.success() {
        println!("{repo}: GitHub Pages already on: {}", String::from_utf8_lossy(&pages.stdout).chars().take(200).collect::<String>());
        return Ok(());
    }
    let o = Command::new("gh").args(["api", "-X", "POST", &format!("repos/{repo}/pages"), "-f", "build_type=workflow"]).output()?;
    if !o.status.success() {
        bail!("enabling GitHub Pages on {repo}: {} {}", String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim());
    }
    println!("{repo}: GitHub Pages on, deployed by its workflow: {}", pages_url(repo));
    Ok(())
}

fn source(repo: &Path, t0: Instant) -> From {
    let branch = git_out(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let commit = git_out(repo, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git_out(repo, &["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    let _ = t0;
    From { repo: repo.to_path_buf(), branch, commit, dirty, time: clock::catalog::now_utc(), build_id: format!("{}-{}", now_secs(), std::process::id()) }
}

fn assets(repo: &Path) -> Result<Assets> {
    let viewer_dir = repo.join("web/viewer");
    if !viewer_dir.join("mcview.js").is_file() {
        bail!("{}: no viewer bundle (mcview.js)", viewer_dir.display());
    }
    let textures_dir = repo.join(mcview::atlas::vanilla::DIR);
    if !textures_dir.join(mcview::atlas::vanilla::ATLAS).is_file() {
        bail!("{}: no {} (the vendored game textures)", textures_dir.display(), mcview::atlas::vanilla::ATLAS);
    }
    Ok(Assets { viewer: tree_hash(&viewer_dir)?, textures: tree_hash(&textures_dir)?, css: short_hash(generate::STYLE.as_bytes()), viewer_dir, textures_dir })
}

/// `mctas site preview`.
pub fn preview(a: &PreviewArgs) -> Result<()> {
    let t0 = Instant::now();
    let repo_root = crate::published::repo_root();
    if a.setup {
        setup(&a.repo)?;
        if a.designs.is_empty() && !a.clean {
            return Ok(());
        }
    }
    let site_url = crate::deploy::PAGES_URL;
    let preview_url = pages_url(&a.repo);
    if a.designs.is_empty() {
        if !a.clean {
            bail!("no design given (`mctas site preview <design>... [--name <slug>]`; `--clean` alone prunes old previews)");
        }
        let ch = Change { slug: None, rows: Vec::new(), files: Vec::new(), assets: None, older_than: a.older_than, msg: format!("prune previews older than {} days", a.older_than) };
        match publish(&a.repo, &ch)? {
            Some(c) => println!("pruned: {} ({c})", preview_url),
            None => println!("nothing to prune"),
        }
        return Ok(());
    }
    let slug = slug_of(&a.name.clone().unwrap_or_else(|| a.designs[0].file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
    let from = source(&repo_root, t0);
    let assets = assets(&repo_root)?;
    // the bundles' own hashes in their URLs, as on the site (their folders are content-addressed
    // already: a page never meets another viewer than its own)
    pages::set_versions(pages::Versions {
        css: pages::bytes_hash(generate::STYLE.as_bytes()),
        viewer_js: pages::file_hash(&assets.viewer_dir.join("mcview.js")),
        viewer_wasm: pages::file_hash(&assets.viewer_dir.join("mcview_bg.wasm")),
        ..Default::default()
    });
    let root = repo_root.join(circuits::ROOT);
    // each design's verdict: archived, else the kind's judge on redsim in the background (a
    // judge slower than --judge-timeout does not hold the page)
    let mut verdicts: Vec<Verdict> = Vec::new();
    let mut jobs: Vec<(usize, std::sync::mpsc::Receiver<crate::preview_judge::Judged>)> = Vec::new();
    for (i, p) in a.designs.iter().enumerate() {
        let c = circuit::Circuit::load(p)?;
        if c.kind.is_none() {
            bail!("{}: no `circuit: <kind>` line (`site preview` renders circuit designs)", p.display());
        }
        if let Some(r) = archived_row(&root, &c) {
            verdicts.push(Verdict::Archived(r));
            continue;
        }
        if a.no_judge {
            verdicts.push(Verdict::None { why: "the preview was made without its judge (--no-judge)".into(), pending: false });
            continue;
        }
        let text = std::fs::read_to_string(p)?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(judge(&text));
        });
        jobs.push((i, rx));
        verdicts.push(Verdict::None { why: "its judge is still running (the page goes up again with its numbers)".into(), pending: true });
    }
    let deadline = t0 + Duration::from_secs(a.judge_timeout);
    let mut waiting: Vec<(usize, std::sync::mpsc::Receiver<crate::preview_judge::Judged>)> = Vec::new();
    for (i, rx) in jobs {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(j) => verdicts[i] = Verdict::Judged(j),
            Err(_) => waiting.push((i, rx)),
        }
    }
    let mut round = 1;
    loop {
        let t_render = Instant::now();
        let mut pages_out: Vec<Page> = Vec::new();
        for (p, v) in a.designs.iter().zip(&verdicts) {
            pages_out.push(render(p, &slug, &from, &assets, v, site_url, &preview_url)?);
        }
        let stems: BTreeSet<&String> = pages_out.iter().map(|p| &p.stem).collect();
        if stems.len() < pages_out.len() {
            bail!("two designs with one file name in one preview: give them different names");
        }
        println!("rendered {} page{} in {:.1} s", pages_out.len(), if pages_out.len() == 1 { "" } else { "s" }, t_render.elapsed().as_secs_f64());
        if let Some(out) = &a.out {
            write_out(out, &slug, &pages_out, &assets, site_url, &preview_url, a.older_than, &from, &a.repo)?;
            println!("wrote {} (not pushed): {} page(s) under {slug}/", out.display(), pages_out.len());
            return Ok(());
        }
        let rows: Vec<Vec<String>> = pages_out
            .iter()
            .map(|p| vec![slug.clone(), format!("{slug}/{}.html", p.stem), p.title.clone(), p.status.clone(), from.time.clone(), from.branch.clone(), from.commit.chars().take(9).collect(), p.design_hash.clone(), assets.viewer.clone(), assets.textures.clone(), assets.css.clone()])
            .map(|r| r.into_iter().map(|f| f.replace(['\t', '\n'], " ")).collect())
            .collect();
        let files: Vec<(String, Vec<u8>)> = pages_out.iter().flat_map(|p| p.files.iter().cloned()).collect();
        let msg = format!("preview {slug}: {} ({}@{})", pages_out.iter().map(|p| p.stem.as_str()).collect::<Vec<_>>().join(", "), from.branch, from.commit.chars().take(9).collect::<String>());
        let ch = Change { slug: Some(slug.clone()), rows, files, assets: Some(Assets { viewer: assets.viewer.clone(), textures: assets.textures.clone(), css: assets.css.clone(), viewer_dir: assets.viewer_dir.clone(), textures_dir: assets.textures_dir.clone() }), older_than: a.older_than, msg };
        let t_push = Instant::now();
        let pushed = publish(&a.repo, &ch)?;
        if let Some(c) = &pushed {
            println!("pushed {} to {} in {:.1} s ({:.1} s since the start)", &c[..9.min(c.len())], a.repo, t_push.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64());
        }
        let first = format!("{preview_url}{slug}/{}.html", pages_out[0].stem);
        if !a.no_wait && pushed.is_some() {
            let secs = wait_live(&first, &format!("data-build=\"{}\"", from.build_id), t0, Duration::from_secs(600), &a.repo)?;
            println!("live in {secs:.1} s from the start of the command{}", if round > 1 { " (with the judge's numbers)" } else { "" });
        }
        for p in &pages_out {
            println!("{preview_url}{slug}/{}.html  ({})", p.stem, p.status);
        }
        println!("every preview: {preview_url}");
        if waiting.is_empty() || pages_out.iter().all(|p| !p.pending) {
            return Ok(());
        }
        // the judges that were still running: their numbers, then the page again
        println!("waiting for the judge of {} design(s), then the page again with its numbers", waiting.len());
        for (i, rx) in waiting.drain(..) {
            verdicts[i] = match rx.recv() {
                Ok(j) => Verdict::Judged(j),
                Err(_) => Verdict::None { why: "its judge stopped without a verdict".into(), pending: false },
            };
        }
        round += 1;
    }
}

/// `--out`: the preview as it would be served (its pages, the shared files, the index), written
/// to a folder instead of pushed.
#[allow(clippy::too_many_arguments)]
fn write_out(out: &Path, slug: &str, pages: &[Page], a: &Assets, site_url: &str, preview_url: &str, older_than: u32, from: &From, repo: &str) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut files: Vec<(String, Vec<u8>)> = base_files(site_url, preview_url, repo);
    files.extend(pages.iter().flat_map(|p| p.files.iter().cloned()));
    files.push((format!("css/{}/style.css", a.css), generate::STYLE.as_bytes().to_vec()));
    let rows: Vec<Vec<String>> = pages
        .iter()
        .map(|p| vec![slug.to_string(), format!("{slug}/{}.html", p.stem), p.title.clone(), p.status.clone(), from.time.clone(), from.branch.clone(), from.commit.chars().take(9).collect(), p.design_hash.clone(), a.viewer.clone(), a.textures.clone(), a.css.clone()])
        .collect();
    files.push(("index.html".into(), index_page(&rows, &a.css, site_url, older_than).into_bytes()));
    for (p, bytes) in &files {
        let to = out.join(p);
        if let Some(d) = to.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&to, bytes)?;
    }
    generate::copy_tree(&a.viewer_dir, &out.join("viewer").join(&a.viewer))?;
    generate::copy_tree(&a.textures_dir, &out.join("textures").join(&a.textures))?;
    // what a push would refuse
    let mut found = Vec::new();
    for (p, bytes) in files.iter().filter(|(p, _)| !p.ends_with(".litematic")) {
        for w in internal_names(&String::from_utf8_lossy(bytes)) {
            found.push(format!("{p}: {w}"));
        }
    }
    if !found.is_empty() {
        println!("WARNING: a push would refuse these internal names: {}", found.join("; "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_names_are_found_and_the_page_markup_is_not_one() {
        assert!(internal_names("<meta charset=\"utf-8\"><span class=\"fb-big\">x</span> metadata").is_empty());
        assert_eq!(internal_names("ran on devvm123.example"), vec!["devvm123".to_string()]);
        assert_eq!(internal_names("see D123456789 for it"), vec!["D123456789".to_string()]);
        assert_eq!(internal_names("Meta"), vec!["Meta".to_string()]);
        assert_eq!(internal_names("an fb note"), vec!["fb".to_string()]);
        assert_eq!(internal_names("session 12345678-90ab-cdef-1234-567890abcdef."), vec!["12345678-90ab-cdef-1234-567890abcdef".to_string()]);
        // a door design's name, a diff-like word with too few digits, a hash
        assert!(internal_names("D12345 c7bb35e36e4d077c").is_empty());
    }

    #[test]
    fn slugs_and_pages_urls() {
        assert_eq!(slug_of("Iron D1 bed plate!"), "iron-d1-bed-plate");
        assert_eq!(slug_of("  "), "preview");
        assert_eq!(pages_url("vjeux/redstonepedia-preview"), "https://vjeux.github.io/redstonepedia-preview/");
    }

    #[test]
    fn relink_moves_the_shared_files_and_sends_the_rest_to_the_site() {
        let a = Assets { viewer: "v1".into(), textures: "t1".into(), css: "c1".into(), viewer_dir: PathBuf::new(), textures_dir: PathBuf::new() };
        let html = format!(
            "<html data-mcview-textures=\"../{}\"><link rel=\"stylesheet\" href=\"../style.css?v=1\"><a class=\"logo\" href=\"../index.html\">R</a> <a id=\"tab-door\" href=\"../index.html#kind=door\">Doors <span class=\"count\">3</span></a><form action=\"../index.html\"></form><div data-src=\"../s/x.txt\"></div><a href=\"../s/x.litematic\">d</a><link rel=\"icon\" href=\"../favicon.svg\"><script type=\"module\">import init from \"../viewer/mcview.js?v=2\"; init({{module_or_path: \"../viewer/mcview_bg.wasm?v=3\"}});</script>",
            mcview::atlas::vanilla::SITE_DIR
        );
        let out = relink(&html, "s", &a, "https://site/").unwrap();
        assert!(out.contains("data-mcview-textures=\"../textures/t1\""));
        assert!(out.contains("href=\"../css/c1/style.css?v=1\""));
        assert!(out.contains("href=\"https://site/index.html\""));
        assert!(out.contains("href=\"https://site/index.html#kind=door\">Doors</a>"));
        assert!(out.contains("action=\"https://site/index.html\""));
        assert!(out.contains("data-src=\"../s/x.txt\""));
        assert!(out.contains("href=\"../s/x.litematic\""));
        assert!(out.contains("href=\"../favicon.svg\""));
        assert!(out.contains("\"../viewer/v1/mcview.js?v=2\"") && out.contains("\"../viewer/v1/mcview_bg.wasm?v=3\""));
    }
}
