//! `mctas site deploy`: the generated folder becomes ONE orphan commit on the gh-pages branch
//! (built with a temporary index, never touching the working tree or the current branch), pushed
//! with the token of `gh auth token` (the push recipe of AGENTS.md). GitHub Pages serves it at
//! https://vjeux.github.io/redstonepedia/.
//!
//! Two guards (several agents deploy): the checkout must contain the commit the current site was
//! built from (else a deploy from an older tree would take features away: merge first), and
//! GitHub Pages builds a branch about 10 times an hour at most (later builds error out), so a
//! deploy waits when 9 builds started in the last hour. After the push it follows the Pages
//! build to its end and fails when it errors.
//!
//! The push sends deltas: the site's commit is an orphan (no history for a clone to download),
//! and git deltas a pushed file only against the files of its commit's parents, so an orphan
//! alone re-sends every changed file whole. A viewer change rewrites the ~4435 pages' asset tags
//! and every deploy their footer's commit: 12.9 MB for a viewer-only change, all but the wasm
//! pages differing by a few bytes. So the push carries a second commit beside the site's, the
//! transport commit: the same tree with the live site as its parent, on a ref outside branches
//! and tags (`refs/mctas/<branch>-transport`: clones do not fetch it, no workflow runs). Git then
//! sends each changed page as a delta against its live version: 1.7 MB for the same change
//! (measured 2026-10-21). The ref keeps the previous site reachable until the next deploy
//! replaces it. Refused (a remote without atomic pushes or that namespace), the next attempt
//! pushes the site's commit alone, as before.

use crate::DeployArgs;
use anyhow::{Context, Result, bail};
use base64::Engine;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const PAGES_URL: &str = "https://vjeux.github.io/redstonepedia/";
/// The repository (vjeux 13:06Z: renamed from vjeux/minecraft-tas; GitHub redirects git, the web
/// and the API from the old name, not the old Pages site; local clones keep their folder names).
pub const REPO: &str = "vjeux/redstonepedia";
/// The site's path under vjeux.github.io (the repository's name): the 404 page links absolutely
/// (`pages::not_found`), and `site serve` strips it.
pub const SITE_PATH: &str = "/redstonepedia/";
/// GitHub Pages' soft limit is 10 builds an hour for a branch; keep one for the next deploy.
const BUILDS_PER_HOUR: usize = 9;

fn git(args: &[&str], env: &[(&str, &Path)], work_tree: Option<(&Path, &str)>) -> Result<String> {
    let mut c = Command::new("git");
    if let Some((w, git_dir)) = work_tree {
        // run inside the folder (so `.` is all of it) against the repository's git dir
        c.arg("-C").arg(w).arg("--git-dir").arg(git_dir).arg("--work-tree").arg(w);
    }
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().with_context(|| format!("git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn gh_json(path: &str) -> Result<serde_json::Value> {
    let o = Command::new("gh").args(["api", path]).output().context("running gh")?;
    if !o.status.success() {
        bail!("gh api {path}: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(serde_json::from_slice(&o.stdout)?)
}

/// Seconds since 1970 of `2026-10-20T00:42:19Z`.
pub fn parse_utc(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let d: Vec<i64> = date.split('-').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let t: Vec<i64> = time.split(':').map(|x| x.split('.').next().unwrap_or(x).parse().ok()).collect::<Option<_>>()?;
    if d.len() != 3 || t.len() < 2 {
        return None;
    }
    // days from the civil date (Howard Hinnant)
    let (y, m, day) = (if d[1] <= 2 { d[0] - 1 } else { d[0] }, d[1], d[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) * 86400 + t[0] * 3600 + t[1] * 60 + t.get(2).copied().unwrap_or(0))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The start times (seconds) of the Pages builds of the last hour.
fn builds_last_hour() -> Result<Vec<i64>> {
    let v = gh_json(&format!("repos/{REPO}/pages/builds?per_page=40"))?;
    let now = now_secs();
    Ok(v.as_array().map(|a| a.iter().filter_map(|b| b["created_at"].as_str().and_then(parse_utc)).filter(|t| now - t < 3600).collect()).unwrap_or_default())
}

/// The commit the current gh-pages was built from (its message: `... from <hash>`), if known.
fn deployed_source(remote: &str, branch: &str) -> Option<String> {
    git(&["fetch", "-q", remote, branch], &[], None).ok()?;
    let msg = git(&["log", "-1", "--format=%s", "FETCH_HEAD"], &[], None).ok()?;
    let hash = msg.rsplit(" from ").next()?.trim().to_string();
    (!hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit())).then_some(hash)
}

/// Guard 1: never deploy an older tree over a newer site: the checkout must contain the commit
/// the live site was built from.
fn guard_contains(a: &DeployArgs) -> Result<()> {
    if let Some(src) = deployed_source(&a.remote, &a.branch) {
        let known = git(&["cat-file", "-e", &format!("{src}^{{commit}}")], &[], None).is_ok();
        let contained = known && git(&["merge-base", "--is-ancestor", &src, "HEAD"], &[], None).is_ok();
        if !contained {
            bail!(
                "the live site was built from {src}, which this checkout does not contain{}: merge it first (git fetch && git merge {src}), rebuild, or --force",
                if known { "" } else { " (unknown here: fetch the branch it came from)" }
            );
        }
    }
    Ok(())
}

/// The ref of the transport commit (module docs): outside branches and tags.
fn transport_ref(branch: &str) -> String {
    format!("refs/mctas/{branch}-transport")
}

/// The live site's commit (none: the branch does not exist yet).
pub fn live_commit(remote: &str, branch: &str) -> Option<String> {
    git(&["fetch", "-q", remote, branch], &[], None).ok()?;
    git(&["rev-parse", "FETCH_HEAD^{commit}"], &[], None).ok()
}

/// The bytes a push of `want` sends to a remote that has `have`: the thin pack git push builds.
fn pack_bytes(want: &[&str], have: Option<&str>) -> Result<u64> {
    let mut c = Command::new("git").args(["pack-objects", "--revs", "--thin", "--stdout", "-q"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().context("git pack-objects")?;
    {
        let mut stdin = c.stdin.take().context("pack-objects stdin")?;
        for w in want {
            writeln!(stdin, "{w}")?;
        }
        if let Some(h) = have {
            writeln!(stdin, "^{h}")?;
        }
    }
    let mut out = c.stdout.take().context("pack-objects stdout")?;
    let n = std::io::copy(&mut out, &mut std::io::sink())?;
    if !c.wait()?.success() {
        bail!("git pack-objects failed");
    }
    Ok(n)
}

fn megabytes(n: u64) -> String {
    format!("{:.1} MB", n as f64 / 1e6)
}

/// What a push sent, from its progress lines: `8866 objects, 1.66 MiB`.
fn sent(stderr: &str) -> Option<String> {
    let line = stderr.split(['\r', '\n']).filter(|l| l.contains("Writing objects:")).last()?;
    let (counts, rest) = line.split_once("% (")?.1.split_once("), ")?;
    let size = rest.split([' ', ',']).take(2).collect::<Vec<_>>().join(" ");
    Some(format!("{} objects, {size}", counts.split('/').next_back().unwrap_or(counts)))
}

/// The remote is GitHub (a Pages build to follow, a build budget to respect); else a mirror,
/// e.g. a local test repository.
fn on_github(remote: &str) -> bool {
    git(&["remote", "get-url", remote], &[], None).unwrap_or_else(|_| remote.to_string()).contains("github.com")
}

/// `http.extraHeader=Authorization: Basic ...` for `git -c`: the push recipe of AGENTS.md, with
/// the token of `gh auth token` (`required`: fail without one; a local mirror needs none).
pub fn auth_header(required: bool) -> Result<String> {
    let token = Command::new("gh").args(["auth", "token"]).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if token.is_empty() && required {
        bail!("no GitHub token (`gh auth token`)");
    }
    Ok(format!("http.extraHeader=Authorization: Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("vjeux:{token}"))))
}

pub fn deploy(a: &DeployArgs) -> Result<()> {
    let dir = a.dir.canonicalize().with_context(|| format!("{}", a.dir.display()))?;
    if !dir.join("index.html").exists() || !dir.join("data/site.json").exists() {
        bail!("{} is not a generated site (no index.html / data/site.json): run `mctas site build` first", dir.display());
    }
    let git_dir = git(&["rev-parse", "--absolute-git-dir"], &[], None)?;
    let index = std::env::temp_dir().join(format!("mctas-site-index-{}", std::process::id()));
    let _ = std::fs::remove_file(&index);
    let env = [("GIT_INDEX_FILE", index.as_path())];
    git(&["add", "-A", "-f", "."], &env, Some((&dir, &git_dir)))?;
    let tree = git(&["write-tree"], &env, Some((&dir, &git_dir)))?;
    let _ = std::fs::remove_file(&index);
    let head = git(&["rev-parse", "--short=9", "HEAD"], &[], None).unwrap_or_default();
    let msg = format!("REDSTONEPEDIA: site built by `mctas site build` from {head}");
    let commit = git(&["commit-tree", &tree, "-m", &msg], &[], None)?;
    println!("gh-pages commit {commit} (tree {tree})");
    // the transport commit: the same tree with the live site as parent (module docs)
    let live = live_commit(&a.remote, &a.branch);
    let transport = match &live {
        Some(l) => Some(git(&["commit-tree", &tree, "-p", l, "-m", &format!("transport for {commit}: its tree with the live site as parent, so the push sends deltas")], &[], None)?),
        None => None,
    };
    // no page of the live site may go missing (a dead link: crates/site/published.tsv): a page
    // the folder lacks joins the list, and the build that follows writes its redirect
    let repo = crate::published::repo_root();
    if let Some(l) = &live {
        let missing: Vec<String> = crate::published::tree_pages(&crate::published::Git::here(), l)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| !dir.join(p).exists() && !crate::generate::DELETED_PAGES.contains(&p.as_str()))
            .collect();
        if !missing.is_empty() {
            let n = crate::published::merge_tree(&repo, l)?;
            let msg = format!(
                "{} pages of the live site would 404 (e.g. {}): {} lists them now ({n} new); rebuild (`mctas site build`) to write their redirects, then deploy",
                missing.len(),
                missing.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
                crate::published::FILE
            );
            if a.force {
                eprintln!("--force: {msg}");
            } else {
                bail!("{msg}");
            }
        }
    }
    if a.dry_run {
        if !a.force {
            guard_contains(a)?;
        }
        // what the push would send (a thin pack against the live site)
        let alone = pack_bytes(&[&commit], live.as_deref())?;
        match &transport {
            Some(t) => println!("push: {} with the transport commit ({} for the site's commit alone)", megabytes(pack_bytes(&[t, &commit], live.as_deref())?), megabytes(alone)),
            None => println!("push: {} (no live site to send deltas against)", megabytes(alone)),
        }
        println!("dry run: not pushed");
        return Ok(());
    }
    // guard 2: GitHub Pages' build rate
    if !a.force && on_github(&a.remote) {
        loop {
            let mut recent = builds_last_hour().unwrap_or_default();
            if recent.len() < BUILDS_PER_HOUR {
                break;
            }
            recent.sort();
            let free_at = recent[recent.len() - BUILDS_PER_HOUR] + 3600;
            let wait = (free_at - now_secs()).max(10);
            let minutes = (wait + 59) / 60;
            if !a.wait {
                bail!("{} Pages builds in the last hour (GitHub builds about 10 an hour; later ones error out): try again in {minutes} min, or pass --wait", recent.len());
            }
            println!("{} Pages builds in the last hour: waiting {minutes} min", recent.len());
            std::thread::sleep(std::time::Duration::from_secs(wait.min(600) as u64));
        }
        // guard 1, checked right before the push (another deploy may have landed while waiting)
        guard_contains(a)?;
    }
    let github = on_github(&a.remote);
    let header = auth_header(github)?;
    let refspec = format!("+{commit}:refs/heads/{}", a.branch);
    let tref = transport_ref(&a.branch);
    // GitHub sometimes rejects a large push once ("remote rejected ... (failed)"): retry; the
    // transport commit and the site's in one atomic push, then (refused) the site's alone
    let mut with_transport = transport.is_some();
    let mut last = String::new();
    let mut pushed = None;
    for attempt in 1..=3 {
        let mut args: Vec<String> = vec!["-c".into(), header.clone(), "push".into(), "--progress".into()];
        if with_transport {
            args.push("--atomic".into());
        }
        args.push(a.remote.clone());
        if let Some(t) = transport.as_ref().filter(|_| with_transport) {
            args.push(format!("+{t}:{tref}"));
        }
        args.push(refspec.clone());
        let t0 = std::time::Instant::now();
        let o = Command::new("git").args(&args).output()?;
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        if o.status.success() {
            pushed = Some((sent(&err), t0.elapsed().as_secs_f64(), with_transport));
            break;
        }
        last = err.trim().to_string();
        eprintln!("push attempt {attempt} failed: {}", last.split(['\r', '\n']).filter(|l| !l.trim().is_empty()).last().unwrap_or(""));
        if with_transport {
            eprintln!("  next: the site's commit alone (no transport commit)");
            with_transport = false;
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    let Some((what, secs, carried)) = pushed else { bail!("push failed: {last}") };
    println!(
        "pushed {} to {} {} ({}{secs:.1} s{})",
        commit,
        a.remote,
        a.branch,
        what.map(|w| format!("{w}, ")).unwrap_or_default(),
        if carried { ", deltas against the live site" } else { "" }
    );
    // what it published joins the list of published pages (the site carries its own list)
    match crate::published::merge_site(&repo, &dir) {
        Ok(0) => {}
        Ok(n) => println!("{}: {n} pages published for the first time: commit it", crate::published::FILE),
        Err(e) => eprintln!("{}: not updated: {e:#}", crate::published::FILE),
    }
    if a.enable_pages {
        enable_pages(&a.branch)?;
    }
    if github {
        follow_build(&commit)?;
        println!("{PAGES_URL}");
    } else {
        println!("(not GitHub: no Pages build to follow)");
    }
    Ok(())
}

/// Follow the Pages build of `commit` to its end (up to 6 minutes): fail when it errors.
fn follow_build(commit: &str) -> Result<()> {
    for _ in 0..72 {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let Ok(v) = gh_json(&format!("repos/{REPO}/pages/builds?per_page=10")) else { continue };
        let mine = v.as_array().and_then(|a| a.iter().find(|b| b["commit"].as_str() == Some(commit)).cloned());
        match mine.as_ref().and_then(|b| b["status"].as_str()) {
            Some("built") => {
                println!("Pages build of {}: built", &commit[..9.min(commit.len())]);
                return Ok(());
            }
            Some("errored") => {
                let why = mine.as_ref().and_then(|b| b["error"]["message"].as_str()).unwrap_or("");
                bail!("the Pages build of {} errored ({why}): a newer deploy or GitHub's build limit (about 10 an hour) may have superseded it", &commit[..9.min(commit.len())]);
            }
            _ => {}
        }
    }
    println!("Pages build of {}: still running after 6 min (check https://github.com/{REPO}/deployments)", &commit[..9.min(commit.len())]);
    Ok(())
}

/// Turn GitHub Pages on for `branch` (root folder) when it is off.
fn enable_pages(branch: &str) -> Result<()> {
    let probe = Command::new("gh").args(["api", &format!("repos/{REPO}/pages")]).output()?;
    if probe.status.success() {
        println!("GitHub Pages already on: {}", String::from_utf8_lossy(&probe.stdout).chars().take(300).collect::<String>());
        return Ok(());
    }
    let o = Command::new("gh")
        .args(["api", "-X", "POST", &format!("repos/{REPO}/pages"), "-f", &format!("source[branch]={branch}"), "-f", "source[path]=/"])
        .output()?;
    if !o.status.success() {
        bail!("enabling GitHub Pages failed: {} {}", String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim());
    }
    println!("GitHub Pages enabled on {branch}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_times() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-10-20T00:42:19Z"), Some(1792456939));
        assert_eq!(parse_utc("2026-10-20T00:42:19.123Z"), Some(1792456939));
        assert_eq!(parse_utc("nonsense"), None);
    }

    #[test]
    fn what_a_push_sent() {
        let err = "Enumerating objects: 13371, done.\nCounting objects: 100% (13371/13371), done.\rWriting objects:  50% (4433/8866), 0.80 MiB | 1 MiB/s\rWriting objects: 100% (8866/8866), 1.66 MiB | 10.20 MiB/s, done.\nTotal 8866 (delta 4433)";
        assert_eq!(sent(err).as_deref(), Some("8866 objects, 1.66 MiB"));
        assert_eq!(sent("Everything up-to-date"), None);
    }
}
