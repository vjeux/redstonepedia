//! The clocks: one build per clock type of designs/clocks/archive (the smallest design of each
//! type, as `mctas clock catalog` shows them), the dominance fold (variants), the extendable
//! families grouped into cards like CATALOG.md, and every member n of every family, generated
//! here with the family's own `extend generator` and simulated in redsim (the clock time search
//! expands families to these). The grouping mirrors crates/clock/src/catalog.rs::catalog.

use anyhow::{Context, Result};
use clock::catalog::{self, CardItem, Row};
use clock::extend::{self, Family as ExtFamily, Formula as ExtFormula, Instance};
use clock::Clock;
use site_model::{Build, Card, ClockInfo, Family, Formula, Member, STATUS_FORMULA, STATUS_JUDGED, STATUS_REDSIM};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ARCHIVE: &str = "designs/clocks/archive";
/// Real-game recordings for the viewer (`<design stem>.trace`, JSON Lines: designs/clocks/traces/README.md).
pub const TRACES: &str = "designs/clocks/traces";
/// The catalog's real-game pictures (`<stem>.jpg`, family members `<base stem>.n<k>.jpg`).
pub const IMG: &str = "designs/clocks/img";
/// Where the pictures are served from (linked, not copied: ~170 MB): the repository on GitHub.
pub const RAW_BASE: &str = "https://raw.githubusercontent.com/vjeux/redstonepedia/main/";

/// The URL of `designs/clocks/img/<stem>.jpg` when the repository has it.
fn picture(repo: &Path, stem: &str) -> Option<String> {
    let rel = format!("{IMG}/{stem}.jpg");
    repo.join(&rel).is_file().then(|| format!("{RAW_BASE}{rel}"))
}

/// The knobs of the clock export.
pub struct ClockOptions {
    /// Where generated member designs go (`<site>/designs/clocks/members`); `None`: members are
    /// listed from the formulas only, nothing generated or simulated.
    pub members_dir: Option<PathBuf>,
    /// redsim horizon for checking a generated member's period (ticks); 0: no check.
    pub sim_horizon: u32,
    /// Threads for generating and simulating members.
    pub threads: usize,
}

/// What the clock export produced.
pub struct ClockExport {
    pub builds: Vec<Build>,
    pub cards: Vec<Card>,
    pub members: Vec<Member>,
    /// Problems worth a look (a generator that failed, redsim disagreeing with a formula).
    pub notes: Vec<String>,
}

fn read_captions(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(catalog::CAPTIONS))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(f, _)| *f != "file")
        .map(|(f, c)| (f.to_string(), c.trim().to_string()))
        .collect()
}

fn stem(file: &str) -> &str {
    file.strip_suffix(".txt").unwrap_or(file)
}

pub fn formula(f: &ExtFormula) -> Formula {
    match *f {
        ExtFormula::Linear { a, b } => Formula::Linear { a, b },
        ExtFormula::Geometric { a, k, c } => Formula::Geometric { a, k, c },
    }
}

/// Width x height x depth and volume of a design's box.
fn dims_of(c: &Clock) -> (String, u32) {
    crate::simulate::dims_of(c)
}

/// A card's heading as CATALOG.md writes it: the group's `extend group period:`, else the one
/// formula its families share, else `a + bn gt, a in {...}`, else every formula.
fn heading(fams: &[&ExtFamily]) -> String {
    let mut texts: Vec<String> = Vec::new();
    for g in fams {
        if !texts.contains(&g.period.text()) {
            texts.push(g.period.text());
        }
    }
    let lin: Vec<(i64, i64)> = fams.iter().filter_map(|g| match g.period { ExtFormula::Linear { a, b } => Some((a, b)), _ => None }).collect();
    let h = match fams.iter().find_map(|g| g.group_period.clone()) {
        Some(gp) => gp,
        None if texts.len() == 1 => texts[0].clone(),
        None if lin.len() == fams.len() && lin.iter().all(|x| x.1 == lin[0].1) => {
            let mut a: Vec<i64> = lin.iter().map(|x| x.0).collect();
            a.sort();
            a.dedup();
            format!("a + {}n gt, a in {{{}}}", lin[0].1, a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
        }
        None => texts.join(" · "),
    };
    if h.ends_with(" gt") || h.contains(" gt,") { h } else { format!("{h} gt") }
}

/// A unique slug.
fn slug(s: &str, taken: &mut BTreeSet<String>) -> String {
    let base: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' }).collect();
    let mut id = base.clone();
    let mut k = 2;
    while !taken.insert(id.clone()) {
        id = format!("{base}-{k}");
        k += 1;
    }
    id
}

/// One family member to generate: (card id, family index, n).
struct Job {
    card: String,
    fam: usize,
    n: i64,
    /// Its estimated cost (blocks x ticks simulated): the costliest jobs start first.
    cost: i64,
}

pub fn export(repo: &Path, o: &ClockOptions) -> Result<ClockExport> {
    let dir = repo.join(ARCHIVE);
    let all_rows = catalog::read_rows(&dir)?;
    let caps = read_captions(&dir);
    let text_of = |file: &str| std::fs::read_to_string(dir.join(file)).unwrap_or_default();
    let composite: BTreeSet<String> = all_rows.iter().filter(|r| text_of(&r.file).lines().any(|l| l.trim_start().starts_with("# composite"))).map(|r| r.file.clone()).collect();
    // the catalog's own rule (`Row::shown`): a clock the coordinator ruled out of a normal survival
    // world (`hidden`: no output, command-only, daytime only...) or whose output pulses unevenly
    // (clockinfra's `gaps`: C38, "a clock on the site must deliver evenly spaced pulses at its
    // declared output") is not published; the repository keeps it
    let off_site = all_rows.iter().filter(|r| !r.shown()).count();
    // the one-edge rule (the coordinator 2026-10-21): a clock counts for period P only with one
    // rising edge per P; several evenly spaced edges per cycle list under their spacing
    // (`catalog::as_listed`, as `mctas clock catalog` lists them)
    let rows: Vec<Row> = all_rows.iter().filter(|r| !composite.contains(&r.file) && r.shown()).map(catalog::as_listed).collect();
    // each design's own cycle (its state period: extend instances are keyed by it)
    let cycle_of: BTreeMap<String, u32> = all_rows.iter().map(|r| (r.file.clone(), r.period)).collect();
    let type_of: BTreeMap<String, String> = {
        let keys = catalog::type_keys(&dir, &rows);
        rows.iter().map(|r| (r.file.clone(), keys.get(&r.file).cloned().unwrap_or_else(|| r.type_key.clone()))).collect()
    };
    let removed_aliases = catalog::read_aliases(&dir);
    // what a chunk reload does to each clock (clockinfra's `reload` column) and whether a builder
    // can take its output into a circuit (the judge's `usable` column), read by their headers so
    // the site follows the table as soon as a column fills in; empty: not checked
    let column_of = |name: &str| -> BTreeMap<String, String> {
        let text = std::fs::read_to_string(dir.join(catalog::VERDICTS)).unwrap_or_default();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        match (head.iter().position(|h| *h == "file"), head.iter().position(|h| *h == name)) {
            (Some(fi), Some(ri)) => lines
                .filter_map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    Some((f.get(fi)?.to_string(), f.get(ri)?.trim().to_string())).filter(|(_, r)| !r.is_empty())
                })
                .collect(),
            _ => BTreeMap::new(),
        }
    };
    let reload_of = column_of("reload");
    let usable_of = column_of("usable");
    let mut clocks: BTreeMap<String, Clock> = BTreeMap::new();
    for r in &all_rows {
        if let Ok(c) = Clock::load(&dir.join(&r.file)) {
            clocks.insert(r.file.clone(), c);
        }
    }
    let kinds_of = |file: &str| -> BTreeSet<String> { clocks.get(file).map(|c| c.kinds()).unwrap_or_default() };
    // extendable families (archived base designs with `extend` headers)
    let mut fams: Vec<(Row, ExtFamily)> = Vec::new();
    for r in &rows {
        if let Some(c) = clocks.get(&r.file) {
            if let Ok(Some(f)) = ExtFamily::of(c) {
                fams.push((r.clone(), f));
            }
        }
    }
    fams.sort_by(|a, b| (a.1.period.eval(a.1.base), &a.1.id).cmp(&(b.1.period.eval(b.1.base), &b.1.id)));
    let fam_files: BTreeSet<String> = fams.iter().map(|(r, _)| r.file.clone()).collect();
    // one design per type: the smallest (blocks, volume), family base designs first, then name
    let mut by_type: BTreeMap<(u32, String), Row> = BTreeMap::new();
    for r in &rows {
        let k = (r.period, type_of[&r.file].clone());
        let key = |x: &Row| (x.blocks, x.volume, !fam_files.contains(&x.file), x.file.clone());
        if by_type.get(&k).is_none_or(|b| key(r) < key(b)) {
            by_type.insert(k, r.clone());
        }
    }
    let instances: Vec<Instance> = extend::read_instances(&dir);
    let is_instance = |r: &Row| -> Option<(String, i64)> {
        if let Some((_, f)) = fams.iter().find(|(fr, _)| fr.file == r.file) {
            return Some((f.id.clone(), f.base));
        }
        let tk = &type_of[&r.file];
        let cycle = cycle_of.get(&r.file).copied().unwrap_or(r.period);
        instances.iter().find(|x| x.period == cycle && &x.type_key == tk).map(|x| (x.family.clone(), x.n))
    };
    // the dominance fold within each period, instances left out (as the catalog does)
    let periods: BTreeSet<u32> = rows.iter().map(|r| r.period).collect();
    let mut folded: BTreeMap<String, String> = BTreeMap::new();
    for p in &periods {
        let shown: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).filter(|r| is_instance(r).is_none()).collect();
        // as `mctas clock catalog` folds (palette alone is never a distinction)
        let items: Vec<(usize, i64, String)> = shown.iter().map(|r| (r.blocks, r.volume, r.file.clone())).collect();
        let robust: Vec<bool> = shown.iter().map(|r| r.robustness == "robust").collect();
        for (i, h) in catalog::fold_hosts_robust(&items, &robust, |a, b| catalog::clock_beats(shown[a], shown[b])).into_iter().enumerate() {
            if let Some(h) = h {
                folded.insert(shown[i].file.clone(), shown[h].file.clone());
            }
        }
    }
    // the cards
    let order: BTreeMap<&str, usize> = rows.iter().enumerate().map(|(i, r)| (r.file.as_str(), i)).collect();
    let fam_items: Vec<CardItem> = fams
        .iter()
        .map(|(r, f)| CardItem {
            formula: f.period.text(),
            kinds: kinds_of(&r.file),
            size: (f.blocks.eval(f.n_lo), r.volume, order.get(r.file.as_str()).copied().unwrap_or(usize::MAX)),
            group: f.group.clone(),
        })
        .collect();
    let mut groups = catalog::card_groups(&fam_items);
    groups.sort_by(|a, b| {
        let (fa, fb) = (&fams[a.members[0]].1, &fams[b.members[0]].1);
        (fa.period.eval(fa.base), &fa.id).cmp(&(fb.period.eval(fb.base), &fb.id))
    });
    let judged_of = |f: &ExtFamily| -> Vec<i64> {
        let mut v: Vec<i64> = instances.iter().filter(|x| x.family == f.id && x.period as i64 == f.period.eval(x.n)).map(|x| x.n).collect();
        v.sort();
        v.dedup();
        v
    };
    let mut taken = BTreeSet::new();
    let mut cards: Vec<Card> = Vec::new();
    let mut card_of_fam: BTreeMap<String, String> = BTreeMap::new();
    for g in &groups {
        let fs: Vec<&ExtFamily> = g.members.iter().map(|&i| &fams[i].1).collect();
        let first = &fams[g.members[0]];
        let title = if g.members.len() == 1 {
            first.1.id.clone()
        } else if let Some(t) = fs.iter().find_map(|f| f.group_title.clone()) {
            t
        } else if let Some(gr) = &g.group {
            gr.clone()
        } else {
            format!("{} (one block swapped: {} versions)", first.1.id, g.members.len())
        };
        let id = slug(g.group.as_deref().unwrap_or(&first.1.id), &mut taken);
        let swaps: Vec<Vec<String>> = g.members.iter().map(|&i| kinds_of(&fams[i].0.file).difference(&g.core).cloned().collect()).collect();
        let works_with = if g.members.len() > 1 && swaps.iter().all(|s| s.len() == 1) { swaps.iter().map(|s| s[0].replace('_', " ")).collect() } else { Vec::new() };
        let texts: BTreeSet<String> = fs.iter().map(|f| f.period.text()).collect();
        let families = g
            .members
            .iter()
            .map(|&i| {
                let (r, f) = &fams[i];
                card_of_fam.insert(f.id.clone(), id.clone());
                Family {
                    id: f.id.clone(),
                    build: format!("clocks/{}", stem(&r.file)),
                    period: formula(&f.period),
                    period_text: f.period.text(),
                    blocks: formula(&f.blocks),
                    blocks_text: f.blocks.text(),
                    rule: f.rule.trim_end_matches('.').to_string(),
                    n_lo: f.n_lo,
                    n_hi: f.n_hi,
                    base: f.base,
                    judged: judged_of(f),
                    extra: kinds_of(&r.file).difference(&g.core).cloned().collect(),
                }
            })
            .collect();
        cards.push(Card {
            id: id.clone(),
            title,
            heading: format!("P = {}", heading(&fs)),
            seconds: if texts.len() == 1 { first.1.period.seconds() } else { String::new() },
            page: format!("families/{id}.html"),
            core: g.core.iter().cloned().collect(),
            works_with,
            note: fs.iter().find_map(|f| f.group_note.clone()).unwrap_or_default(),
            families,
            ..Default::default()
        });
    }
    // the builds: one per type (+ every family base design, + composite clocks)
    let mut chosen: Vec<&Row> = by_type.values().collect();
    for (r, _) in &fams {
        if !chosen.iter().any(|c| c.file == r.file) {
            chosen.push(r);
        }
    }
    let comp_rows: Vec<&Row> = all_rows.iter().filter(|r| composite.contains(&r.file)).collect();
    chosen.extend(comp_rows);
    let mut variants_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (v, h) in &folded {
        variants_of.entry(h.clone()).or_default().push(format!("clocks/{}", stem(v)));
    }
    let mut builds: Vec<Build> = Vec::new();
    for r in chosen {
        let st = stem(&r.file).to_string();
        let tk = type_of.get(&r.file);
        let mut aliases: Vec<String> = match tk {
            Some(tk) => rows.iter().filter(|o| o.file != r.file && o.period == r.period && type_of.get(&o.file) == Some(tk) && !fam_files.contains(&o.file)).map(|o| stem(&o.file).to_string()).collect(),
            None => Vec::new(),
        };
        aliases.extend(removed_aliases.get(&r.file).cloned().unwrap_or_default());
        aliases.sort();
        aliases.dedup();
        let inst = if composite.contains(&r.file) { None } else { is_instance(r) };
        let is_base = fam_files.contains(&r.file);
        let host = folded.get(&r.file).map(|h| format!("clocks/{}", stem(h)));
        let mut flags: Vec<String> = Vec::new();
        if r.robustness == "robust" {
            flags.push("robust".into());
        }
        if r.start.starts_with("self") {
            flags.push("self-starting".into());
        } else if r.start.starts_with("kick") {
            flags.push("kicked".into());
        }
        if r.chiral {
            flags.push("chiral".into());
        }
        if inst.is_some() {
            flags.push("extendable".into());
        }
        if host.is_some() {
            flags.push("variant".into());
        }
        if composite.contains(&r.file) {
            flags.push("composite".into());
        }
        let (fam_id, n) = match &inst {
            Some((f, n)) => (Some(f.clone()), Some(*n)),
            None => (None, None),
        };
        // a real-game recording for the viewer's replay (entities and fluids redsim does not model)
        let trace = repo.join(TRACES).join(format!("{st}.trace")).is_file().then(|| format!("designs/clocks/traces/{st}.trace"));
        if trace.is_some() {
            flags.push("traced".into());
        }
        builds.push(Build {
            id: format!("clocks/{st}"),
            kind: "clock".into(),
            group: r.family.clone(),
            name: st.clone(),
            page: format!("clocks/{st}.html"),
            design: format!("designs/clocks/{}", r.file),
            blocks: r.blocks as u32,
            volume: Some(r.volume.max(0) as u32),
            dims: Some(r.dims.clone()).filter(|d| !d.is_empty()),
            kinds: kinds_of(&r.file).into_iter().collect(),
            caption: caps.get(&r.file).cloned().unwrap_or_default(),
            flags,
            door: None,
            clock: Some(ClockInfo {
                period: r.period,
                // with N equal gaps g between the output's rising edges, the output pulses every
                // g ticks: the builder's period (clockinfra: "show that as the clock's time")
                pulse_every: catalog::parse_gaps(&r.gaps).filter(|g| !g.is_empty() && !catalog::uneven(g)).map(|g| g[0]).filter(|g| *g > 0 && *g < r.period),
                reload: reload_of.get(&r.file).cloned().unwrap_or_default(),
                usable: usable_of.get(&r.file).cloned().unwrap_or_default(),
                kind_range: String::new(),
                warmup: r.warmup,
                on: r.on,
                edges: r.edges,
                pulses: r.pulses.clone(),
                robustness: r.robustness.clone(),
                start: r.start.clone(),
                chiral: r.chiral,
                aliases,
                host,
                variants: variants_of.get(&r.file).cloned().unwrap_or_default(),
                card: fam_id.as_ref().and_then(|f| card_of_fam.get(f).cloned()),
                family: fam_id,
                n,
                composite: composite.contains(&r.file),
                instance: inst.is_some() && !is_base,
                ..Default::default()
            }),
            circuit: None,
            trace,
            // (found at the end of the build, for every published build: generate.rs)
            still: None,
            picture: picture(repo, &st),
            archived: site_model::normalize_time(&crate::doors::archived_at(&text_of(&r.file))),
        });
    }
    builds.sort_by(|a, b| {
        let p = |x: &Build| x.clock.as_ref().map(|c| c.every()).unwrap_or(0);
        (p(a), a.blocks, a.volume, &a.name).cmp(&(p(b), b.blocks, b.volume, &b.name))
    });
    // the members of every family
    let mut notes = Vec::new();
    let build_of_member: BTreeMap<(String, i64), String> = builds
        .iter()
        .filter_map(|b| {
            let c = b.clock.as_ref()?;
            Some(((c.family.clone()?, c.n?), b.id.clone()))
        })
        .collect();
    let mut jobs: Vec<Job> = Vec::new();
    for card in &cards {
        for f in &card.families {
            let fi = fams.iter().position(|(_, ef)| ef.id == f.id).context("card family")?;
            // the verified range, and every n judged in the game past it (archive/extend.tsv:
            // `clock obs at-periods --record` judges the members at the periods players ask for,
            // 1 min, 5 min: a time search finds them as members)
            let mut ns: Vec<i64> = (f.n_lo..=f.n_hi).collect();
            ns.extend(instances.iter().filter(|x| x.family == f.id && (x.n < f.n_lo || x.n > f.n_hi) && fams[fi].1.period.eval(x.n) == x.period as i64).map(|x| x.n));
            ns.sort();
            ns.dedup();
            for n in ns {
                let ef = &fams[fi].1;
                let p = ef.period.eval(n);
                // (a run lasts about a warm-up and two periods, as long as the horizon at most)
                let cost = ef.blocks.eval(n).max(1).saturating_mul((2 * p).clamp(1, o.sim_horizon.max(1) as i64));
                jobs.push(Job { card: card.id.clone(), fam: fi, n, cost });
            }
        }
    }
    let results = run_jobs(&jobs, &fams, &dir, o);
    let mut members = Vec::new();
    for (job, res) in jobs.iter().zip(results) {
        let (r, f) = &fams[job.fam];
        let want_p = f.period.eval(job.n);
        let want_b = f.blocks.eval(job.n);
        let judged = instances.iter().any(|x| x.family == f.id && x.n == job.n && x.period as i64 == want_p);
        let (design, dims, volume, status, run, measured) = match res {
            Gen::Base => {
                let c = clocks.get(&r.file);
                let (d, v) = c.map(dims_of).unzip();
                // the base: its archived row's builder period (the one-edge rule)
                let m = catalog::builder_period(r);
                (format!("designs/clocks/{}", r.file), d, v, if judged || job.n == f.base { STATUS_JUDGED } else { STATUS_FORMULA }, None, m)
            }
            Gen::Formula => (String::new(), None, None, if judged { STATUS_JUDGED } else { STATUS_FORMULA }, None, None),
            Gen::Made { rel, dims, volume, sim, blocks, run, measured } => {
                if blocks as i64 != want_b {
                    notes.push(format!("{} n = {}: {} blocks, the formula says {}", f.id, job.n, blocks, want_b));
                }
                let status = if judged {
                    STATUS_JUDGED
                } else {
                    // redsim confirms a member when its OUTPUT gives a builder the claimed period
                    // (the one-edge rule), not only its whole state
                    match (measured, sim) {
                        (Some(m), _) if m as i64 == want_p => STATUS_REDSIM,
                        (Some(_), _) => STATUS_FORMULA,
                        (None, Some(p)) => {
                            notes.push(format!("{} n = {}: redsim finds a state period of {} gt but no even output, the formula says {}", f.id, job.n, p, want_p));
                            STATUS_FORMULA
                        }
                        (None, None) => STATUS_FORMULA,
                    }
                };
                (rel, Some(dims), Some(volume), status, run, measured)
            }
            Gen::Failed(why) => {
                notes.push(format!("{} n = {}: generator failed: {why}", f.id, job.n));
                continue;
            }
        };
        // the member's period is the one measured at its output (the coordinator 2026-10-23), the
        // formula's only when redsim measured none; a member whose output gives another period
        // than its family claims is said so
        let period = match measured {
            Some(m) if m as i64 != want_p => {
                notes.push(format!("{} n = {}: its output gives a builder a period of {m} gt, the family claims {want_p}: listed under {m}", f.id, job.n));
                m
            }
            Some(m) => m,
            None => want_p.max(0) as u32,
        };
        members.push(Member {
            run,
            card: job.card.clone(),
            family: f.id.clone(),
            n: job.n,
            period,
            blocks: want_b.max(0) as u32,
            design,
            dims,
            volume,
            status: status.to_string(),
            build: build_of_member.get(&(f.id.clone(), job.n)).cloned(),
            // the base design's picture, or the catalog's picture of this member (n_lo + 1)
            picture: {
                let base_stem = stem(&r.file);
                if job.n == f.base { picture(repo, base_stem) } else { picture(repo, &format!("{base_stem}.n{}", job.n)) }
            },
            axis: String::new(),
            base: String::new(),
        });
    }
    // the census clocks (named `census-...`, captions that are the census tool's log) leave the
    // default lists and search unless one is THE record of its period (the smallest; a clock a
    // builder can wire wins a tie (the coordinator, 2026-10-21), then a named clock) (the
    // coordinator's decision (a)); their pages stay, the other clocks of a period link them
    let mut record: BTreeMap<u32, (u32, bool, bool, u32, String)> = BTreeMap::new();
    for b in &builds {
        if let Some(c) = &b.clock {
            if c.host.is_none() && !c.instance {
                let key = (b.blocks, !c.usable.starts_with("yes"), b.name.starts_with("census-"), b.volume.unwrap_or(u32::MAX), b.name.clone());
                let e = record.entry(c.every()).or_insert_with(|| key.clone());
                if key < *e {
                    *e = key;
                }
            }
        }
    }
    let mut folded = 0;
    for b in builds.iter_mut() {
        let Some(c) = &b.clock else { continue };
        let is_record = record.get(&c.every()).is_some_and(|r| r.4 == b.name);
        if b.name.starts_with("census-") && c.card.is_none() && !is_record && !b.flags.iter().any(|f| f == "variant") {
            b.flags.push("census".into());
            folded += 1;
        }
    }
    notes.push(format!("clocks: {folded} census clocks off the default lists (not the record of their period)"));
    notes.push(format!("clocks: {off_site} archived clocks not published (the catalog's rule: ruled out of survival, or an uneven output)"));
    Ok(ClockExport { builds, cards, members, notes })
}

/// A generated member.
enum Gen {
    /// n is the base design itself: the archived file.
    Base,
    /// Not generated (`--no-members`, or a family without a generator): the formula only.
    Formula,
    Made { rel: String, dims: String, volume: u32, sim: Option<u32>, blocks: usize, run: Option<site_model::MemberRun>, measured: Option<u32> },
    Failed(String),
}

fn run_one(job: &Job, fams: &[(Row, ExtFamily)], dir: &Path, o: &ClockOptions) -> Gen {
    let (r, f) = &fams[job.fam];
    if job.n == f.base {
        return Gen::Base;
    }
    let Some(out_dir) = &o.members_dir else { return Gen::Formula };
    if f.generator.is_empty() {
        return Gen::Formula;
    }
    let tmp = std::env::temp_dir().join(format!("mctas-site-{}-{}", std::process::id(), job.fam));
    let base_path = dir.join(&r.file);
    let base_text = std::fs::read_to_string(&base_path).unwrap_or_default();
    // generated and simulated once per base design, generator, n and horizon (and the code that
    // does it: the clock crates, and this generator's own crate): src/cache.rs
    let made = crate::cache::get_or_try(
        crate::cache::Domain::Members,
        &[crate::cache::generator_code(&f.generator).as_bytes(), base_text.as_bytes(), f.generator.as_bytes(), f.id.as_bytes(), &job.n.to_le_bytes(), &o.sim_horizon.to_le_bytes()],
        || crate::simulate::generate_member(f, &base_path, job.n, o.sim_horizon, &tmp),
    );
    let m = match made {
        Ok(m) => m,
        Err(e) => return Gen::Failed(e),
    };
    let name = format!("{}-n{}.txt", f.id, job.n);
    if let Err(e) = std::fs::create_dir_all(out_dir).and_then(|_| std::fs::write(out_dir.join(&name), &m.text)) {
        return Gen::Failed(format!("{e}"));
    }
    // its thumbnail's still frame into the cache now, beside the pass's long pole (the still-frame
    // pass reads it: generate.rs `warm_stills`)
    crate::generate::warm_stills(std::slice::from_ref(&m.text));
    Gen::Made { rel: format!("designs/clocks/members/{name}"), dims: m.dims, volume: m.volume, sim: m.sim, blocks: m.blocks, run: m.run, measured: m.measured }
}

/// The archived clocks the site shows (the catalog's rule, composites aside), their design texts:
/// their thumbnails' still frames are found while the members' pass runs (generate.rs).
pub fn shown_texts(repo: &Path) -> Vec<String> {
    let dir = repo.join(ARCHIVE);
    let Ok(rows) = catalog::read_rows(&dir) else { return Vec::new() };
    rows.iter()
        .filter(|r| r.shown())
        .filter_map(|r| std::fs::read_to_string(dir.join(&r.file)).ok())
        .filter(|t| !t.lines().any(|l| l.trim_start().starts_with("# composite")))
        .collect()
}

/// Generate (and simulate) every member, in parallel, the costliest first (a long tail of one big
/// member finished last otherwise: dropring-1d-r1 n = 426, 866 blocks, alone took 3 minutes of a
/// 3.3-minute pass); results in job order.
fn run_jobs(jobs: &[Job], fams: &[(Row, ExtFamily)], dir: &Path, o: &ClockOptions) -> Vec<Gen> {
    let threads = o.threads.max(1).min(jobs.len().max(1));
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(jobs[i].cost));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Gen>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let took: std::sync::Mutex<Vec<(f64, String)>> = std::sync::Mutex::new(Vec::new());
    let t0 = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&i) = order.get(k) else { break };
                    let t = std::time::Instant::now();
                    let g = run_one(&jobs[i], fams, dir, o);
                    took.lock().unwrap().push((t.elapsed().as_secs_f64(), format!("{} n = {}", fams[jobs[i].fam].1.id, jobs[i].n)));
                    *slots[i].lock().unwrap() = Some(g);
                }
            });
        }
    });
    // (a long pass says what took the time: the cache's reuse is in the build's last lines)
    if t0.elapsed().as_secs_f64() > 30.0 {
        let mut took = took.into_inner().unwrap();
        took.sort_by(|a, b| b.0.total_cmp(&a.0));
        eprintln!(
            "  note: clock members: {:.0} s, {:.0} thread-s; slowest {}",
            t0.elapsed().as_secs_f64(),
            took.iter().map(|t| t.0).sum::<f64>(),
            took.iter().take(4).map(|(t, n)| format!("{n} ({t:.0} s)")).collect::<Vec<_>>().join(", ")
        );
    }
    slots.into_iter().map(|m| m.into_inner().unwrap().unwrap_or(Gen::Failed("not run".into()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_unique() {
        let mut t = BTreeSet::new();
        assert_eq!(slug("clk-obs-obs-ring", &mut t), "clk-obs-obs-ring");
        assert_eq!(slug("clk-obs-obs-ring", &mut t), "clk-obs-obs-ring-2");
        assert_eq!(slug("Drop Ring!", &mut t), "drop-ring-");
    }

    #[test]
    fn headings() {
        let f = |p: &str, gp: Option<&str>| ExtFamily {
            id: "x".into(),
            period: ExtFormula::parse(p).unwrap(),
            blocks: ExtFormula::parse("4 + 2n").unwrap(),
            rule: String::new(),
            n_lo: 0,
            n_hi: 3,
            base: 0,
            generator: String::new(),
            group: None,
            group_title: None,
            group_period: gp.map(String::from),
            group_note: None,
            remove: Vec::new(),
        };
        let (a, b, c) = (f("10 + 4n", None), f("12 + 4n", None), f("40 * 2^n", None));
        assert_eq!(heading(&[&a]), "10 + 4n gt");
        assert_eq!(heading(&[&a, &b]), "a + 4n gt, a in {10, 12}");
        assert_eq!(heading(&[&a, &c]), "10 + 4n · 40 · 2^n gt");
        let g = f("27 + 14n", Some("27 + 8k + 14n gt"));
        assert_eq!(heading(&[&g, &a]), "27 + 8k + 14n gt");
    }
}
