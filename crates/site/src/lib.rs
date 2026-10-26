//! `mctas site ...`: REDSTONEPEDIA, the website of every build (docs/site.md).
//!
//! - `mctas site build --out site/` exports every build of the repository (the archived 3x3
//!   piston doors, the clocks and their extendable families, the circuit studies) with all its
//!   properties to `site/data/site.json`, writes one HTML page per build and per extendable
//!   family, copies the design files, builds the filtering UI (web/site: Rust compiled to
//!   WebAssembly with wasm-bindgen) into `site/pkg/` and copies webrender's 3D viewer bundle
//!   into `site/viewer/` when given one.
//! - `mctas site deploy --dir site/` publishes the folder as the `gh-pages` branch (GitHub
//!   Pages: https://vjeux.github.io/redstonepedia/).

pub mod audit;
pub mod cache;
pub mod circuits;
pub mod clocks;
pub mod deploy;
pub mod doors;
pub mod explain;
pub mod generate;
pub mod kinds;
pub mod mechanism;
pub mod pages;
pub mod plain;
pub mod published;
pub mod read;
pub mod serve;
pub mod simulate;
pub mod web;
pub mod webkit;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum SiteCmd {
    /// Generate the whole site into a folder: data/site.json, one page per build and per
    /// extendable family, the design files, the filtering UI (WebAssembly) and the 3D viewer.
    Build(BuildArgs),
    /// Publish a generated folder as the gh-pages branch (GitHub Pages), one orphan commit.
    Deploy(DeployArgs),
    /// Build and deploy, in one command: what to run after merging into main.
    Publish(BuildArgs),
    /// Download Chrome for Testing's headless shell into ~/.cache/mctas (for the optional smoke).
    Chrome,
    /// The circuits a builder has no reason to pick: another design of the same kind and main
    /// value is smaller and at least as fast, and the listed one only avoids a material (slime,
    /// honey, torches...). Both lists: strictly slower, and the same speed.
    Dominated {
        /// The repository (designs/circuits).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Also the designs crates/site/hidden.tsv already keeps off the site.
        #[arg(long)]
        all: bool,
        /// Append one crates/site/hidden.tsv line per design of the "nothing in its favour" list
        /// (`dominated: <winner> is N blocks smaller, same speed`) under a dated header; never a
        /// design the site keeps (a pick, a size or a variant). Rebuild mctas after: the file is
        /// compiled in.
        #[arg(long)]
        hide: bool,
    },
    /// One world-best design per kind (vjeux 21:47Z): every study's kinds, the chosen design (a
    /// parametric kind: its sizes and the widest family), and the other candidates.
    Kinds {
        /// The repository (designs/circuits).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Also the designs crates/site/hidden.tsv keeps off the site.
        #[arg(long)]
        all: bool,
        /// Print the repository file of every published design instead (each pick and every row
        /// of a parametric kind's table), one per line.
        #[arg(long)]
        paths: bool,
    },
    /// The Clocks tab's cards of one period in a generated site, each with the clocks folded under
    /// it (one card per mechanism: crates/site/src/mechanism.rs) and what they swap; without
    /// `--period`, one line per period: its cards and clocks (diff two builds' lines to see what
    /// a fold change did).
    Mechanisms {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        /// The period, in game ticks (as the Clocks tab groups them).
        #[arg(long)]
        period: Option<u32>,
        /// Without `--period`: compare with an earlier run's lines (saved output of this command)
        /// and print the periods whose card count changed, before -> after.
        #[arg(long)]
        before: Option<PathBuf>,
    },
    /// A generated page's text as a visitor reads it, in order (headings, paragraphs, lists, table
    /// rows; no 3D view, charts or design text): for a player's-eye review.
    Read {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        /// Every card's page on the Doors and Circuits tabs (one per kind).
        #[arg(long)]
        kinds: bool,
        /// Pages to read (paths in the site, repeatable).
        #[arg(long)]
        page: Vec<String>,
        /// Also the Details block (how it works in depth, the judge's test, measurements).
        #[arg(long)]
        details: bool,
    },
    /// Serve a generated folder on 127.0.0.1 for a local preview.
    Serve {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        #[arg(long, default_value_t = 8000)]
        port: u16,
    },
    /// The list of every page path the site ever published (crates/site/published.tsv: a removed
    /// or renamed design never 404s, `site build` writes a redirect at each path it does not
    /// build). Without a flag: a summary of the list.
    Published {
        /// The repository (crates/site/published.tsv).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Rebuild the list from every deploy GitHub Pages kept (its Pages builds' commits, fetched
        /// by hash into ~/.cache/mctas/site/pages.git), merged into the repository's.
        #[arg(long)]
        recover: bool,
        /// Merge the live site's pages (the remote's gh-pages) into the list.
        #[arg(long)]
        live: bool,
    },
    /// Check a generated site in a headless Chrome (`--dump-dom`): the UI loads the data, draws
    /// the cards, the time search expands families, facet links filter, build and family pages
    /// render. Exits non-zero on a failed check.
    Smoke {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        /// The headless Chrome (default: $CHROME, else ~/tools/chrome-headless-shell-linux64).
        #[arg(long)]
        chrome: Option<PathBuf>,
        /// Virtual time each page gets to run its scripts (ms).
        #[arg(long, default_value_t = 20_000)]
        budget: u32,
        /// Check the deployed site at this URL instead of serving `dir` (e.g.
        /// https://vjeux.github.io/redstonepedia/); `dir` still gives the expected answers.
        #[arg(long)]
        url: Option<String>,
        /// `chrome` (headless Chrome: every check) or `webkit` (WebKitGTK through WebKitWebDriver
        /// in Xvfb, desktop + phone: the 3D views of the main page, a door, a clock, a circuit
        /// and a family page must draw a real picture).
        #[arg(long, default_value = "chrome")]
        engine: String,
        /// `--engine webkit` only: instead of the checks, film these pages (paths in the site,
        /// repeatable): `--frames` screenshots of each page's first 3D view, `--interval` ms apart,
        /// desktop and phone, into $MCTAS_WEBKIT_SHOTS (default /tmp/mctas-webkit-shots).
        #[arg(long)]
        page: Vec<String>,
        #[arg(long, default_value_t = 6)]
        frames: u32,
        #[arg(long, default_value_t = 700)]
        interval: u64,
        /// With --page: press the buttons with these labels, in order, once the view is ready (a
        /// viewer's input button: `INP` flips a shulker machine's start lever; `20×` picks that
        /// speed in the speed dropdown), or `css:<selector>` (any element: a viewer's Record
        /// button is `css:[data-mcview-record]`).
        #[arg(long)]
        click: Vec<String>,
        /// With --page: read each page like a visitor instead of filming its 3D view: screenshots
        /// from the top, one window-full at a time (at most --frames), once the page settled
        /// (--interval ms after its first view is ready); prints its height and whatever makes it
        /// wider than the window.
        #[arg(long)]
        full: bool,
        /// With --page: only this window (`phone` or `desktop`).
        #[arg(long)]
        only: Option<String>,
        /// `--engine webkit` only: just the group pages' responsiveness (every tab of the index
        /// and every study's cards, scrolled through: a script must come back within 2 s at every
        /// step), desktop and phone (`--only` one); the full WebKit smoke runs it too.
        #[arg(long)]
        responsive: bool,
    },
}

#[derive(Args)]
pub struct BuildArgs {
    /// Output folder (created; an existing one must be a previous site build: it is replaced).
    #[arg(long, default_value = "site")]
    pub out: PathBuf,
    /// webrender's viewer bundle (a folder with mcview.js + mcview_bg.wasm), copied to viewer/.
    /// Without it the pages show the interim pictures or text cards where the 3D views go.
    #[arg(long)]
    pub viewer: Option<PathBuf>,
    /// Take the viewer bundle from a git ref instead: its `web/viewer/` files
    /// (e.g. `origin/explore/webrender`, or `HEAD` once merged).
    #[arg(long, conflicts_with = "viewer")]
    pub viewer_ref: Option<String>,
    /// No 3D viewer even when the checkout has one (`web/viewer/`, used by default).
    #[arg(long)]
    pub no_viewer: bool,
    /// A prebuilt filtering UI (wasm-bindgen output of web/site), copied to pkg/ instead of
    /// building it.
    #[arg(long)]
    pub pkg: Option<PathBuf>,
    /// Take the filtering UI from a git ref's `pkg/` instead of building it (e.g.
    /// `origin/gh-pages`, the last deploy: for a box without the wasm32 target or wasm-bindgen).
    #[arg(long, conflicts_with = "pkg")]
    pub pkg_ref: Option<String>,
    /// Skip the filtering UI (no WebAssembly build; the index then only links to the circuits).
    #[arg(long)]
    pub no_wasm: bool,
    /// GitHub release whose `<stem>.mp4` assets are the doors' videos (`gh`); empty: no links.
    #[arg(long, default_value = "videos")]
    pub release: String,
    /// List extendable family members from their formulas only: nothing generated or simulated.
    #[arg(long)]
    pub no_members: bool,
    /// redsim horizon (ticks) for checking each generated member's period; 0: no check.
    #[arg(long, default_value_t = 50_000)]
    pub sim_horizon: u32,
    /// Threads for generating and simulating members.
    #[arg(long, default_value_t = 32)]
    pub threads: usize,
    /// `publish` only: wait for GitHub's Pages build budget instead of failing.
    #[arg(long)]
    pub wait: bool,
    /// Accepted for old command lines: `publish` never runs the browser smoke any more (vjeux
    /// 2026-10-20 14:20Z: "Please remove the e2e tests"); `mctas site smoke` stays a dev tool.
    #[arg(long, hide = true)]
    pub no_smoke: bool,
}

#[derive(Args)]
pub struct DeployArgs {
    /// The generated site folder.
    #[arg(long, default_value = "site")]
    pub dir: PathBuf,
    #[arg(long, default_value = "gh-pages")]
    pub branch: String,
    #[arg(long, default_value = "origin")]
    pub remote: String,
    /// Build the commit but do not push.
    #[arg(long)]
    pub dry_run: bool,
    /// Also turn GitHub Pages on for the branch if it is off (`gh api`).
    #[arg(long)]
    pub enable_pages: bool,
    /// Wait for GitHub's build budget (about 10 Pages builds an hour) instead of failing.
    #[arg(long)]
    pub wait: bool,
    /// Skip the guards (the checkout must contain what the live site was built from; the hourly
    /// build budget).
    #[arg(long)]
    pub force: bool,
}

/// `mctas site dominated --hide`: appends to crates/site/hidden.tsv one line per design a sibling
/// beats with nothing in its favour (`audit::hide_lines`), under a dated header, leaving out the
/// designs the site keeps (the circuits' selection run on a copy: picks, sizes, variants) and the
/// lines the file already has; prints both.
fn hide_dominated(repo: &std::path::Path, builds: Vec<site_model::Build>) -> Result<()> {
    let mut sel = builds.clone();
    let _ = kinds::select_circuits(&mut sel);
    let kept: std::collections::BTreeSet<String> = sel.iter().map(|b| b.id.clone()).collect();
    let path = repo.join("crates/site/hidden.tsv");
    let mut text = std::fs::read_to_string(&path)?;
    let mut present: std::collections::BTreeSet<String> = text.lines().filter_map(|l| l.split('\t').next()).map(|p| p.trim_start_matches(['!', '~']).to_string()).collect();
    // until nothing more is dominated: a design folded under one hidden now stands on its own
    // and may be dominated too (cart-loader-t2 under t4, both beaten by t5)
    let (mut builds, mut lines, mut left) = (builds, Vec::new(), Vec::new());
    loop {
        let site = site_model::Site { builds: builds.clone(), ..Default::default() };
        let (new, why) = audit::hide_lines(&site, &kept, &present);
        for w in why {
            if !left.contains(&w) {
                left.push(w);
            }
        }
        if new.is_empty() {
            break;
        }
        let gone: std::collections::BTreeSet<String> = new.iter().filter_map(|l| l.split('\t').next()).map(|p| format!("circuits/{p}")).collect();
        present.extend(new.iter().filter_map(|l| l.split('\t').next()).map(String::from));
        builds.retain(|b| !gone.contains(&b.id));
        for b in &mut builds {
            if let Some(c) = b.circuit.as_mut()
                && c.host.as_ref().is_some_and(|h| gone.contains(h))
            {
                c.host = None;
                b.flags.retain(|f| f != "variant");
            }
        }
        lines.extend(new);
    }
    for l in &left {
        println!("not hidden: {l}");
    }
    if lines.is_empty() {
        println!("nothing to hide");
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!(
        "# {}, `mctas site dominated --hide`: designs a sibling of the same kind and value beats (fewer blocks, smaller by its\n# kind's own rule, as robust, at least as fast) with nothing in their favour, not even a material\n",
        clock::catalog::now_utc()
    ));
    for l in &lines {
        println!("hidden: {l}");
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&path, text)?;
    println!("{}: {} lines appended (rebuild mctas: the file is compiled in)", path.display(), lines.len());
    Ok(())
}

pub fn run(cmd: SiteCmd) -> Result<()> {
    match cmd {
        SiteCmd::Build(a) => generate::build(&a),
        SiteCmd::Deploy(a) => deploy::deploy(&a),
        SiteCmd::Publish(a) => {
            // build and deploy; no browser end-to-end gate (vjeux 14:20Z: "I think we're
            // overreacting to a single bug we had. Please remove the e2e tests"). The live site's
            // pages first join the list of published pages, so the build writes a redirect for any
            // it drops (crates/site/published.tsv: a list the publisher forgot to commit is never
            // lost, the live site carries it)
            let repo = published::repo_root();
            match deploy::live_commit("origin", "gh-pages") {
                Some(live) => {
                    let n = published::merge_tree(&repo, &live)?;
                    if n > 0 {
                        eprintln!("{}: {n} pages of the live site listed for the first time", published::FILE);
                    }
                }
                None => eprintln!("(no live site to read the published pages of)"),
            }
            generate::build(&a)?;
            deploy::deploy(&DeployArgs { dir: a.out.clone(), branch: "gh-pages".into(), remote: "origin".into(), dry_run: false, enable_pages: false, wait: a.wait, force: false })
        }
        SiteCmd::Published { repo, recover, live } => {
            if recover {
                published::recover(&repo)?;
            }
            if live {
                let c = deploy::live_commit("origin", "gh-pages").ok_or_else(|| anyhow::anyhow!("no live site (origin gh-pages)"))?;
                let n = published::merge_tree(&repo, &c)?;
                println!("{}: {n} pages of the live site listed for the first time", published::FILE);
            }
            let map = published::read(&repo.join(published::FILE));
            let mut by_what: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
            for e in map.values() {
                *by_what.entry(e.what.as_str()).or_default() += 1;
            }
            println!("{}: {} paths ({})", published::FILE, map.len(), by_what.iter().map(|(w, k)| format!("{k} {w}")).collect::<Vec<_>>().join(", "));
            Ok(())
        }
        SiteCmd::Chrome => serve::install_chrome().map(|_| ()),
        SiteCmd::Dominated { repo, all, hide } => {
            let (builds, _notes) = circuits::export_with(&repo, all)?;
            if hide {
                return hide_dominated(&repo, builds);
            }
            let site = site_model::Site { builds, ..Default::default() };
            print!("{}", audit::report(&site));
            Ok(())
        }
        SiteCmd::Kinds { repo, all, paths } => {
            let (builds, _notes) = circuits::export_with(&repo, all)?;
            if paths {
                print!("{}", kinds::paths(&builds, &repo));
            } else {
                print!("{}", kinds::report(&builds));
            }
            Ok(())
        }
        SiteCmd::Read { dir, kinds, page, details } => read::read(&dir, kinds, &page, details),
        SiteCmd::Serve { dir, port } => serve::serve(&dir, port),
        SiteCmd::Mechanisms { dir, period, before } => {
            let site: site_model::Site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
            let Some(period) = period else {
                // every period: cards (not folded) and clocks
                let mut per: std::collections::BTreeMap<u32, (usize, usize)> = std::collections::BTreeMap::new();
                for c in site.builds.iter().filter_map(|b| b.clock.as_ref()) {
                    let e = per.entry(c.every()).or_default();
                    e.1 += 1;
                    if c.host.is_none() {
                        e.0 += 1;
                    }
                }
                let (cards, clocks) = per.values().fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1));
                if let Some(path) = before {
                    // an earlier run's lines: `every P ticks: C cards for N clocks`
                    let old: std::collections::BTreeMap<u32, usize> = std::fs::read_to_string(&path)?
                        .lines()
                        .filter_map(|l| {
                            let rest = l.strip_prefix("every ")?;
                            let (p, rest) = rest.split_once(" ticks: ")?;
                            Some((p.parse().ok()?, rest.split_whitespace().next()?.parse().ok()?))
                        })
                        .collect();
                    let was: usize = old.values().sum();
                    let mut changed: Vec<(u32, usize, usize)> = per.iter().map(|(p, v)| (*p, old.get(p).copied().unwrap_or(0), v.0)).filter(|(_, a, b)| a != b).collect();
                    changed.sort_by_key(|(p, a, b)| (std::cmp::Reverse(a.abs_diff(*b)), *p));
                    println!("{was} -> {cards} cards ({} periods changed)", changed.len());
                    for (p, a, b) in changed {
                        println!("every {p} ticks: {a} -> {b} cards");
                    }
                    return Ok(());
                }
                println!("{cards} cards for {clocks} clocks over {} periods", per.len());
                for (p, (cards, clocks)) in per {
                    println!("every {p} ticks: {cards} cards for {clocks} clocks");
                }
                return Ok(());
            };
            let at: Vec<&site_model::Build> = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.every() == period)).collect();
            let cards: Vec<&&site_model::Build> = at.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_none())).collect();
            println!("every {period} ticks: {} cards for {} clocks", cards.len(), at.len());
            let mut cards = cards;
            cards.sort_by_key(|b| (b.blocks, b.volume, b.name.clone()));
            for b in cards {
                let c = b.clock.as_ref().unwrap();
                println!("{} ({} blocks, {}): {}", b.name, b.blocks, b.dims.clone().unwrap_or_default(), b.kinds.join(" "));
                for v in &c.variants {
                    if let Some(o) = site.builds.iter().find(|o| &o.id == v) {
                        let plus: Vec<&String> = o.kinds.iter().filter(|k| !b.kinds.contains(k)).collect();
                        let minus: Vec<&String> = b.kinds.iter().filter(|k| !o.kinds.contains(k)).collect();
                        println!("    {} ({} blocks): +{:?} -{:?}", o.name, o.blocks, plus, minus);
                    }
                }
            }
            Ok(())
        }
        SiteCmd::Smoke { dir, chrome, budget, url, engine, page, frames, interval, click, full, only, responsive } => {
            if responsive {
                if engine != "webkit" {
                    bail!("--responsive loads the pages in WebKit: add --engine webkit");
                }
                let fails = webkit::responsive(&dir, url.as_deref(), only.as_deref())?;
                if fails > 0 {
                    bail!("{fails} group pages not responsive");
                }
                return Ok(());
            }
            if !page.is_empty() {
                if engine != "webkit" {
                    bail!("--page films pages in WebKit: add --engine webkit");
                }
                if full {
                    return webkit::look(&dir, url.as_deref(), &page, frames, interval, &click, only.as_deref());
                }
                return webkit::film(&dir, url.as_deref(), &page, frames, interval, &click);
            }
            let fails = match engine.as_str() {
                "webkit" => webkit::smoke(&dir, url.as_deref())?,
                "chrome" => serve::smoke(&dir, chrome.as_deref(), budget, url.as_deref())?,
                other => bail!("--engine {other}: chrome or webkit"),
            };
            if fails > 0 {
                bail!("{fails} smoke check(s) failed");
            }
            println!("all smoke checks passed");
            Ok(())
        }
    }
}
