//! `mctas site build`: everything into one folder.

use crate::pages::{self, Scripts};
use crate::{BuildArgs, circuits, clocks, doors, web};
use anyhow::{Context, Result, bail};
use site_model::{Build, DATA_VERSION, Member, Site};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const STYLE: &str = include_str!("../assets/style.css");
pub const FAVICON: &str = include_str!("../assets/favicon.svg");
/// Pages vjeux deleted (2026-10-20 13:35Z: "I asked to remove the things from the header, it's
/// not to put them back in the footer. Delete these pages altogether"): the door boards, the
/// extendable families' index, the list of everything and the about page. `mctas site smoke`
/// fails if one is written or linked.
pub const DELETED_PAGES: [&str; 4] = ["boards.html", "families/index.html", "all.html", "about.html"];

/// The repository commit (HEAD), or `unknown`.
pub fn commit(repo: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Empty `out`, refusing to delete a folder that is not a previous site build.
fn fresh_dir(out: &Path) -> Result<()> {
    if out.exists() {
        let empty = std::fs::read_dir(out)?.next().is_none();
        if !empty && !out.join("data/site.json").exists() {
            bail!("{} exists and is not a site build (no data/site.json): refusing to replace it", out.display());
        }
        std::fs::remove_dir_all(out).with_context(|| format!("removing {}", out.display()))?;
    }
    std::fs::create_dir_all(out)?;
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Copy a directory tree into `to`, preserving relative paths (the vendored game
/// textures, with their `particle/` subdirectory).
pub fn copy_tree(from: &Path, to: &Path) -> Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))?.flatten() {
        let p = e.path();
        if p.is_dir() {
            n += copy_tree(&p, &to.join(e.file_name()))?;
        } else if p.is_file() {
            std::fs::copy(&p, to.join(e.file_name()))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Copy every regular file of `from` (not recursive) into `to`.
pub fn copy_files(from: &Path, to: &Path, ext: Option<&str>) -> Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))?.flatten() {
        let p = e.path();
        if !p.is_file() || ext.is_some_and(|x| p.extension().is_none_or(|pe| pe != x)) {
            continue;
        }
        std::fs::copy(&p, to.join(e.file_name()))?;
        n += 1;
    }
    Ok(n)
}

/// Copy `<git ref>:web/viewer/*` into `to` (git ls-tree + git show, bytes as committed).
pub fn viewer_from_ref(repo: &Path, r: &str, to: &Path) -> Result<usize> {
    let n = files_from_ref(repo, r, "web/viewer/", to)?;
    if !to.join("mcview.js").exists() {
        bail!("{r}: no web/viewer/mcview.js");
    }
    Ok(n)
}

/// Copy the files of `<git ref>:<folder>` (not recursive into subfolders' names: flattened) into `to`.
pub fn files_from_ref(repo: &Path, r: &str, folder: &str, to: &Path) -> Result<usize> {
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let o = std::process::Command::new("git").arg("-C").arg(repo).args(args).output()?;
        if !o.status.success() {
            bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
        }
        Ok(o.stdout)
    };
    let list = String::from_utf8(git(&["ls-tree", "-r", "--name-only", r, folder])?)?;
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for path in list.lines().filter(|l| !l.is_empty()) {
        let name = path.rsplit('/').next().unwrap_or(path);
        std::fs::write(to.join(name), git(&["show", &format!("{r}:{path}")])?)?;
        n += 1;
    }
    if n == 0 {
        bail!("{r}: nothing under {folder}");
    }
    Ok(n)
}

/// One card per mechanism: doors (the coordinator's decision (b): "near-identical variants become
/// one card") with the same category, box and open/close times, and clocks (critique C46:
/// identical pairs on the Clocks tab) with the same period, blocks, parts and output fold under
/// one twin (a door: its `host`, listed on the twin's page; a clock: the `twin` flag, hidden from
/// the default lists like the variants, listed among the twin's same-period clocks). A record
/// (a door leading a board, a family's base) is never folded. Returns (doors, clocks) folded.
fn fold_twins(builds: &mut [Build]) -> (usize, usize) {
    let mut doors: BTreeMap<(String, String, Option<u32>, Option<u32>, String, Vec<String>), Vec<usize>> = BTreeMap::new();
    let mut clocks: BTreeMap<(u32, u32, String, u32, u32), Vec<usize>> = BTreeMap::new();
    // what a builder picks a door by besides its numbers: a door with one of these never hides
    // under one without it
    const DOOR_PROPS: [&str; 7] = ["slime-free", "flat", "rotation-safe", "hallway", "eligible", "wall", "ground"];
    for (i, b) in builds.iter().enumerate() {
        if let Some(d) = &b.door {
            if d.host.is_none() {
                let props: Vec<String> = DOOR_PROPS.iter().filter(|p| b.flags.iter().any(|f| f == *p)).map(|p| p.to_string()).collect();
                doors.entry((b.group.clone(), b.dims.clone().unwrap_or_default(), d.open, d.close, d.seamless.clone(), props)).or_default().push(i);
            }
        }
        if let Some(c) = &b.clock {
            if c.host.is_none() && !c.instance && c.card.is_none() && !b.flags.iter().any(|f| f == "census" || f == "variant") {
                clocks.entry((c.period, b.blocks, b.group.clone(), c.on, c.edges)).or_default().push(i);
            }
        }
    }
    let mut folded = (0, 0);
    for mut same in doors.into_values().filter(|v| v.len() > 1) {
        same.sort_by_key(|&i| {
            let b = &builds[i];
            (b.door.as_ref().is_none_or(|d| d.leads.is_empty()), b.blocks, b.volume, b.name.clone())
        });
        let rep = same[0];
        let rep_id = builds[rep].id.clone();
        for &i in &same[1..] {
            if builds[i].door.as_ref().is_some_and(|d| !d.leads.is_empty()) {
                continue;
            }
            let id = builds[i].id.clone();
            if let Some(d) = builds[i].door.as_mut() {
                d.host = Some(rep_id.clone());
            }
            builds[i].flags.push("variant".into());
            if let Some(d) = builds[rep].door.as_mut() {
                d.variants.push(id);
            }
            folded.0 += 1;
        }
    }
    for mut same in clocks.into_values().filter(|v| v.len() > 1) {
        same.sort_by_key(|&i| (builds[i].name.starts_with("census-"), builds[i].volume, builds[i].name.clone()));
        for &i in &same[1..] {
            builds[i].flags.push("twin".into());
            folded.1 += 1;
        }
    }
    folded
}

/// A clock family member built at build time (its design under designs/clocks/members/, not in
/// the repository).
pub fn is_member(b: &Build) -> bool {
    b.design.starts_with("designs/clocks/members/")
}

/// A build's design text: a member's from the output folder, else the repository's.
fn read_design(b: &Build, repo: &Path, out: &Path) -> Result<String> {
    let path = if is_member(b) { out.join(&b.design) } else { repo.join(repo_design_path(b)) };
    std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
}

/// The text a build's thumbnail still frame is found in: a design shown from its own view file (a
/// display's lit still or a minecart line's chests), or a card's own still (a crop farm's field
/// grown), from that file, which the card loads; else its design file.
fn still_text(b: &Build, repo: &Path, out: &Path) -> Option<String> {
    if b.design.is_empty() {
        return None;
    }
    match b.circuit.as_ref().map(|c| if c.card_text.is_empty() { c.view_text.as_str() } else { c.card_text.as_str() }).filter(|t| !t.is_empty()) {
        Some(t) => Some(t.to_string()),
        None => read_design(b, repo, out).ok(),
    }
}

/// The thumbnails' still frames of these texts into the cache ([`still_text`]; the still-frame
/// pass then reads them): run while the clocks' pass waits on its long pole, its other threads
/// idle (2026-11-08: cold, the members' pass was one member long for its last ~65 s, and the
/// stills' 25 s came after it).
pub(crate) fn warm_stills(texts: &[String]) {
    crate::circuits::par_map(texts, |t| {
        let _ = crate::cache::get_or(crate::cache::Domain::View, &[b"still", t.as_bytes()], || crate::simulate::still(t));
    });
}

/// The repository path of a build's design file.
pub fn repo_design_path(b: &Build) -> String {
    match b.kind.as_str() {
        "door" => format!("{}/{}.txt", doors::archive_of(b), b.name),
        "circuit" => circuits::repo_path(b),
        _ => format!("{}/{}", clocks::ARCHIVE, b.design.rsplit('/').next().unwrap_or("")),
    }
}

/// The site path of the `.litematic` next to a design file (`designs/doors/x.txt` ->
/// `designs/doors/x.litematic`).
pub fn litematic_path(design: &str) -> String {
    match design.strip_suffix(".txt") {
        Some(stem) => format!("{stem}.litematic"),
        None => format!("{design}.litematic"),
    }
}

/// How to use a build once pasted, for the schematic's description.
fn usage_note(b: &Build) -> String {
    match (&b.door, &b.clock, &b.circuit) {
        (Some(_), _, _) => "Flip the lever to open the door, again to close it.".into(),
        (_, Some(c), _) => match c.start.strip_prefix("kick: last ") {
            Some(cell) => format!("To start it after pasting, break and re-place the block at {cell} (from the design's origin)."),
            None if c.start.starts_with("self") => "It starts by itself once pasted.".into(),
            None => format!("Start: {}.", c.start),
        },
        // what it does and its connections, in words (critique C31: "flying-machine: inputs ,
        // outputs ." printed empty lists)
        (_, _, Some(c)) => {
            let mut s = crate::plain::one_liner(c);
            let names = |v: &[String]| v.iter().map(|n| n.to_uppercase()).collect::<Vec<_>>().join(", ");
            match (c.inputs.is_empty(), c.outputs.is_empty()) {
                (false, false) => s.push_str(&format!(" Input {}, output {}.", names(&c.inputs), names(&c.outputs))),
                (false, true) => s.push_str(&format!(" Input {}.", names(&c.inputs))),
                (true, false) => s.push_str(&format!(" Output {}.", names(&c.outputs))),
                (true, true) => {}
            }
            s
        }
        _ => String::new(),
    }
}

/// `2026-10-19 20:07Z` (a build's archive time) in Unix milliseconds: a litematic's creation time.
/// The archive's, not the build's clock: an unchanged design writes the same file, so a deploy
/// sends only what changed (every build re-sent all 4417 schematics, each stamped with its `now`).
fn archived_ms(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once([' ', 'T']).unwrap_or((s, "00:00"));
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':').map(|x| x.parse::<i64>().ok());
    let (h, mi) = (t.next().flatten().unwrap_or(0), t.next().flatten().unwrap_or(0));
    // days from the civil date (Howard Hinnant's days_from_civil)
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + dd - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 24 + h) * 60 + mi) * 60_000)
}

/// Write `<design>.litematic` next to every build's and every member's design in the site.
fn write_litematics(site: &Site, repo: &Path, out: &Path) -> Result<usize> {
    // (a build without an archive time: the archive's newest, as stable)
    let newest = site.builds.iter().filter_map(|b| archived_ms(&b.archived)).max().unwrap_or(0);
    // every schematic on the build's threads (5038 of them took 7.6 s on one core, two thirds of
    // a warm build: 2026-11-08); the first failure fails the build
    let written = crate::circuits::par_map(&site.builds, |b| -> Result<()> {
        let time = archived_ms(&b.archived).unwrap_or(newest);
        let path = if is_member(b) { b.design.clone() } else { repo_design_path(b) };
        let text = read_design(b, repo, out)?;
        let d = mctas::design::Design::parse(&text).with_context(|| format!("parsing {path}"))?;
        let desc = [crate::explain::caption_text(&b.caption, crate::explain::ticks_matter(b)).as_str(), &usage_note(b), &format!("REDSTONEPEDIA: {}{}", crate::deploy::PAGES_URL, b.page)].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let bytes = litematic::export(&d, &litematic::Meta::new(&b.name, &desc, time)).with_context(|| format!("{path}: litematic"))?;
        write_bytes(&out.join(litematic_path(&b.design)), &bytes)
    });
    for r in written {
        r?;
    }
    let mut n = site.builds.len();
    // members: the generated ones (a member that is its family's archived base design shares that
    // build's file, already written); listed under their period on the clock card's page
    let clock_page = site.builds.iter().find(|b| b.clock.as_ref().is_some_and(|c| !c.kind_range.is_empty())).map(|b| b.page.clone()).unwrap_or_default();
    let build_designs: std::collections::BTreeSet<&str> = site.builds.iter().map(|b| b.design.as_str()).collect();
    let members: Vec<&Member> = site.members.iter().filter(|m| !m.design.is_empty() && !build_designs.contains(m.design.as_str())).collect();
    let written = crate::circuits::par_map(&members, |m| -> Result<()> {
        let text = std::fs::read_to_string(out.join(&m.design)).with_context(|| format!("reading {}", m.design))?;
        let d = mctas::design::Design::parse(&text).with_context(|| format!("parsing {}", m.design))?;
        let desc = format!(
            "A clock pulsing every {} gt ({}), {} blocks ({}), extended from a tested design (n = {}). REDSTONEPEDIA: {}{clock_page}#p{}",
            m.period,
            site_model::real_time(m.period),
            m.blocks,
            site_model::status_label(&m.status),
            m.n,
            crate::deploy::PAGES_URL,
            m.period
        );
        let name = format!("{}-n{}", m.family, m.n);
        let bytes = litematic::export(&d, &litematic::Meta::new(&name, &desc, newest)).with_context(|| format!("{}: litematic", m.design))?;
        write_bytes(&out.join(litematic_path(&m.design)), &bytes)
    });
    for r in written {
        r?;
    }
    n += members.len();
    Ok(n)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// Give the memory the allocator holds but no longer uses back to the system (glibc's
/// `malloc_trim`: a thread's arena otherwise stays at its high-water mark), between the build's
/// memory-heavy passes.
fn release_freed_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim takes no pointer and only returns free heap pages to the system.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// The memory a box must have available for the clocks' and the circuits' passes to run beside
/// each other: a cold build beside each other peaked at 41.6 GB (2026-11-08, 41.0 GB one after the
/// other: the circuits' pass runs while the members' pass is down to its long pole), with room for
/// the box's other agents.
const BESIDE_GB: u64 = 96;

/// Whether this box has the memory to run the two simulation passes beside each other (its
/// available memory, /proc/meminfo, at least [`BESIDE_GB`]); `site build` says which it chose.
fn overlap_passes() -> bool {
    let avail_kb = std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| m.lines().find_map(|l| l.strip_prefix("MemAvailable:").and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok()))).unwrap_or(0);
    let gb = avail_kb / 1_048_576;
    let beside = gb >= BESIDE_GB;
    eprintln!("simulation passes: {} ({gb} GB available; beside each other from {BESIDE_GB} GB; --serial-passes forces one after the other)", if beside { "the clocks' and the circuits' beside each other" } else { "one after the other" });
    beside
}

pub fn build(a: &BuildArgs) -> Result<()> {
    let repo = PathBuf::from(".");
    if !repo.join(doors::ARCHIVE).is_dir() || !repo.join(clocks::ARCHIVE).is_dir() {
        bail!("run from the repository root (no {} or {} here)", doors::ARCHIVE, clocks::ARCHIVE);
    }
    let t0 = std::time::Instant::now();
    // (every parallel pass of the build on `--threads`: the box is shared)
    crate::circuits::set_threads(a.threads);
    fresh_dir(&a.out)?;
    let out = &a.out;
    // the doors
    let release_assets = if a.release.is_empty() { Default::default() } else { piston_door::showcase::release_assets(&a.release) };
    if !a.release.is_empty() && release_assets.is_empty() {
        eprintln!("warning: no assets in release {} (gh): no video links", a.release);
    }
    let (door_builds, uncached, not_ours) = doors::export(&repo, &doors::DoorOptions { release_assets, release_tag: a.release.clone() })?;
    if !uncached.is_empty() {
        eprintln!("{} archived doors have no cached verdict from main's judge yet (left out): {}", uncached.len(), uncached.join(", "));
    }
    if !not_ours.is_empty() {
        let by_prov = repo.join(doors::ARCHIVE).join(doors::PROVENANCE).exists();
        let conflicts: Vec<&String> = not_ours.iter().filter(|(_, why)| why.starts_with("CONFLICT")).map(|(s, _)| s).collect();
        let unlisted: Vec<&String> = not_ours.iter().filter(|(_, why)| why.starts_with("not listed")).map(|(s, _)| s).collect();
        eprintln!(
            "{} archived doors left out as community work ({}; only our own designs are published)",
            not_ours.len(),
            if by_prov { "by provenance.tsv + the seed rule" } else { "no provenance.tsv yet: by the seed rule (community*, c79*, closed-b-fill*, satwire-c79*, satwire-63b*, c88*, fills of community designs)" }
        );
        if !conflicts.is_empty() {
            eprintln!(
                "  WARNING: {} doors marked ours in provenance.tsv match the seed rule and stay out (e.g. {}): fix provenance.tsv or the seed rule (site/src/doors.rs COMMUNITY_PREFIXES)",
                conflicts.len(),
                conflicts.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if !unlisted.is_empty() {
            eprintln!("  WARNING: {} archived doors are not in provenance.tsv and stay out until listed: {}", unlisted.len(), unlisted.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    // (the design files are copied after the one-per-kind selection: only published builds')
    eprintln!("doors: {} builds ({:.1}s)", door_builds.len(), t0.elapsed().as_secs_f64());
    // the clocks (no family pages: vjeux 00:33Z; but every clock that makes a period, family
    // members at their extension included, is listed under that period: 00:43Z, only the
    // members a judge verified, in the game or simulated to exactly their period), then the
    // circuits: one after the other on a box short of memory (the coordinator 2026-10-21: cold,
    // the members' pass peaked at 82 GB and the circuits' at 49 GB, 132 GB at once on a shared
    // 223 GB box where an out-of-memory kill takes agents' services down; one after the other,
    // ~82 GB, ~200 s cold instead of 148 s), beside each other on a box with the memory for both
    // (`overlap_passes`; 2026-11-08: the cold members' pass is one member long, clk-rt-counter
    // n = 3 alone for 339 s of its 359 s, and the circuits took 77 s after it; the whole cold
    // build peaked at 41 GB one after the other)
    let beside = !a.serial_passes && overlap_passes();
    let t = std::time::Instant::now();
    let clock_opts = clocks::ClockOptions { members_dir: Some(out.join("designs/clocks/members")), sim_horizon: a.sim_horizon, threads: a.threads };
    let (ce, circuits_done, clock_s, circuit_s) = if beside {
        std::thread::scope(|s| {
            let circuits_pass = s.spawn(|| {
                let t = std::time::Instant::now();
                let r = circuits::export(&repo);
                let secs = t.elapsed().as_secs_f64();
                // then, while the clocks' pass runs on, the doors', the circuits' and the archived
                // clocks' thumbnails' still frames (the members' after each member: clocks.rs)
                let mut texts: Vec<String> = door_builds.iter().filter_map(|b| still_text(b, &repo, out)).collect();
                if let Ok((cb, _)) = &r {
                    texts.extend(cb.iter().filter_map(|b| still_text(b, &repo, out)));
                }
                texts.extend(clocks::shown_texts(&repo));
                warm_stills(&texts);
                (r, secs)
            });
            let ce = clocks::export(&repo, &clock_opts);
            let clock_s = t.elapsed().as_secs_f64();
            let (r, circuit_s) = circuits_pass.join().unwrap_or_else(|_| (Err(anyhow::anyhow!("the circuits' pass panicked")), 0.0));
            (ce, r, clock_s, circuit_s)
        })
    } else {
        let ce = clocks::export(&repo, &clock_opts);
        let clock_s = t.elapsed().as_secs_f64();
        // the members' memory back to the system before the circuits run: glibc keeps each
        // thread's arena at its high-water mark, so the circuits' pass stacked on what the
        // members had freed (cold, 111 GB at the peak one after the other, 132 GB beside each
        // other)
        release_freed_memory();
        let t = std::time::Instant::now();
        let r = circuits::export(&repo);
        (ce, r, clock_s, t.elapsed().as_secs_f64())
    };
    let ce = ce?;
    let (circuit_builds, cnotes) = circuits_done?;
    release_freed_memory();
    eprintln!("clocks: {} builds ({clock_s:.1}s)", ce.builds.len());
    let base_of: BTreeMap<(String, String), String> = ce.cards.iter().flat_map(|c| c.families.iter().map(move |f| ((c.id.clone(), f.id.clone()), f.build.clone()))).collect();
    // each family's words and steps for its clocks' pages: its kind of clock, how one step extends
    // it, the n of its base, what one step adds (critique: an extended clock said only "a longer
    // version of <id>"; critique 41f1ec62a: the hopper clock's page never said its range)
    let fam_words: BTreeMap<String, site_model::Extend> = ce
        .cards
        .iter()
        .flat_map(|c| {
            c.families.iter().map(move |f| {
                let ext = site_model::Extend {
                    base: f.build.clone(),
                    title: c.title.clone(),
                    rule: f.rule.clone(),
                    base_n: f.base,
                    dt: f.period.eval(f.base + 1) - f.period.eval(f.base),
                    db: f.blocks.eval(f.base + 1) - f.blocks.eval(f.base),
                    hi: 0,
                };
                (f.id.clone(), ext)
            })
        })
        .collect();
    let mut clock_members: Vec<Member> = Vec::new();
    let mut dropped_members = 0;
    for mut m in ce.members {
        let verified = m.status == site_model::STATUS_JUDGED || m.status == site_model::STATUS_REDSIM;
        let generated = m.build.is_none() && m.design.starts_with("designs/clocks/members/");
        if verified && generated {
            m.base = base_of.get(&(m.card.clone(), m.family.clone())).cloned().unwrap_or_default();
            clock_members.push(m);
        } else if generated {
            // (an unverified member's design is not published)
            let _ = std::fs::remove_file(out.join(&m.design));
            dropped_members += 1;
        }
    }
    eprintln!("  note: clock family members: {} verified listed under their periods, {dropped_members} not verified left out", clock_members.len());
    for n in &ce.notes {
        eprintln!("  note: {n}");
    }
    if repo.join(clocks::TRACES).is_dir() {
        let n = copy_files(&repo.join(clocks::TRACES), &out.join("designs/clocks/traces"), Some("trace"))?;
        eprintln!("clock traces: {n} real-game recordings");
    }
    // the circuits (exported after the clocks, above)
    eprintln!("circuits: {} builds ({circuit_s:.1}s, after the clocks; {:.1}s so far)", circuit_builds.len(), t0.elapsed().as_secs_f64());
    for n in &cnotes {
        eprintln!("  note: {n}");
    }
    let mut builds = door_builds;
    builds.extend(ce.builds);
    builds.extend(circuit_builds);
    // one card per mechanism (the coordinator's decision (b), critique C46): doors and clocks
    // whose cards would read the same fold under one
    let (fd, fc) = fold_twins(&mut builds);
    eprintln!("  note: one card per mechanism: {fd} doors and {fc} clocks folded under a twin with the same numbers");
    // no extendable families: no family pages; the verified clock members only, listed under
    // their periods (vjeux 00:33Z, 00:43Z)
    let mut cards: Vec<site_model::Card> = Vec::new();
    let mut members: Vec<Member> = clock_members;
    // (and nothing says "comes in every size" any more)
    for b in &mut builds {
        b.flags.retain(|f| f != "extendable");
    }
    // one world-best design per kind (vjeux 21:47Z): one door card with every board's record;
    // every clock a card, grouped by period (01:24Z)
    for n in crate::kinds::select_doors(&mut builds).into_iter().chain(crate::kinds::select_clocks(&mut builds, &mut cards, &mut members)) {
        eprintln!("  note: {n}");
    }
    // the verified clock members are clocks of their own: a card, a page with its 3D view (vjeux
    // 01:24Z: "members get real pages again, just not family pages")
    let ids: BTreeMap<String, usize> = builds.iter().enumerate().map(|(i, b)| (b.id.clone(), i)).collect();
    let extra: Vec<Build> = members.iter().filter_map(|m| ids.get(&m.base).map(|&i| crate::kinds::member_build(m, &builds[i]))).collect();
    eprintln!("  note: {} verified clock family members as clocks of their own", extra.len());
    builds.extend(extra);
    members.clear();
    // every extended clock (a generated member or an archived one) and every family's base design
    // names its family: its base, its kind of clock, the rule, what a step adds and the slowest of
    // the family on the site; an extended clock's base must be on the site (no dead link, nothing
    // dropped silently: a missing base fails the build)
    let ids: std::collections::BTreeSet<String> = builds.iter().map(|b| b.id.clone()).collect();
    let mut slowest: BTreeMap<String, u32> = BTreeMap::new();
    for b in &builds {
        if let Some((f, p)) = b.clock.as_ref().and_then(|c| c.family.clone().map(|f| (f, c.every()))) {
            let e = slowest.entry(f).or_insert(0);
            *e = (*e).max(p);
        }
    }
    for b in builds.iter_mut() {
        let extended = b.flags.iter().any(|f| f == "extended");
        let id = b.id.clone();
        let Some(c) = b.clock.as_mut() else { continue };
        let Some(fam) = c.family.clone() else { continue };
        let Some(words) = fam_words.get(&fam) else {
            if extended {
                bail!("{id}: an extended clock without its family's words");
            }
            continue;
        };
        if extended && !ids.contains(&words.base) {
            bail!("{id}: its base design {} is not on the site", words.base);
        }
        if extended || words.base == id {
            c.extend = Some(site_model::Extend { hi: slowest.get(&fam).copied().unwrap_or(0), ..words.clone() });
        }
    }
    // what a lamp on each clock's output does (critique a417977c3), from the same run of the 3D
    // view's session as its chart (cached: src/cache.rs), every clock in parallel; and beside it
    // every thumbnail's still frame (webrender's data-still: the viewer's own still-frame search,
    // run here once per design, cached with the viewer's code; in the browser a long clock's
    // thumbnail took 3.6 s to find it, 0.1 s to play to it): two independent passes at once, the
    // stills' slowest design (a 5-minute member's, ~20 s) no longer waiting for the lamp check
    let t_view = std::time::Instant::now();
    let jobs: Vec<(usize, String)> = builds
        .iter()
        .enumerate()
        .filter(|(_, b)| b.clock.is_some())
        .map(|(i, b)| (i, read_design(b, &repo, out).unwrap_or_default()))
        .collect();
    let texts: Vec<(usize, String)> = builds.iter().enumerate().filter_map(|(i, b)| still_text(b, &repo, out).map(|t| (i, t))).collect();
    let (checks, stills) = std::thread::scope(|s| {
        let stills = s.spawn(|| crate::circuits::par_map(&texts, |(i, text)| (*i, crate::cache::get_or(crate::cache::Domain::View, &[b"still", text.as_bytes()], || crate::simulate::still(text)))));
        let checks: Vec<(usize, Option<crate::simulate::Lamp>)> = crate::circuits::par_map(&jobs, |(i, text)| (*i, builds[*i].clock.as_ref().and_then(|c| pages::clock_view(text, c).lamp)));
        (checks, stills.join())
    });
    let stills = stills.map_err(|_| anyhow::anyhow!("the still-frame pass panicked"))?;
    let found = stills.iter().filter(|(_, s)| s.attr.is_some()).count();
    eprintln!("  note: lamp check and thumbnail still frames ({found} of {} designs): {:.1}s", texts.len(), t_view.elapsed().as_secs_f64());
    // one card per mechanism and period (the coordinator 11:33Z, vjeux at 19 ticks: "The first 10
    // clocks look all the same to me?"): the clocks of one period whose pictures (at their
    // thumbnails' still frames) differ only by swappable blocks (a note block, a lamp or a copper
    // bulb; a plain or a sticky piston), turned or mirrored, fold under the smallest, which lists
    // them; their pages stay (crates/site/src/mechanism.rs)
    let text_of: BTreeMap<usize, &String> = texts.iter().map(|(i, t)| (*i, t)).collect();
    let mut keys: Vec<Vec<String>> = vec![Vec::new(); builds.len()];
    let mut door_keys: Vec<Vec<String>> = vec![Vec::new(); builds.len()];
    for (i, s) in &stills {
        if let Some(t) = text_of.get(i) {
            if builds[*i].clock.is_some() {
                keys[*i] = crate::mechanism::keys_of(s, t);
            } else if builds[*i].door.as_ref().is_some_and(|d| !d.leads.is_empty()) {
                door_keys[*i] = crate::mechanism::keys_of(s, t);
            }
        }
    }
    for (i, s) in stills {
        builds[i].still = s.attr;
    }
    let cards_before = builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_none())).count();
    let folded = crate::mechanism::fold(&mut builds, &keys);
    eprintln!("  note: clock mechanisms: {} cards for {cards_before} clocks ({folded} folded under the card of their period and mechanism)", cards_before - folded);
    // the doors alike (vjeux 13:10Z: "What about doors?"): board leaders of one mechanism and the
    // same numbers are one card leading all their boards
    let door_notes = crate::mechanism::fold_doors(&mut builds, &door_keys);
    let door_cards = builds.iter().filter(|b| b.door.as_ref().is_some_and(|d| !d.leads.is_empty() && d.host.is_none())).count();
    eprintln!("  note: door mechanisms: {door_cards} door cards ({} folded under another board leader)", door_notes.len());
    for n in door_notes {
        eprintln!("  note: {n}");
    }
    let (mut stays, mut misses, mut follows, mut relit, mut unexplained, mut entered) = (0, 0, 0, Vec::new(), Vec::new(), Vec::new());
    for (i, r) in checks {
        match r {
            Some(crate::simulate::Lamp::Fast(v, gap)) => {
                match v.as_str() {
                    "stays lit" => stays += 1,
                    "relit" => relit.push(format!("{} (off {gap} gt)", builds[i].name)),
                    _ => misses += 1,
                }
                if let Some(c) = builds[i].clock.as_mut() {
                    c.lamp = v;
                    c.lamp_gap = gap;
                }
            }
            Some(crate::simulate::Lamp::Follows(_)) => follows += 1,
            Some(crate::simulate::Lamp::Unexplained(gap)) => unexplained.push(format!("{} (off {gap} gt)", builds[i].name)),
            Some(crate::simulate::Lamp::Entered) => entered.push(builds[i].name.clone()),
            None => {}
        }
    }
    eprintln!(
        "  note: lamp check (a lamp in the output cell, simulated): {stays} clocks too fast for it (it stays lit), {misses} where it misses pulses, {follows} it follows; {} where it stays lit though off more than 4 gt, relit within the tick ({}); {} where it misses pulses for no reason we name (no claim made: {}); {} whose output cell blocks move through (no claim made, no chart: {})",
        relit.len(),
        relit.join(", "),
        unexplained.len(),
        unexplained.join(", "),
        entered.len(),
        entered.join(", ")
    );
    // no simulator attribution in the captions (vjeux 02:01Z)
    for b in &mut builds {
        b.caption = crate::explain::scrub_attribution(&b.caption);
    }
    // the curated cards (crates/site/src/cards.rs): an unknown field, a written-out number or a
    // failing check fails the build here
    let curated = crate::cards::apply(&repo, &mut builds)?;
    eprintln!("cards: {curated} designs with a curated card");
    // only the published builds' design files (the others stay in the repository): doors,
    // clocks and circuits alike (a member's was generated in place)
    for b in builds.iter().filter(|b| !is_member(b)) {
        let to = out.join(&b.design);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        let from = repo.join(repo_design_path(b));
        // (the player's header in place of the repository's notes: `pages::published_text`)
        let text = std::fs::read_to_string(&from).with_context(|| format!("reading {}", from.display()))?;
        write(&to, &pages::published_text(b, &text))?;
        // a display's lit still beside it (`Circuit::view_src`: its card and its view show it)
        if let Some(c) = b.circuit.as_ref().filter(|c| !c.view_src.is_empty() && !c.view_text.is_empty()) {
            write(&out.join(&c.view_src), &c.view_text)?;
        }
        // a card's own still (`Circuit::card_src`: a crop farm's field grown)
        if let Some(c) = b.circuit.as_ref().filter(|c| !c.card_src.is_empty() && !c.card_text.is_empty()) {
            write(&out.join(&c.card_src), &c.card_text)?;
        }
    }
    let site = Site { version: DATA_VERSION, generated: clock::catalog::now_utc(), commit: commit(&repo), builds, cards, members };
    // data: the door properties tables live on the build pages only (the UI never reads them)
    let mut slim = site.clone();
    for b in &mut slim.builds {
        if let Some(d) = &mut b.door {
            d.props.clear();
        }
        // an extended clock's caption is its base design's, word for word, and its family's
        // details are for its page: the index's data leaves them out (1.1 MB of its 5.5 MB; the
        // Clocks tab: the coordinator 06:07Z); it is still found by what it is made of
        if b.flags.iter().any(|f| f == "extended") {
            b.caption.clear();
        }
        if let Some(c) = &mut b.clock {
            c.extend = None;
        }
    }
    write(&out.join("data/site.json"), &serde_json::to_string(&slim)?)?;
    drop(slim);
    // the viewer (webrender's bundle: --viewer, --viewer-ref, else the checkout's web/viewer/)
    // and the UI
    let checkout_viewer = repo.join("web/viewer");
    let viewer_dir = a.viewer.clone().or_else(|| (a.viewer_ref.is_none() && !a.no_viewer && checkout_viewer.join("mcview.js").is_file()).then(|| checkout_viewer.clone()));
    let viewer = match (&viewer_dir, &a.viewer_ref) {
        _ if a.no_viewer => false,
        (Some(dir), _) => {
            let n = copy_files(dir, &out.join("viewer"), None)?;
            if !out.join("viewer/mcview.js").exists() {
                bail!("{}: no mcview.js in the viewer bundle", dir.display());
            }
            eprintln!("viewer: {n} files from {}", dir.display());
            true
        }
        (None, Some(r)) => {
            let n = viewer_from_ref(&repo, r, &out.join("viewer"))?;
            eprintln!("viewer: {n} files from {r}:web/viewer");
            true
        }
        (None, None) => false,
    };
    let app = if a.no_wasm {
        false
    } else {
        match (&a.pkg, &a.pkg_ref) {
            (Some(dir), _) => {
                copy_files(dir, &out.join("pkg"), None)?;
                web::check_copied_ui(&repo, &out.join("pkg"), &dir.display().to_string())?;
                true
            }
            (None, Some(r)) => {
                let n = files_from_ref(&repo, r, "pkg/", &out.join("pkg"))?;
                web::check_copied_ui(&repo, &out.join("pkg"), &format!("{r}:pkg"))?;
                eprintln!("UI: {n} files from {r}:pkg (not rebuilt)");
                true
            }
            (None, None) => {
                web::build_ui(&repo, &out.join("pkg"))?;
                true
            }
        }
    };
    // the game's textures, vendored in the repo (web/textures/<version>), served with the
    // site: the viewer fetches them same-origin. A build without them is not a site.
    let tex_from = repo.join(mcview::atlas::vanilla::DIR);
    if !tex_from.join(mcview::atlas::vanilla::ATLAS).is_file() {
        bail!("{}: no {} (the vendored game textures)", tex_from.display(), mcview::atlas::vanilla::ATLAS);
    }
    let n = copy_tree(&tex_from, &out.join(mcview::atlas::vanilla::SITE_DIR))?;
    eprintln!("textures: {n} files from {}", tex_from.display());
    let scripts = Scripts { viewer, app: false, ui: app };
    // pages
    write(&out.join("style.css"), STYLE)?;
    write(&out.join("favicon.svg"), FAVICON)?;
    // cache-busting: the pages load every bundle, the CSS and the data by a content hash
    pages::set_versions(pages::Versions {
        css: pages::bytes_hash(STYLE.as_bytes()),
        viewer_js: pages::file_hash(&out.join("viewer/mcview.js")),
        viewer_wasm: pages::file_hash(&out.join("viewer/mcview_bg.wasm")),
        ui_js: pages::file_hash(&out.join(format!("pkg/{}.js", web::UI_NAME))),
        ui_wasm: pages::file_hash(&out.join(format!("pkg/{}_bg.wasm", web::UI_NAME))),
        data: pages::file_hash(&out.join("data/site.json")),
    });
    // GitHub Pages: serve every file as is (no Jekyll)
    write(&out.join(".nojekyll"), "")?;
    write(&out.join("index.html"), &pages::index_page(&site, Scripts { viewer, app, ui: app }))?;
    write(&out.join("404.html"), &pages::not_found_page(&site))?;
    // pages vjeux deleted (13:35Z: "Delete these pages altogether"): never written, and gone from
    // an output folder an earlier build left them in
    for gone in DELETED_PAGES {
        let _ = std::fs::remove_file(out.join(gone));
    }
    // Litematica schematics of every build and every generated family member (a design that
    // cannot be exported fails the build: nothing is dropped silently)
    let n = write_litematics(&site, &repo, out)?;
    eprintln!("litematic: {n} schematics ({:.1}s)", t0.elapsed().as_secs_f64());
    // every build's page on the build's threads (beside the schematics, the other serial part of
    // a warm build)
    // the iron farm's components, so a full farm's page links to the ones it is built from
    let parts = crate::farms::load_parts(&repo);
    let written = crate::circuits::par_map(&site.builds, |b| -> Result<Vec<String>> {
        // (a member's design is the generated one; it has no repository file to link)
        let repo_path = if is_member(b) { String::new() } else { repo_design_path(b) };
        let text = read_design(b, &repo, out).unwrap_or_default();
        let html = pages::build_page(&site, b, &text, &repo_path, scripts);
        // what a visitor reads holds none of our internal names (the parent's ask, 2026-11-08:
        // "U1/U2, L2d, D1U, M2, redsim, mctas, gt"): every hit fails the build below
        let hits: Vec<String> = crate::explain::internal_tokens(&pages::visible_text(&html)).into_iter().map(|t| format!("{}: {t}", b.page)).collect();
        write(&out.join(&b.page), &html)?;
        Ok(hits)
    });
    let mut internal: Vec<String> = Vec::new();
    for r in written {
        internal.extend(r?);
    }
    if !internal.is_empty() {
        for h in &internal {
            eprintln!("  internal name: {h}");
        }
        bail!("internal names on {} page(s) a visitor reads (crates/site/src/explain.rs::internal_tokens; say it in the player's words or drop it): {}", internal.len(), internal.iter().take(20).cloned().collect::<Vec<_>>().join("; "));
    }
    // the generated members' design files as published too (read above as generated: their pages
    // and schematics are made; now the player's header in place of the repository's notes)
    for b in site.builds.iter().filter(|b| is_member(b)) {
        let path = out.join(&b.design);
        if let Ok(text) = std::fs::read_to_string(&path) {
            write(&path, &pages::published_text(b, &text))?;
        }
    }
    // the obvious per-kind URLs resolve (/doors/, /clocks/); /circuits/ is the studies' overview
    for (folder, title, kind) in [("doors", "Doors", "door"), ("clocks", "Clocks", "clock")] {
        write(&out.join(folder).join("index.html"), &pages::redirect_page(&site, "../", title, &format!("kind={kind}")))?;
    }
    let circuits_index = pages::circuits_index(&site, scripts);
    write(&out.join("circuits/index.html"), &circuits_index)?;
    let mut subs: Vec<String> = site.builds.iter().filter_map(|b| b.page.strip_prefix("circuits/").and_then(|r| r.split_once('/')).map(|(sub, _)| sub.to_string())).collect();
    subs.sort();
    subs.dedup();
    // (the overviews hold no internal names either: the same check as the design pages')
    let mut overviews: Vec<(String, String)> = vec![("circuits/index.html".into(), circuits_index)];
    for sub in &subs {
        let html = pages::study_page(&site, sub, scripts);
        write(&out.join("circuits").join(sub).join("index.html"), &html)?;
        overviews.push((format!("circuits/{sub}/index.html"), html));
    }
    // the Farms section (vjeux 2026-11-08 12:14): the farm types, the iron farm's hub with its
    // stages and components, the crop farms' collection, each component's page
    let farms_index = crate::farms::farms_index(&site, scripts);
    write(&out.join("farms/index.html"), &farms_index)?;
    overviews.push(("farms/index.html".into(), farms_index));
    let hub = crate::farms::iron_hub(&site, &parts, scripts);
    write(&out.join("farms/iron/index.html"), &hub)?;
    overviews.push(("farms/iron/index.html".into(), hub));
    if site.builds.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.study == "farm")) {
        let crops = pages::study_page(&site, "farm", scripts);
        write(&out.join("farms/crops/index.html"), &crops)?;
        overviews.push(("farms/crops/index.html".into(), crops));
    }
    for (part, _, _) in crate::farms::PARTS {
        let members: Vec<crate::farms::Member> = parts.iter().filter(|m| m.part == part).cloned().collect();
        if members.is_empty() {
            continue;
        }
        for m in &members {
            let from = repo.join(crate::farms::member_design("iron", m));
            let text = std::fs::read_to_string(&from).with_context(|| format!("reading {}", from.display()))?;
            write(&out.join(crate::farms::member_design("iron", m)), &text)?;
        }
        let path = crate::farms::part_page_path("iron", part);
        let html = crate::farms::part_page(&site, "iron", part, &members, scripts);
        write(&out.join(&path), &html)?;
        overviews.push((path, html));
    }
    let internal: Vec<String> = overviews.iter().flat_map(|(p, html)| crate::explain::internal_tokens(&pages::visible_text(html)).into_iter().map(move |t| format!("{p}: {t}"))).collect();
    if !internal.is_empty() {
        bail!("internal names on the overview pages (crates/site/src/explain.rs::internal_tokens): {}", internal.join("; "));
    }
    let mut by_card: BTreeMap<String, Vec<&Member>> = BTreeMap::new();
    for m in &site.members {
        by_card.entry(m.card.clone()).or_default().push(m);
    }
    for c in &site.cards {
        let ms = by_card.get(&c.id).cloned().unwrap_or_default();
        write(&out.join(&c.page), &pages::card_page(&site, c, &ms, scripts))?;
    }
    // every page ever published still answers (crates/site/published.tsv, the coordinator
    // 2026-10-21 11:05Z: vjeux hit a dead link to a design rejudged out): a redirect where this
    // build has no page, to the current page of the same kind; and the list in the site (the next
    // publish reads it back)
    let built = crate::published::built_pages(&site);
    let built_paths: std::collections::BTreeSet<String> = built.iter().map(|e| e.path.clone()).collect();
    let listed = crate::published::read(&repo.join(crate::published::FILE));
    let (redirects, by_what) = crate::published::write_redirects(&site, &repo, out, &listed, &built_paths)?;
    let mut all = listed.clone();
    let when = site.generated.replacen('T', " ", 1);
    crate::published::merge(
        &mut all,
        built.into_iter().map(|mut e| {
            e.first = when.clone();
            e
        }),
    );
    write(&out.join(crate::published::SITE_FILE), &crate::published::render(&all))?;
    eprintln!("  note: published pages: {} listed, {redirects} answered by a redirect ({})", all.len(), by_what.join(", "));
    eprintln!(
        "wrote {}: {} builds, {} family pages, {} members, viewer: {}, UI: {} ({:.1}s)",
        out.display(),
        site.builds.len(),
        site.cards.len(),
        site.members.len(),
        if viewer { "yes" } else { "no (text cards)" },
        if app { "yes" } else { "no" },
        t0.elapsed().as_secs_f64()
    );
    // the simulations reused and computed (src/cache.rs), and the cache kept from growing
    eprintln!("{}", crate::cache::report());
    crate::cache::prune();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::archived_ms;

    #[test]
    fn an_archive_time_is_a_fixed_instant() {
        assert_eq!(archived_ms("1970-01-01 00:00Z"), Some(0));
        // (date -u -d '2026-10-19 20:07' +%s; a leap day)
        assert_eq!(archived_ms("2026-10-19 20:07Z"), Some(1_792_440_420_000));
        assert_eq!(archived_ms("2024-02-29T23:59Z"), Some(1_709_251_140_000));
        assert_eq!(archived_ms(""), None);
    }
}
