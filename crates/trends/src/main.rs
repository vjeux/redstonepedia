//! `trends`: search demand for Redstonepedia's topics on the open web (docs/search-demand.md).
//!
//! Google's own numbers, three ways:
//! - `trends suggest`: Google (or YouTube) autocomplete of seed phrases, optionally expanded with
//!   every letter and digit: the searches people actually type, in Google's order.
//! - `trends related`: Google Trends' related queries (the top list and the rising list) of
//!   each term.
//! - `trends rank`: the Google Trends interest of every term of a list on ONE scale. Trends
//!   compares at most five terms per request and scales each request to its own busiest week, so
//!   every request carries the same anchor term plus four others, and a term's score is its mean
//!   weekly interest divided by the anchor's in the same request (x100, or scaled so that
//!   `--scale-to` is 100). A batch squeezed by a much bigger term is re-measured without it, and
//!   a second pass can re-measure the long tail against a smaller anchor (finer resolution).
//!
//! Not a `mctas` subcommand on purpose: it needs the internet, which design work never uses
//! (docs/process.md), and an internet box must be able to build it without the SAT crates
//! (rustsat-cadical needs libclang). Requests go through `curl` (the workspace has no TLS stack),
//! with the cookie Google Trends wants, a pause between requests and a growing wait on HTTP 429.

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                          Chrome/130.0.0.0 Safari/537.36";
const TRENDS: &str = "https://trends.google.com";
const SUGGEST: &str = "https://suggestqueries.google.com/complete/search";
/// Google Trends compares at most five terms per request: the anchor and four others.
const PER_BATCH: usize = 4;
/// Under this mean, a batch's anchor was squeezed by a much bigger term of the batch.
const SQUEEZED: f64 = 10.0;

#[derive(Parser)]
#[command(
    name = "trends",
    about = "Search demand for Minecraft topics: Google autocomplete and Google Trends \
             (docs/search-demand.md). Needs internet access and curl."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Autocomplete of each seed, in Google's order (`--alphabet`: also `<seed> a` .. `<seed> 9`).
    Suggest(SuggestArgs),
    /// Google Trends related queries of each term: the top list and the rising list.
    Related(RelatedArgs),
    /// Every term of a list on one Google Trends scale, anchor-normalized.
    Rank(RankArgs),
}

#[derive(Args)]
struct Net {
    /// Pause between two requests, in milliseconds.
    #[arg(long, default_value_t = 1500)]
    delay_ms: u64,
    /// curl's cookie jar (Google Trends refuses requests without its cookie); default: a file in
    /// the temp directory.
    #[arg(long)]
    cookies: Option<PathBuf>,
}

#[derive(Args)]
struct Scope {
    /// Region (`US`, `GB`, ...); empty = worldwide.
    #[arg(long, default_value = "")]
    geo: String,
    /// Time range: `today 12-m`, `today 5-y`, `2025-01-01 2025-12-31`, ...
    #[arg(long, default_value = "today 12-m")]
    time: String,
    /// YouTube searches instead of web searches.
    #[arg(long)]
    youtube: bool,
}

impl Scope {
    fn property(&self) -> &'static str {
        if self.youtube { "youtube" } else { "" }
    }

    fn describe(&self, range: &str) -> String {
        let region = if self.geo.is_empty() { "worldwide" } else { self.geo.as_str() };
        let what = if self.youtube { "YouTube" } else { "Google web" };
        let when = if range.is_empty() { self.time.clone() } else { range.replace(' ', " to ") };
        format!("{region} {what} searches, {when}")
    }
}

#[derive(Args)]
struct SuggestArgs {
    /// Seed phrases, e.g. "minecraft redstone".
    #[arg(required = true)]
    seeds: Vec<String>,
    /// Also ask for every `<seed> <letter or digit>`.
    #[arg(long)]
    alphabet: bool,
    /// YouTube's autocomplete instead of Google's.
    #[arg(long)]
    youtube: bool,
    /// Language of the suggestions.
    #[arg(long, default_value = "en")]
    hl: String,
    /// Country of the suggestions.
    #[arg(long, default_value = "us")]
    gl: String,
    #[command(flatten)]
    net: Net,
}

#[derive(Args)]
struct RelatedArgs {
    /// Terms (`a + b` counts searches for either).
    #[arg(required = true)]
    terms: Vec<String>,
    #[command(flatten)]
    scope: Scope,
    #[command(flatten)]
    net: Net,
}

#[derive(Args)]
struct RankArgs {
    /// The list: one term per line, `term<TAB>kind` (kind optional), `#` starts a comment. A term
    /// counts every search containing all of its words in any order; `a + b` counts either.
    #[arg(long)]
    terms: PathBuf,
    /// The term in every request. Pick one of medium interest; batches squeezed by a much bigger
    /// term are re-measured without it.
    #[arg(long)]
    anchor: String,
    /// The term printed as 100 (default: the anchor).
    #[arg(long)]
    scale_to: Option<String>,
    /// The anchor of a second pass for the long tail: a term of the list, well measured against
    /// the anchor; every term under `--refine-below` points next to the anchor is re-measured
    /// against it.
    #[arg(long)]
    low_anchor: Option<String>,
    /// See `--low-anchor`.
    #[arg(long, default_value_t = 5.0)]
    refine_below: f64,
    /// Measurements already made, appended after every request and read back by a rerun with the
    /// same scope: a rate-limited run resumes where it stopped.
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Title of the Markdown table.
    #[arg(long, default_value = "Search demand")]
    title: String,
    /// Write the ranking as TSV here (default: stdout).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Also write it as a Markdown table here.
    #[arg(long)]
    md: Option<PathBuf>,
    #[command(flatten)]
    scope: Scope,
    #[command(flatten)]
    net: Net,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Cmd::Suggest(args) => suggest(&args),
        Cmd::Related(args) => related(&args),
        Cmd::Rank(args) => rank(&args),
    }
}

/// HTTP through curl, paced, with one cookie jar.
struct Client {
    cookies: PathBuf,
    delay: Duration,
    last: Option<Instant>,
    primed: bool,
    requests: usize,
    /// The time range Google resolved for the last Trends comparison (`2025-11-08 2026-11-08`).
    range: String,
}

impl Client {
    fn new(net: &Net) -> Client {
        Client {
            cookies: net.cookies.clone().unwrap_or_else(|| std::env::temp_dir().join("trends.cookies")),
            delay: Duration::from_millis(net.delay_ms),
            last: None,
            primed: false,
            requests: 0,
            range: String::new(),
        }
    }

    /// GET `url` with the URL-encoded `query`: the HTTP status and the body.
    fn get(&mut self, url: &str, query: &[(&str, &str)]) -> Result<(u16, String)> {
        if let Some(rest) = self.last.and_then(|last| self.delay.checked_sub(last.elapsed())) {
            sleep(rest);
        }
        let mut cmd = Command::new("curl");
        cmd.args(["--silent", "--show-error", "--compressed", "--max-time", "60"])
            .args(["--user-agent", USER_AGENT])
            .arg("--cookie")
            .arg(&self.cookies)
            .arg("--cookie-jar")
            .arg(&self.cookies)
            .args(["--write-out", "\n%{http_code}", "--get"]);
        for (key, value) in query {
            cmd.arg("--data-urlencode").arg(format!("{key}={value}"));
        }
        let out = cmd.arg(url).output().context("running curl")?;
        self.last = Some(Instant::now());
        self.requests += 1;
        if !out.status.success() {
            bail!("curl {url}: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let (body, status) = text.rsplit_once('\n').ok_or_else(|| anyhow!("curl {url}: no status line"))?;
        let status = status.trim().parse().with_context(|| format!("curl {url}: status {status:?}"))?;
        Ok((status, body.to_string()))
    }

    /// A Google Trends API call: its JSON text without Google's `)]}'` guard. On HTTP 429 (rate
    /// limit): a fresh cookie and a growing wait, six tries in all.
    fn trends(&mut self, path: &str, query: &[(&str, &str)]) -> Result<String> {
        let mut wait = Duration::from_secs(30);
        for attempt in 1..=6 {
            if !self.primed {
                let (status, _) = self.get(&format!("{TRENDS}/trends/"), &[("geo", "US")])?;
                if status != 200 {
                    bail!("Google Trends home page: HTTP {status}");
                }
                self.primed = true;
            }
            let (status, body) = self.get(&format!("{TRENDS}{path}"), query)?;
            match status {
                200 => {
                    let start = body.find(['{', '[']).ok_or_else(|| anyhow!("{path}: no JSON in {:?}", head(&body)))?;
                    return Ok(body[start..].to_string());
                }
                429 => {
                    eprintln!("trends: HTTP 429 on {path} (try {attempt}/6), waiting {} s", wait.as_secs());
                    sleep(wait);
                    wait = (wait * 2).min(Duration::from_secs(600));
                    self.primed = false;
                }
                _ => bail!("{path}: HTTP {status}: {}", head(&body)),
            }
        }
        bail!("{path}: still rate limited after 6 tries")
    }
}

/// The first 300 characters of `s`, for error messages.
fn head(s: &str) -> String {
    s.chars().take(300).collect()
}

/// A widget of a Trends explore answer: its id, its token, and its request exactly as Google sent
/// it (the token is issued for that text, so it is passed back byte for byte).
struct Widget {
    id: String,
    token: String,
    request: String,
}

fn explore(client: &mut Client, scope: &Scope, terms: &[&str]) -> Result<Vec<Widget>> {
    let items: Vec<Value> = terms
        .iter()
        .map(|term| serde_json::json!({"keyword": term, "geo": scope.geo, "time": scope.time}))
        .collect();
    let req = serde_json::json!({"comparisonItem": items, "category": 0, "property": scope.property()}).to_string();
    let text = client.trends("/trends/api/explore", &[("hl", "en-US"), ("tz", "0"), ("req", &req)])?;
    let json: Value = serde_json::from_str(&text).context("explore: JSON")?;
    let widgets: Vec<&Value> = json["widgets"]
        .as_array()
        .map(|all| all.iter().filter(|widget| widget.get("request").is_some()).collect())
        .unwrap_or_default();
    let raw = raw_values(&text, "\"request\":");
    if widgets.is_empty() || widgets.len() != raw.len() {
        bail!("explore {terms:?}: {} widgets with a request, {} request texts", widgets.len(), raw.len());
    }
    Ok(widgets
        .iter()
        .zip(raw)
        .map(|(widget, request)| Widget {
            id: widget["id"].as_str().unwrap_or_default().to_string(),
            token: widget["token"].as_str().unwrap_or_default().to_string(),
            request: request.to_string(),
        })
        .collect())
}

/// The JSON text of the value after every `key` in `text`, in order (a value's own nested keys are
/// skipped with it).
fn raw_values<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find(key) {
        let start = from + pos + key.len();
        let Some(len) = balanced(&text[start..]) else { break };
        out.push(&text[start..start + len]);
        from = start + len;
    }
    out
}

/// The length of the JSON object or array at the start of `s` (strings and escapes respected).
fn balanced(s: &str) -> Option<usize> {
    if !s.starts_with(['{', '[']) {
        return None;
    }
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (i, byte) in s.bytes().enumerate() {
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The time range Google resolved for a comparison (`2025-11-08 2026-11-08`), from its
/// TIMESERIES widget.
fn resolved_range(widgets: &[Widget]) -> Option<String> {
    let widget = widgets.iter().find(|widget| widget.id == "TIMESERIES")?;
    serde_json::from_str::<Value>(&widget.request).ok()?["time"].as_str().map(str::to_string)
}

/// Each term's mean weekly interest over the range (Google scales the comparison's busiest week
/// to 100). Means of the weekly values, finer than Google's rounded averages.
fn averages(client: &mut Client, scope: &Scope, terms: &[&str]) -> Result<Vec<f64>> {
    let widgets = explore(client, scope, terms)?;
    let widget = widgets
        .iter()
        .find(|widget| widget.id == "TIMESERIES")
        .ok_or_else(|| anyhow!("explore {terms:?}: no TIMESERIES widget"))?;
    if let Some(range) = resolved_range(&widgets) {
        client.range = range;
    }
    let query = [("hl", "en-US"), ("tz", "0"), ("req", widget.request.as_str()), ("token", widget.token.as_str())];
    let text = client.trends("/trends/api/widgetdata/multiline", &query)?;
    let json: Value = serde_json::from_str(&text).context("multiline: JSON")?;
    let weeks = json["default"]["timelineData"].as_array().ok_or_else(|| anyhow!("multiline {terms:?}: no timelineData"))?;
    if weeks.is_empty() {
        bail!("multiline {terms:?}: empty timeline");
    }
    let mut sums = vec![0.0; terms.len()];
    for week in weeks {
        let values = week["value"].as_array().ok_or_else(|| anyhow!("multiline {terms:?}: a week without values"))?;
        if values.len() != terms.len() {
            bail!("multiline {terms:?}: {} values for {} terms", values.len(), terms.len());
        }
        for (sum, value) in sums.iter_mut().zip(values) {
            *sum += value.as_f64().unwrap_or(0.0);
        }
    }
    Ok(sums.into_iter().map(|sum| sum / weeks.len() as f64).collect())
}

fn suggest(args: &SuggestArgs) -> Result<()> {
    let mut client = Client::new(&args.net);
    let mut seen = HashSet::new();
    println!("seed\tquery\tposition\tsuggestion");
    for seed in &args.seeds {
        let mut queries = vec![seed.clone()];
        if args.alphabet {
            queries.extend(('a'..='z').chain('0'..='9').map(|c| format!("{seed} {c}")));
        }
        for query in &queries {
            let mut params = vec![("client", "firefox"), ("hl", args.hl.as_str()), ("gl", args.gl.as_str()), ("oe", "utf-8")];
            if args.youtube {
                params.push(("ds", "yt"));
            }
            params.push(("q", query.as_str()));
            let (status, body) = client.get(SUGGEST, &params)?;
            if status != 200 {
                bail!("suggest {query:?}: HTTP {status}: {}", head(&body));
            }
            let json: Value = serde_json::from_str(&body).with_context(|| format!("suggest {query:?}: {}", head(&body)))?;
            for (i, suggestion) in json[1].as_array().into_iter().flatten().filter_map(Value::as_str).enumerate() {
                if seen.insert(suggestion.to_string()) {
                    println!("{seed}\t{query}\t{}\t{suggestion}", i + 1);
                }
            }
        }
    }
    eprintln!("suggest: {} requests, {} distinct suggestions", client.requests, seen.len());
    Ok(())
}

fn related(args: &RelatedArgs) -> Result<()> {
    let mut client = Client::new(&args.net);
    println!("term\tlist\tvalue\tquery");
    for term in &args.terms {
        let widgets = explore(&mut client, &args.scope, &[term.as_str()])?;
        let widget = widgets
            .iter()
            .find(|widget| widget.id.starts_with("RELATED_QUERIES"))
            .ok_or_else(|| anyhow!("{term:?}: no related queries widget"))?;
        let query = [("hl", "en-US"), ("tz", "0"), ("req", widget.request.as_str()), ("token", widget.token.as_str())];
        let text = client.trends("/trends/api/widgetdata/relatedsearches", &query)?;
        let json: Value = serde_json::from_str(&text).context("relatedsearches: JSON")?;
        let lists = json["default"]["rankedList"].as_array().cloned().unwrap_or_default();
        for (list, name) in lists.iter().zip(["top", "rising"]) {
            for keyword in list["rankedKeyword"].as_array().into_iter().flatten() {
                let value = keyword["formattedValue"].as_str().unwrap_or_default();
                println!("{term}\t{name}\t{value}\t{}", keyword["query"].as_str().unwrap_or_default());
            }
        }
    }
    eprintln!("related: {} requests", client.requests);
    Ok(())
}

struct Term {
    text: String,
    kind: String,
}

fn read_terms(path: &Path) -> Result<Vec<Term>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default();
        if line.trim().is_empty() {
            continue;
        }
        let (term, kind) = line.split_once('\t').unwrap_or((line, ""));
        let term = term.trim().to_string();
        if !seen.insert(term.clone()) {
            bail!("{}: {term:?} is listed twice", path.display());
        }
        terms.push(Term { text: term, kind: kind.trim().to_string() });
    }
    Ok(terms)
}

/// One measurement: a term's mean next to an anchor's mean in the same request.
#[derive(Clone, Copy)]
struct Measure {
    raw: f64,
    anchor: f64,
}

/// Measurements by (anchor, term) for one scope, optionally kept in a file (append-only, the
/// last line of a pair wins).
struct Cache {
    scope: String,
    known: HashMap<(String, String), Measure>,
    file: Option<PathBuf>,
}

impl Cache {
    fn open(file: Option<&Path>, scope: &Scope) -> Result<Cache> {
        let scope = format!("{}|{}|{}", scope.geo, scope.time, scope.property());
        let mut known = HashMap::new();
        if let Some(path) = file.filter(|path| path.exists()) {
            for line in std::fs::read_to_string(path)?.lines() {
                let fields: Vec<&str> = line.split('\t').collect();
                if let [s, anchor, term, raw, anchor_raw] = fields[..] {
                    if s == scope {
                        let measure = Measure { raw: raw.parse()?, anchor: anchor_raw.parse()? };
                        known.insert((anchor.to_string(), term.to_string()), measure);
                    }
                }
            }
            eprintln!("rank: {} measurements from {}", known.len(), path.display());
        }
        Ok(Cache { scope, known, file: file.map(Path::to_path_buf) })
    }

    fn get(&self, anchor: &str, term: &str) -> Option<Measure> {
        self.known.get(&(anchor.to_string(), term.to_string())).copied()
    }

    fn put(&mut self, anchor: &str, term: &str, measure: Measure) -> Result<()> {
        if let Some(path) = &self.file {
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            writeln!(file, "{}\t{anchor}\t{term}\t{}\t{}", self.scope, measure.raw, measure.anchor)?;
        }
        self.known.insert((anchor.to_string(), term.to_string()), measure);
        Ok(())
    }
}

/// Measures `terms` against `anchor`, four per request; terms already in the cache are not asked
/// again unless `force`.
fn measure(
    client: &mut Client,
    scope: &Scope,
    cache: &mut Cache,
    anchor: &str,
    terms: &[&str],
    force: bool,
) -> Result<Vec<Measure>> {
    let todo: Vec<&str> = terms.iter().copied().filter(|term| force || cache.get(anchor, term).is_none()).collect();
    let batches = todo.len().div_ceil(PER_BATCH);
    for (b, chunk) in todo.chunks(PER_BATCH).enumerate() {
        let mut asked = vec![anchor];
        asked.extend_from_slice(chunk);
        let means = averages(client, scope, &asked)?;
        if means[0] < 0.5 {
            bail!("the anchor {anchor:?} averaged {:.2} next to {chunk:?}: pick a more searched anchor", means[0]);
        }
        let mut line = format!("rank: {anchor:?} batch {}/{batches}: anchor {:.1}", b + 1, means[0]);
        for (term, raw) in chunk.iter().zip(&means[1..]) {
            cache.put(anchor, term, Measure { raw: *raw, anchor: means[0] })?;
            write!(line, " | {term} {raw:.1}")?;
        }
        eprintln!("{line}");
    }
    terms
        .iter()
        .map(|term| cache.get(anchor, term).ok_or_else(|| anyhow!("{term:?} was not measured")))
        .collect()
}

/// A ranked row: the term's interest relative to the anchor (1.0 = the anchor), and how.
struct Row<'a> {
    term: &'a Term,
    ratio: f64,
    measure: Option<Measure>,
    against: &'a str,
}

fn rank(args: &RankArgs) -> Result<()> {
    let mut terms = read_terms(&args.terms)?;
    if !terms.iter().any(|term| term.text == args.anchor) {
        terms.insert(0, Term { text: args.anchor.clone(), kind: "anchor".to_string() });
    }
    let mut client = Client::new(&args.net);
    let mut cache = Cache::open(args.cache.as_deref(), &args.scope)?;
    let anchor = args.anchor.as_str();
    let others: Vec<&str> = terms.iter().map(|term| term.text.as_str()).filter(|term| *term != anchor).collect();

    // Pass 1: everything against the anchor. A term in a batch whose anchor was squeezed by a much
    // bigger term (anchor mean under SQUEEZED) is measured again among small terms only.
    let first = measure(&mut client, &args.scope, &mut cache, anchor, &others, false)?;
    let squeezed: Vec<&str> = others
        .iter()
        .zip(&first)
        .filter(|(_, m)| m.anchor < SQUEEZED && m.raw < 2.0 * m.anchor)
        .map(|(term, _)| *term)
        .collect();
    if !squeezed.is_empty() {
        eprintln!("rank: {} terms shared a batch with a much bigger term, measuring them again", squeezed.len());
        measure(&mut client, &args.scope, &mut cache, anchor, &squeezed, true)?;
    }
    let mut ratios: HashMap<&str, (f64, Option<Measure>, &str)> = HashMap::new();
    ratios.insert(anchor, (1.0, None, anchor));
    for term in &others {
        let m = cache.get(anchor, term).ok_or_else(|| anyhow!("{term:?} was not measured"))?;
        ratios.insert(term, (m.raw / m.anchor, Some(m), anchor));
    }

    // Pass 2: the long tail against a smaller anchor, chained through its own ratio.
    if let Some(low) = args.low_anchor.as_deref() {
        let (low_ratio, low_measure, _) = *ratios.get(low).ok_or_else(|| anyhow!("--low-anchor {low:?} is not in the list"))?;
        if low_measure.is_some_and(|m| m.raw < 10.0) {
            eprintln!("rank: warning: the low anchor {low:?} averaged under 10 next to {anchor:?}");
        }
        let tail: Vec<&str> = others
            .iter()
            .copied()
            .filter(|term| *term != low && ratios[term].1.is_some_and(|m| m.raw < args.refine_below))
            .collect();
        eprintln!("rank: {} terms under {} points, measuring them against {low:?}", tail.len(), args.refine_below);
        for (term, m) in tail.iter().zip(measure(&mut client, &args.scope, &mut cache, low, &tail, false)?) {
            ratios.insert(term, (m.raw / m.anchor * low_ratio, Some(m), low));
        }
    }

    let scale = match args.scale_to.as_deref() {
        Some(to) => {
            let ratio = ratios.get(to).ok_or_else(|| anyhow!("--scale-to {to:?} is not in the list"))?.0;
            if ratio <= 0.0 {
                bail!("--scale-to {to:?} measured 0");
            }
            100.0 / ratio
        }
        None => 100.0,
    };
    let mut rows: Vec<Row> = terms
        .iter()
        .map(|term| {
            let (ratio, measure, against) = ratios[term.text.as_str()];
            Row { term, ratio, measure, against }
        })
        .collect();
    rows.sort_by(|a, b| b.ratio.total_cmp(&a.ratio).then_with(|| a.term.text.cmp(&b.term.text)));

    let mut tsv = String::from("rank\tsearch\tkind\tinterest\traw\tanchor_raw\tmeasured_against\n");
    for (i, row) in rows.iter().enumerate() {
        let (raw, anchor_raw) = row.measure.map_or((String::new(), String::new()), |m| (format!("{:.2}", m.raw), format!("{:.2}", m.anchor)));
        writeln!(tsv, "{}\t{}\t{}\t{:.1}\t{raw}\t{anchor_raw}\t{}", i + 1, row.term.text, row.term.kind, row.ratio * scale, row.against)?;
    }
    match &args.out {
        Some(path) => std::fs::write(path, &tsv).with_context(|| format!("writing {}", path.display()))?,
        None => print!("{tsv}"),
    }
    if let Some(path) = &args.md {
        if client.range.is_empty() {
            // Everything came from the cache: one explore tells which dates "today 12-m" means.
            client.range = resolved_range(&explore(&mut client, &args.scope, &[anchor])?).unwrap_or_default();
        }
        let scale_term = args.scale_to.as_deref().unwrap_or(anchor);
        let mut md = format!(
            "<!-- generated by `trends rank` (crates/trends): edit the term list and run it again, never by hand -->\n\
             # {}\n\nGoogle Trends interest, {} (fetched {}). 100 = `{scale_term}`. A row counts every \
             search containing all of its words, in any order (words in double quotes as that exact \
             phrase); `a + b` counts either. Method and caveats: \
             [docs/search-demand.md](../search-demand.md).\n\n| # | search | kind | interest |\n|---:|---|---|---:|\n",
            args.title,
            args.scope.describe(&client.range),
            today(),
        );
        for (i, row) in rows.iter().enumerate() {
            let interest = row.ratio * scale;
            let interest = if interest >= 10.0 { format!("{interest:.0}") } else { format!("{interest:.1}") };
            writeln!(md, "| {} | {} | {} | {interest} |", i + 1, row.term.text, row.term.kind)?;
        }
        std::fs::write(path, md).with_context(|| format!("writing {}", path.display()))?;
    }
    eprintln!("rank: {} terms, {} requests", rows.len(), client.requests);
    Ok(())
}

/// Today's date (UTC), `YYYY-MM-DD`.
fn today() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
