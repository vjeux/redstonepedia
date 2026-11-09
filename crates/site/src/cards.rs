//! Curated cards (vjeux 2026-11-08 11:16: "Different farms have different tradeoffs. We don't
//! care about lava trips for the high end farm. Please have the ability to adapt what is shown
//! based on the objective."): card lines an agent writes for one design or for a whole kind, every
//! number in them bound to the design's measurements, so no number can be invented or drift.
//!
//! - One file per study, `designs/circuits/<study>/cards.tsv`, header `key<TAB>slot<TAB>line`, one
//!   row per line, in order. `key` is a design's file stem (`iron-day1`: that design's card) or a
//!   kind (`mob-farm iron_ingot day-1`: the default of every design of the kind). A design's own
//!   rows win, then its kind's, then the site's built-in lines.
//! - `slot`: `sub` (the card's second line and the page's lede, its parts joined with " · "),
//!   `fact` (the page's tradeoff sentences), `diff` (what sets it apart from its neighbours, on its
//!   page's front), `if` (a check on the design: `<field> <op> <value>` with op `=`, `!=`, `>=` or
//!   `<=`; the build fails when it does not hold, so a re-judge that flips a fact breaks the build
//!   instead of leaving a card that lies).
//! - Placeholders: `{field}`, `{field|round}`, `{field|about}` (rounded as the titles round a
//!   rate: `3,200`), `{field|thousands}`, `{field|plural:trip}` (`16
//!   trips`, `1 trip`); `{field@stem}` is another design's value (same study), `{delta:field@stem}`
//!   the difference between the two, `{ratio:field@stem}` this one's over the other's. The fields:
//!   [`fields`].
//! - The build fails on a placeholder that names no field of the design, and on any digit outside a
//!   placeholder; an `if` row (`<field> <op> <value>`) after a row must hold, or the build fails,
//!   except `<field> present`: the row before it is left out of a design that lacks the field (a
//!   farm with no weather or restart runs, a `--no-gate` upload: the parent 2026-11-08). `mctas site
//!   variation` renders a farm's card before it writes anything, so an upload whose card fails is
//!   refused there, not at the deploy's site build.

use anyhow::{Result, bail};
use site_model::{Build, Curated};
use std::collections::BTreeMap;
use std::path::Path;

/// The file of a study's curated cards.
pub fn path(root: &Path, study: &str) -> std::path::PathBuf {
    root.join(crate::circuits::ROOT).join(study).join("cards.tsv")
}

/// One row of a cards file.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: String,
    pub slot: String,
    pub line: String,
    /// Its line number in the file (1 = the header), for the build's errors.
    pub at: usize,
}

/// The rows of a study's cards file (none when it has no file); a malformed row fails.
pub fn read(root: &Path, study: &str) -> Result<Vec<Row>> {
    let p = path(root, study);
    let Ok(text) = std::fs::read_to_string(&p) else { return Ok(Vec::new()) };
    let mut rows = Vec::new();
    for (i, l) in text.lines().enumerate().skip(1) {
        if l.trim().is_empty() || l.starts_with('#') {
            continue;
        }
        let mut parts = l.splitn(3, '\t');
        let (Some(key), Some(slot), Some(line)) = (parts.next(), parts.next(), parts.next()) else {
            bail!("{} line {}: not `key<TAB>slot<TAB>line`", p.display(), i + 1);
        };
        if !matches!(slot.trim(), "sub" | "fact" | "diff" | "if") {
            bail!("{} line {}: slot {slot:?} (sub, fact, diff or if)", p.display(), i + 1);
        }
        rows.push(Row { key: key.trim().to_string(), slot: slot.trim().to_string(), line: line.trim().to_string(), at: i + 1 });
    }
    Ok(rows)
}

/// A metric key as a field name: lowercase, every run of other characters one `_`.
fn field_key(k: &str) -> String {
    k.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// The number at the start of `v` (`295`, `48.6`, `62%`), if it starts with one.
fn lead_number(v: &str) -> Option<String> {
    let t = v.split_whitespace().next()?.trim_end_matches([',', ';', ')']);
    t.starts_with(|c: char| c.is_ascii_digit()).then(|| t.to_string())
}

/// The number right after `key` in `v` (`clear day 305 iron/h` with `clear day ` -> `305`).
fn number_after(v: &str, key: &str) -> Option<String> {
    let rest = v.split(key).nth(1)?;
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n = n.trim_end_matches('.').to_string();
    (!n.is_empty()).then_some(n)
}

/// A design's fields, by name: every measured metric by its key (`iron per hour` ->
/// `iron_per_hour`: its number, or its words when it has none; a time in game ticks also as
/// `<key>_seconds`), the parts of the composite ones (an iron farm's `rain`, `lightning`,
/// `bill`, `storage`, `iron range`...), the qualitative facts as phrases (`first_night`,
/// `days_away`, `sunburn`, `fire`), and the design's own `blocks` (also `blocks_placed`) and
/// `dims`.
pub fn fields(metrics: &[(String, String)], blocks: u32, dims: &str) -> BTreeMap<String, String> {
    let mut f: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in metrics {
        let v = v.trim();
        let key = field_key(k);
        if key.is_empty() {
            continue;
        }
        if let Some(n) = v.strip_suffix(" gt").and_then(|n| n.trim().parse::<u32>().ok()) {
            f.insert(format!("{key}_seconds"), trim_num(n as f64 / 20.0, 2));
        }
        f.insert(key, lead_number(v).unwrap_or_else(|| v.to_string()));
    }
    let m = |k: &str| metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.trim().to_string()).unwrap_or_default();
    // an iron farm's composite lines (ironfarm's and mobinfra's card lines)
    let range = m("iron range");
    if let Some((lo, hi)) = range.split_whitespace().nth(1).and_then(|r| r.split_once('-')) {
        f.insert("iron_per_hour_low".into(), lo.to_string());
        f.insert("iron_per_hour_high".into(), hi.trim_end_matches(',').to_string());
    }
    let rain = m("rain");
    for (part, name) in [("clear day ", "clear"), ("rain day ", "rain"), ("thunder day ", "thunder")] {
        if let Some(n) = number_after(&rain, part) {
            f.insert(format!("{name}_iron_per_hour"), n.clone());
            f.insert(format!("{name}_day_iron_per_hour"), n);
        }
    }
    let lightning = m("lightning");
    if let Some(d) = number_after(&lightning, "the farm breaks in about ") {
        if let Ok(days) = d.parse::<f64>() {
            // (a game day is 20 minutes: 3 an hour of play)
            f.insert("lightning_break_hours".into(), format!("{}", (days / 3.0).round() as u64));
        }
        f.insert("lightning_break_days".into(), d);
        if let Some(first) = lightning.split("the farm breaks in about ").nth(1).and_then(|r| r.split_once('(')).and_then(|(_, r)| r.split_once(')')).map(|(c, _)| c.trim_end_matches(" first").to_string()) {
            f.insert("lightning_first".into(), first);
        }
    }
    if let Some(n) = number_after(&lightning, "about ").filter(|_| lightning.contains(" iron lost per thunderstorm")) {
        f.insert("lightning_loss_per_storm".into(), n);
    }
    let bill = m("bill");
    for part in bill.split(", ") {
        let mut w = part.split_whitespace();
        if let (Some(n), Some(what)) = (w.next(), w.next()) {
            if n.parse::<f64>().is_ok() {
                let name = match what {
                    "iron" => "bill_iron",
                    "villager" | "villagers" => "villagers",
                    "bed" | "beds" => "beds",
                    "zombie" | "zombies" => "zombies",
                    other => other,
                };
                f.insert(name.to_string(), n.to_string());
                if name == "bill_iron" {
                    f.insert("iron_bill".into(), n.to_string());
                }
            }
        }
    }
    // the build's own card lines (ca45f3a02): `build time 47 min (185 placed, 0 dug, 16 lava
    // trips, 3 villagers moved)`, `time to an iron kit 54 min (...)`; the ironspec's names
    let build = m("build time");
    if let Some(n) = lead_number(&build) {
        f.insert("build_minutes".into(), n);
        let inner = build.split_once('(').map(|(_, r)| r.trim_end_matches(')')).unwrap_or("");
        for part in inner.split(", ") {
            let (n, what) = part.split_once(' ').unwrap_or(("", ""));
            if n.parse::<f64>().is_ok() {
                let name = match what.trim() {
                    "placed" => "build_placed",
                    "dug" => "blocks_dug",
                    "lava trips" | "lava trip" => "lava_trips",
                    "villagers moved" | "villager moved" => "villagers_moved",
                    _ => continue,
                };
                f.insert(name.to_string(), n.to_string());
            }
        }
    }
    if let Some(n) = lead_number(&m("time to an iron kit")) {
        f.insert("t_kit_minutes".into(), n);
    }
    let storage = m("storage");
    if let Some(h) = storage.strip_suffix(')').and_then(|s| s.split_once(" h (")).map(|(h, _)| h.trim().to_string()).or_else(|| lead_number(&storage)) {
        f.insert("storage_hours".into(), h);
    }
    if let Some(p) = storage.split("; ").nth(1).and_then(|r| r.split(" poppies/h").next()) {
        f.insert("poppies_per_hour".into(), p.trim().to_string());
    }
    if let Some(h) = f.get("hoppers_per_1000_iron_h").cloned() {
        f.insert("hoppers_per_1000_iron".into(), h);
    }
    if let Some(r) = f.get("iron_per_hour").and_then(|r| r.parse::<f64>().ok()) {
        // (the ironspec's "iron per night": eight hours of it)
        f.insert("iron_per_night".into(), format!("{}", (r * 8.0).round() as u64));
    }
    // the qualitative facts as phrases (a sentence without digits cannot go stale either)
    let night = m("restarts after a missed night");
    if !night.is_empty() {
        let yes = night.starts_with("yes");
        f.insert("restart_missed_night".into(), if yes { "yes" } else { "no" }.into());
        f.insert("first_night".into(), if yes { "starts by itself the first night" } else { "hide the zombie by hand the first night" }.into());
    }
    let away = m("restarts after days away");
    if !away.is_empty() {
        let yes = away.starts_with("yes");
        f.insert("restart_days_away".into(), if yes { "yes" } else { "no" }.into());
        f.insert("days_away".into(), if yes { "restarts by itself when you come back after days away" } else { "needs the zombie hidden by hand after days away" }.into());
    }
    let sun = m("sunburn");
    if !sun.is_empty() {
        f.insert("sunburn".into(), if sun.starts_with("never") { "the zombie never sees the sky" } else { "the zombie can burn in daylight" }.into());
    }
    let fire = m("fire");
    if !fire.is_empty() {
        let words = if fire.starts_with("never") { "nothing in the lava's reach can burn".to_string() } else { format!("fire: {fire}") };
        f.insert("fire".into(), words);
    }
    // whether it needs redstone and how much, in a player's words (ironspec rev 9 iteration 19:
    // day 1 has a pick per redstone fork value, each card saying plainly "needs 12 redstone dust";
    // the count from its blocks by recipe, `crate::farms::redstone_dust`)
    if let Some(n) = f.get("redstone_dust").and_then(|v| v.parse::<u32>().ok()) {
        f.insert("redstone".into(), if n == 0 { "needs no redstone".to_string() } else { format!("needs {n} redstone dust") });
    }
    f.insert("blocks".into(), blocks.to_string());
    f.insert("blocks_placed".into(), blocks.to_string());
    if !dims.is_empty() {
        f.insert("dims".into(), dims.replace('x', "×"));
    }
    f
}

/// `x` with at most `places` decimals, its trailing zeros off.
fn trim_num(x: f64, places: usize) -> String {
    let s = format!("{x:.places$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// The decimals a number is written with (`48.6` -> 1).
fn decimals(s: &str) -> usize {
    s.trim_end_matches('%').split_once('.').map(|(_, d)| d.len()).unwrap_or(0)
}

/// `1234567` -> `1,234,567` (the integer part; a fraction stays).
fn thousands(s: &str) -> String {
    let (int, frac) = s.split_once('.').map(|(a, b)| (a, format!(".{b}"))).unwrap_or((s, String::new()));
    let digits: Vec<char> = int.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 && c.is_ascii_digit() && digits[i - 1].is_ascii_digit() {
            out.push(',');
        }
        out.push(*c);
    }
    out + &frac
}

/// One placeholder filled: `field`, `field|round`, `field@stem`, `delta:field@stem|round`...
fn fill(spec: &str, mine: &BTreeMap<String, String>, other: &dyn Fn(&str) -> Option<BTreeMap<String, String>>) -> Result<String, String> {
    let mut parts = spec.split('|');
    let head = parts.next().unwrap_or("").trim();
    let (op, head) = match head.split_once(':') {
        Some((op, rest)) if op == "delta" || op == "ratio" => (Some(op), rest),
        _ => (None, head),
    };
    let (name, stem) = match head.split_once('@') {
        Some((n, s)) => (n.trim(), Some(s.trim())),
        None => (head, None),
    };
    let theirs = match stem {
        Some(s) => Some(other(s).ok_or_else(|| format!("no design {s:?} of this study to compare with"))?),
        None => None,
    };
    let get = |f: &BTreeMap<String, String>, who: &str| f.get(name).cloned().ok_or_else(|| format!("no field {name:?} in {who}"));
    let num = |s: &str| s.trim_end_matches('%').replace(',', "").parse::<f64>().map_err(|_| format!("{name} is not a number ({s:?})"));
    let mut value = match (op, &theirs) {
        (None, None) => get(mine, "this design")?,
        (None, Some(t)) => get(t, stem.unwrap_or(""))?,
        (Some(op), Some(t)) => {
            let (a, b) = (get(mine, "this design")?, get(t, stem.unwrap_or(""))?);
            let (x, y) = (num(&a)?, num(&b)?);
            if op == "delta" {
                trim_num((x - y).abs(), decimals(&a).max(decimals(&b)))
            } else if y == 0.0 {
                return Err(format!("{name} of {} is 0: no ratio", stem.unwrap_or("")));
            } else {
                trim_num(x / y, 1)
            }
        }
        (Some(op), None) => return Err(format!("{op}: needs another design (`{op}:{name}@<stem>`)")),
    };
    for filter in parts {
        let (fname, arg) = filter.split_once(':').map(|(a, b)| (a.trim(), Some(b.trim()))).unwrap_or((filter.trim(), None));
        value = match fname {
            "round" => {
                let pct = value.ends_with('%');
                format!("{}{}", num(&value)?.round() as i64, if pct { "%" } else { "" })
            }
            "thousands" => thousands(&value),
            // (as the titles round a rate: "3,200 melon slices an hour", two figures from 1,000 up,
            // site_model::html::readable; so a card's line and its neighbour's title agree)
            "about" => site_model::html::readable(num(&value)?),
            "plural" => {
                let word = arg.filter(|w| !w.is_empty()).ok_or("plural needs its word (`|plural:trip`)")?;
                let one = num(&value).is_ok_and(|x| x == 1.0);
                format!("{value} {word}{}", if one { "" } else { "s" })
            }
            other => return Err(format!("unknown filter {other:?} (round, about, thousands, plural:<word>)")),
        };
    }
    // (a number of 1,000 or more is grouped, filter or not, as the titles write it: the parent
    // 2026-11-08, "1700 iron an hour" beside "1,700")
    Ok(crate::explain::group_numbers(&value))
}

/// A card line filled from the design's fields: every placeholder replaced, or why not (no such
/// field, no such design, a digit outside a placeholder).
pub fn render(line: &str, mine: &BTreeMap<String, String>, other: &dyn Fn(&str) -> Option<BTreeMap<String, String>>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find('{') {
        let literal = &rest[..i];
        if let Some(d) = literal.chars().find(|c| c.is_ascii_digit()) {
            return Err(format!("the digit {d:?} is written out: every number must be a placeholder"));
        }
        out.push_str(literal);
        let Some(j) = rest[i..].find('}') else { return Err("a `{` without its `}`".into()) };
        out.push_str(&fill(&rest[i + 1..i + j], mine, other)?);
        rest = &rest[i + j + 1..];
    }
    if let Some(d) = rest.chars().find(|c| c.is_ascii_digit()) {
        return Err(format!("the digit {d:?} is written out: every number must be a placeholder"));
    }
    out.push_str(rest);
    Ok(out)
}

/// A check row (`<field> <op> <value>`) against the design's fields; `<field> present` holds when
/// the design has the field (its row is then the design's; without it, [`card_of`] leaves the row
/// out instead of failing: a card without weather runs, a `--no-gate` farm's, the parent
/// 2026-11-08).
fn holds(check: &str, mine: &BTreeMap<String, String>) -> Result<bool, String> {
    if let Some(f) = present_field(check) {
        return Ok(mine.contains_key(f));
    }
    let mut w = check.split_whitespace();
    let (Some(field), Some(op)) = (w.next(), w.next()) else { return Err(format!("not `<field> <op> <value>`: {check:?}")) };
    let want = w.collect::<Vec<_>>().join(" ");
    let have = mine.get(field).ok_or_else(|| format!("no field {field:?} in this design"))?;
    let n = |s: &str| s.trim_end_matches('%').replace(',', "").parse::<f64>().ok();
    Ok(match (op, n(have), n(&want)) {
        ("=", Some(a), Some(b)) => a == b,
        ("!=", Some(a), Some(b)) => a != b,
        (">=", Some(a), Some(b)) => a >= b,
        ("<=", Some(a), Some(b)) => a <= b,
        ("=", _, _) => have.trim() == want,
        ("!=", _, _) => have.trim() != want,
        (">=" | "<=", _, _) => return Err(format!("{field} = {have:?} is not a number to compare")),
        _ => return Err(format!("unknown op {op:?} (=, !=, >=, <=, present)")),
    })
}

/// The field of a `<field> present` check.
fn present_field(check: &str) -> Option<&str> {
    let mut w = check.split_whitespace();
    match (w.next(), w.next(), w.next()) {
        (Some(f), Some("present"), None) => Some(f),
        _ => None,
    }
}

/// A card row as a sentence: its first letter up, a full stop at its end unless it has one.
fn sentence(s: &str) -> String {
    let s = s.trim();
    let mut chars = s.chars();
    let up = chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default();
    if up.is_empty() || up.ends_with(['.', '!', '?']) { up } else { format!("{up}.") }
}

/// A design's card from the rows that are its own (its stem's), else its kind's; None when it
/// has none. An unknown field, a written-out digit or a failing check fails, naming the row; a row
/// followed by a `<field> present` check whose field the design lacks is left out (a card without
/// those runs: a `--no-gate` farm has no weather or restart runs, the parent 2026-11-08).
pub fn card_of(rows: &[Row], file: &str, stem: &str, kind: &str, mine: &BTreeMap<String, String>, other: &dyn Fn(&str) -> Option<BTreeMap<String, String>>) -> Result<Option<Curated>> {
    let own: Vec<&Row> = rows.iter().filter(|r| r.key == stem).collect();
    let chosen = if own.is_empty() { rows.iter().filter(|r| r.key == kind).collect() } else { own };
    if chosen.is_empty() {
        return Ok(None);
    }
    let mut card = Curated::default();
    let mut i = 0;
    while i < chosen.len() {
        let r = chosen[i];
        // the row's own checks: the `if` rows right after it
        let mut j = i + 1;
        while r.slot != "if" && j < chosen.len() && chosen[j].slot == "if" {
            j += 1;
        }
        let checks = &chosen[i + 1..j];
        if checks.iter().any(|g| present_field(&g.line).is_some_and(|f| !mine.contains_key(f))) {
            i = j;
            continue;
        }
        for r in std::iter::once(r).chain(checks.iter().copied()) {
            let fail = |why: String| anyhow::anyhow!("{file} line {} ({} {}, for {stem}): {why}: {:?}", r.at, r.key, r.slot, r.line);
            match r.slot.as_str() {
                "if" => {
                    if !holds(&r.line, mine).map_err(fail)? {
                        return Err(fail(format!("the check does not hold ({} is {:?})", r.line.split_whitespace().next().unwrap_or(""), mine.get(r.line.split_whitespace().next().unwrap_or("")).cloned().unwrap_or_default())));
                    }
                }
                slot => {
                    let text = render(&r.line, mine, other).map_err(fail)?;
                    match slot {
                        "sub" => card.sub.push(text),
                        // (a fact or a diff row is a sentence of its own: its first letter up, its
                        // full stop added, so rows written as fragments or as one phrase field read
                        // one after the other)
                        "fact" => card.facts.push(sentence(&text)),
                        _ => card.diff.push(sentence(&text)),
                    }
                }
            }
        }
        i = j;
    }
    Ok(Some(card))
}

/// Every circuit build's curated card, from its study's cards file (`root` is the repository);
/// another design named in a placeholder is looked up among `builds`, then in the study's archive.
pub fn apply(root: &Path, builds: &mut [Build]) -> Result<usize> {
    apply_with(root, builds, &BTreeMap::new())
}

/// [`apply`], with more designs to compare with (a preview's other designs, by (study, stem)).
pub fn apply_with(root: &Path, builds: &mut [Build], siblings: &BTreeMap<(String, String), BTreeMap<String, String>>) -> Result<usize> {
    let mut by_study: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut n = 0;
    // every build's fields by (study, stem), for the comparisons
    let mut known: BTreeMap<(String, String), BTreeMap<String, String>> = siblings.clone();
    for b in builds.iter() {
        if let Some(c) = &b.circuit {
            known.insert((c.study.clone(), b.name.clone()), fields(&c.metrics, b.blocks, b.dims.as_deref().unwrap_or("")));
        }
    }
    for b in builds.iter_mut() {
        let Some(c) = b.circuit.as_ref() else { continue };
        let study = c.study.clone();
        if !by_study.contains_key(&study) {
            by_study.insert(study.clone(), read(root, &study)?);
        }
        let rows = &by_study[&study];
        if rows.is_empty() {
            continue;
        }
        let mine = fields(&c.metrics, b.blocks, b.dims.as_deref().unwrap_or(""));
        let other = |stem: &str| known.get(&(study.clone(), stem.to_string())).cloned().or_else(|| archived_fields(root, &study, stem));
        let file = format!("{}/{study}/cards.tsv", crate::circuits::ROOT);
        if let Some(card) = card_of(rows, &file, &b.name, &c.kind, &mine, &other)? {
            b.curated = card;
            n += 1;
        }
    }
    Ok(n)
}

/// An archived design's fields, from its study's verdicts (`stem`.txt); an iron farm's card can
/// also quote a judged design that is not listed ([`crate::farms::quotable_dirs`]: the bars, the
/// stages' variations).
pub fn archived_fields(root: &Path, study: &str, stem: &str) -> Option<BTreeMap<String, String>> {
    let file = format!("{stem}.txt");
    let mut dirs = vec![circuit::archive::archive_dir(&root.join(crate::circuits::ROOT), study)];
    if study == "mobfarm" {
        dirs.extend(crate::farms::quotable_dirs(root));
    }
    let r = dirs.iter().find_map(|d| circuit::archive::read_rows(d).ok()?.into_iter().find(|r| r.file == file))?;
    Some(fields(&crate::circuits::parse_metrics(&r.metrics), r.blocks as u32, &r.dims))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day1() -> BTreeMap<String, String> {
        let metrics = crate::circuits::parse_metrics("iron per hour per block 1.56, iron per hour 295, iron range 95% 292-298 (64 runs), golems per hour 74.1, seconds per golem 48.6, share of ceiling 62%, modules 1, kill 241 gt, restarts after a missed night no (hide the zombie by hand the first night), restarts after days away no, rain clear day 305 iron/h; rain day 293 iron/h; thunder day 299 iron/h, lightning about 0.00032 iron lost per thunderstorm; a witch about every 5332 game days; the farm breaks in about 2871 game days (a fire first); thunderstorms 1.4% of the time (6211 gt each on average), sunburn never (the zombie never sees the sky), fire never, bill 11 iron, 0 quartz, 3 villagers, 3 beds, 1 zombie, build time 47 min (185 placed, 0 dug, 16 lava trips, 3 villagers moved), time to an iron kit 54 min (the build, then (bill + 24) / rate), lava sources 16, storage 5.2 h (32 slots; 74 poppies/h), hoppers per 1000 iron/h 3.4");
        fields(&metrics, 189, "16x7x10")
    }

    #[test]
    fn a_present_check_leaves_out_a_row_the_design_has_no_fields_for() {
        // (the parent 2026-11-08: a --no-gate farm has no weather runs; its card builds without
        // the weather line instead of failing the site build)
        let row = |at: usize, slot: &str, line: &str| Row { key: "mob-farm iron_ingot middle".into(), slot: slot.into(), line: line.into(), at };
        let rows = vec![
            row(2, "fact", "{rain_iron_per_hour|round} iron an hour on a rainy day"),
            row(3, "if", "rain_iron_per_hour present"),
            row(4, "fact", "Iron to build: {bill_iron}"),
        ];
        let none = |_: &str| None;
        let with = card_of(&rows, "cards.tsv", "d1", "mob-farm iron_ingot middle", &day1(), &none).unwrap().unwrap();
        assert_eq!(with.facts, vec!["293 iron an hour on a rainy day.".to_string(), "Iron to build: 11.".to_string()]);
        let mut bare = day1();
        bare.retain(|k, _| !k.contains("rain") && !k.contains("thunder") && !k.contains("clear"));
        let without = card_of(&rows, "cards.tsv", "d1", "mob-farm iron_ingot middle", &bare, &none).unwrap().unwrap();
        assert_eq!(without.facts, vec!["Iron to build: 11.".to_string()], "the weather row is left out, the rest stays");
        // (a row with no such check still fails on a field the design lacks: a typo never hides)
        let strict = vec![row(2, "fact", "{rain_iron_per_hour|round} iron an hour on a rainy day")];
        assert!(card_of(&strict, "cards.tsv", "d1", "mob-farm iron_ingot middle", &bare, &none).is_err());
    }

    #[test]
    fn the_fields_of_an_iron_farm() {
        let f = day1();
        for (k, v) in [
            ("iron_per_hour", "295"),
            ("seconds_per_golem", "48.6"),
            ("share_of_ceiling", "62%"),
            ("kill_seconds", "12.05"),
            ("iron_per_hour_low", "292"),
            ("rain_iron_per_hour", "293"),
            ("thunder_day_iron_per_hour", "299"),
            ("clear_iron_per_hour", "305"),
            ("lightning_break_days", "2871"),
            ("lightning_break_hours", "957"),
            ("lightning_first", "a fire"),
            ("bill_iron", "11"),
            ("villagers", "3"),
            ("zombies", "1"),
            ("lava_sources", "16"),
            ("storage_hours", "5.2"),
            ("poppies_per_hour", "74"),
            ("hoppers_per_1000_iron", "3.4"),
            ("iron_per_night", "2360"),
            ("first_night", "hide the zombie by hand the first night"),
            ("restart_days_away", "no"),
            ("sunburn", "the zombie never sees the sky"),
            ("fire", "nothing in the lava's reach can burn"),
            ("blocks", "189"),
            ("dims", "16×7×10"),
            ("build_minutes", "47"),
            ("blocks_dug", "0"),
            ("lava_trips", "16"),
            ("villagers_moved", "3"),
            ("t_kit_minutes", "54"),
        ] {
            assert_eq!(f.get(k).map(String::as_str), Some(v), "{k}");
        }
    }

    #[test]
    fn a_card_line_is_filled_and_never_invents_a_number() {
        let f = day1();
        let none = |_: &str| None;
        assert_eq!(render("{iron_per_hour} iron an hour, a golem every {seconds_per_golem|round} s", &f, &none).unwrap(), "295 iron an hour, a golem every 49 s");
        assert_eq!(render("lightning breaks it about every {lightning_break_days|thousands} game days", &f, &none).unwrap(), "lightning breaks it about every 2,871 game days");
        assert_eq!(render("about {lightning_break_days|about} game days", &f, &none).unwrap(), "about 2,900 game days");
        assert_eq!(render("{lava_sources|plural:lava trip}", &f, &none).unwrap(), "16 lava trips");
        assert!(render("takes {villagers} of the village's 8 villagers", &f, &none).unwrap_err().contains("written out"));
        assert!(render("{iron_per_hour_sleeping} iron an hour if you sleep", &f, &none).unwrap_err().contains("no field \"iron_per_hour_sleeping\""));
        assert_eq!(render("{t_kit_minutes} minutes from the first block to a full set of iron armour", &f, &none).unwrap(), "54 minutes from the first block to a full set of iron armour");
        // another design's value, the difference and the ratio
        let mut m2 = f.clone();
        m2.insert("iron_per_hour".into(), "439".into());
        let other = |s: &str| (s == "m2-open").then(|| m2.clone());
        assert_eq!(render("{delta:iron_per_hour@m2-open} iron an hour less than the middle farm's {iron_per_hour@m2-open}", &f, &other).unwrap(), "144 iron an hour less than the middle farm's 439");
        assert_eq!(render("{ratio:iron_per_hour@m2-open} times as much", &m2, &|_: &str| Some(day1())).unwrap(), "1.5 times as much");
        assert!(render("{iron_per_hour@nope}", &f, &other).unwrap_err().contains("no design"));
    }

    #[test]
    fn a_design_row_wins_over_its_kind_and_a_check_must_hold() {
        let f = day1();
        let none = |_: &str| None;
        let row = |key: &str, slot: &str, line: &str| Row { key: key.into(), slot: slot.into(), line: line.into(), at: 2 };
        let rows = vec![row("mob-farm iron_ingot day-1", "sub", "{iron_per_hour} iron an hour"), row("iron-day1", "sub", "{first_night}"), row("iron-day1", "if", "restart_missed_night = no")];
        let card = card_of(&rows, "cards.tsv", "iron-day1", "mob-farm iron_ingot day-1", &f, &none).unwrap().unwrap();
        assert_eq!(card.sub, vec!["hide the zombie by hand the first night".to_string()]);
        let card = card_of(&rows, "cards.tsv", "d1-bed-plate", "mob-farm iron_ingot day-1", &f, &none).unwrap().unwrap();
        assert_eq!(card.sub, vec!["295 iron an hour".to_string()]);
        assert!(card_of(&rows, "cards.tsv", "x", "mob-farm iron_ingot middle", &f, &none).unwrap().is_none());
        let flipped = vec![row("iron-day1", "if", "restart_missed_night = yes")];
        assert!(card_of(&flipped, "cards.tsv", "iron-day1", "k", &f, &none).unwrap_err().to_string().contains("does not hold"));
        // (fact rows read as sentences one after the other, written as fragments or not)
        let facts = vec![row("k", "fact", "To build it: {bill_iron} iron in hand"), row("k", "fact", "{first_night}"), row("k", "fact", "Its storage holds {storage_hours} hours of iron.")];
        let card = card_of(&facts, "cards.tsv", "x", "k", &f, &none).unwrap().unwrap();
        assert_eq!(card.facts.join(" "), "To build it: 11 iron in hand. Hide the zombie by hand the first night. Its storage holds 5.2 hours of iron.");
    }
}
