//! The static pages: the index (the filtering UI's shell), one page per build, one per
//! extendable family card, the family index, a static list of everything (no script needed),
//! the about page and the 404 page. Every page is plain HTML5 with relative links (the site is a
//! GitHub project page under /redstonepedia/); viewers are `data-mcview` elements that
//! webrender's bundle (viewer/mcview.js) renders when present.

use site_model::html::{build_card, esc, kind_title, member_href};
use site_model::{Build, Card, Member, Site, flag_label, real_time, status_label};

pub const REPO_URL: &str = "https://github.com/vjeux/redstonepedia";
pub const SITE_NAME: &str = "REDSTONEPEDIA";

/// What a page loads besides its HTML.
#[derive(Clone, Copy, Default)]
pub struct Scripts {
    /// webrender's viewer bundle (viewer/mcview.js): only when the site has one.
    pub viewer: bool,
    /// The filtering UI (pkg/redstonepedia.js) on this page.
    pub app: bool,
    /// Whether the site has the UI at all (pages with in-place swaps load it too).
    pub ui: bool,
}

/// A whole page around `body`. `prefix` is the path from the page to the site root.
pub fn shell(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site) -> String {
    shell_full(title, description, prefix, body, scripts, site, None, None)
}

/// [`shell`] with a preview image for link unfurls (Open Graph).
pub fn shell_with(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site, image: Option<&str>) -> String {
    shell_full(title, description, prefix, body, scripts, site, image, None)
}

/// The header (vjeux, 01:13Z: "clean up the top"): the title, the Doors / Clocks / Circuits tabs
/// (`kind`: the page's own, shown active) and the search box (a plain form to the index, which
/// the index's UI turns into its live search; on the Clocks tab it also takes a period).
pub fn header(prefix: &str, kind: Option<&str>) -> String {
    header_with(prefix, kind, None)
}

/// What each tab's one card holds: the door records, the clock periods, the circuit kinds.
fn kind_counts(site: &Site) -> [usize; 3] {
    site.holds()
}

/// The tabs' counts, computed once per build.
fn tab_counts(site: &Site) -> [usize; 3] {
    static COUNTS: std::sync::OnceLock<[usize; 3]> = std::sync::OnceLock::new();
    *COUNTS.get_or_init(|| kind_counts(site))
}

/// [`header`] with the tabs' counts: every page's header reads like the index's (critique C44:
/// the study pages' tabs lost their counts and the search said just "Search").
pub fn header_with(prefix: &str, kind: Option<&str>, counts: Option<[usize; 3]>) -> String {
    let tab = |k: &str, label: &str, frag: &str, n: Option<usize>| -> String {
        let count = n.map(|n| format!(" <span class=\"count\">{n}</span>")).unwrap_or_default();
        format!("<a id=\"tab-{k}\" class=\"tab-top{}\" href=\"{prefix}index.html#{frag}\">{label}{count}</a>", if kind == Some(k) { " on" } else { "" })
    };
    let hidden = match kind {
        Some(k) => format!("<input type=\"hidden\" name=\"kind\" value=\"{}\">", esc(k)),
        None => String::new(),
    };
    // the index UI's own placeholders (web/site's render)
    let placeholder = match kind {
        Some("clock") => "Search: 48 ticks, 2.5 s…",
        Some("door") => "Search doors",
        Some("circuit") => "Search: toggle, timer, 12…",
        _ => "Search every build",
    };
    format!(
        "<header class=\"top\"><a class=\"logo\" href=\"{prefix}index.html\"><span class=\"logo-dot\"></span>{SITE_NAME}</a><nav class=\"tabs-top\" aria-label=\"kinds\">{}{}{}</nav>\
         <form id=\"hsearch\" class=\"hsearch\" action=\"{prefix}index.html\" method=\"get\" role=\"search\">{hidden}<input id=\"hq\" name=\"q\" type=\"search\" placeholder=\"{placeholder}\" aria-label=\"Search\" autocomplete=\"off\"></form></header>\n",
        tab("door", "Doors", "kind=door", counts.map(|c| c[0])),
        tab("clock", "Clocks", "kind=clock", counts.map(|c| c[1])),
        tab("circuit", "Circuits", "kind=circuit", counts.map(|c| c[2]))
    )
}

/// [`shell`] with everything: the preview image and the page's kind (its header tab).
#[allow(clippy::too_many_arguments)]
pub fn shell_full(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site, image: Option<&str>, kind: Option<&str>) -> String {
    let mut s = String::with_capacity(body.len() + 4096);
    // the viewer fetches the game's textures from the vendored set served with the site
    // (data-mcview-textures: the page's path back to the site root + textures/26.3)
    s.push_str(&format!("<!DOCTYPE html>\n<html lang=\"en\" data-mcview-textures=\"{prefix}{}\">\n<head>\n<meta charset=\"utf-8\">\n", mcview::atlas::vanilla::SITE_DIR));
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let full_title = if title == SITE_NAME { title.to_string() } else { format!("{title} · {SITE_NAME}") };
    s.push_str(&format!("<title>{}</title>\n", esc(&full_title)));
    s.push_str(&format!("<meta name=\"description\" content=\"{}\">\n", esc(description)));
    s.push_str(&format!("<meta property=\"og:title\" content=\"{}\">\n<meta property=\"og:description\" content=\"{}\">\n<meta property=\"og:site_name\" content=\"{SITE_NAME}\">\n", esc(&full_title), esc(description)));
    if let Some(img) = image {
        s.push_str(&format!("<meta property=\"og:image\" content=\"{}\">\n<meta name=\"twitter:card\" content=\"summary_large_image\">\n", esc(img)));
    }
    s.push_str(&stylesheet(prefix));
    s.push_str(&format!("<link rel=\"icon\" href=\"{prefix}favicon.svg\" type=\"image/svg+xml\">\n"));
    s.push_str("</head>\n<body>\n");
    s.push_str(&header_with(prefix, kind, Some(tab_counts(site))));
    s.push_str("<main>\n");
    s.push_str(body);
    s.push_str("\n</main>\n");
    // no page links in the footer (vjeux 13:35Z: "it's not to put them back in the footer"); the
    // date without the time, so it does not wrap on a phone (critique C52)
    s.push_str(&format!(
        "<footer>Built from <a href=\"{REPO_URL}/tree/{}\">vjeux/redstonepedia@{}</a> on <span class=\"nowrap\">{}</span>. Every design was tested in the real game (Minecraft 26.3, tick by tick); the extended clocks listed under a period are longer versions of a tested clock, each tested in the game or simulated to exactly its period.</footer>\n",
        esc(&site.commit),
        esc(&site.commit.chars().take(9).collect::<String>()),
        esc(site.generated.split('T').next().unwrap_or(&site.generated))
    ));
    // module specifiers must start with ./ or ../ to be relative
    let module = if prefix.is_empty() { "./" } else { prefix };
    // cache-busting (GitHub Pages serves max-age=600): every bundle by a content hash, the wasm
    // passed to init explicitly (its default URL would drop the query)
    let v = versions();
    if scripts.app {
        s.push_str(&format!(
            "<script type=\"module\">import init from \"{module}pkg/redstonepedia.js?v={}\"; init({{module_or_path: \"{module}pkg/redstonepedia_bg.wasm?v={}\"}});</script>\n",
            v.ui_js, v.ui_wasm
        ));
    }
    if scripts.viewer {
        s.push_str(&format!(
            "<script type=\"module\">import init from \"{module}viewer/mcview.js?v={}\"; init({{module_or_path: \"{module}viewer/mcview_bg.wasm?v={}\"}});</script>\n",
            v.viewer_js, v.viewer_wasm
        ));
    }
    s.push_str("</body>\n</html>\n");
    s
}

/// The stylesheet's `<link>` (its URL versioned by its content hash: cache-busting).
pub fn stylesheet(prefix: &str) -> String {
    let c = versions().css;
    format!("<link rel=\"stylesheet\" href=\"{prefix}style.css{}\">\n", if c.is_empty() { String::new() } else { format!("?v={c}") })
}

/// Content hashes of the files pages load by a fixed URL (`?v=<hash>`), set by `site build`
/// once the files are written (empty: no query).
#[derive(Clone, Debug, Default)]
pub struct Versions {
    pub css: String,
    pub viewer_js: String,
    pub viewer_wasm: String,
    pub ui_js: String,
    pub ui_wasm: String,
    pub data: String,
}

static VERSIONS: std::sync::RwLock<Option<Versions>> = std::sync::RwLock::new(None);

pub fn set_versions(v: Versions) {
    if let Ok(mut g) = VERSIONS.write() {
        *g = Some(v);
    }
}

fn versions() -> Versions {
    VERSIONS.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// The first 8 hex digits of a file's FNV-1a hash ("" when the file is missing).
pub fn file_hash(path: &std::path::Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => bytes_hash(&bytes),
        Err(_) => String::new(),
    }
}

pub fn bytes_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", h >> 32)
}

fn row(k: &str, v: &str) -> String {
    format!("<tr><th scope=\"row\">{}</th><td>{v}</td></tr>\n", esc(k))
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn chips(items: &[String], prefix: &str, param: &str) -> String {
    items
        .iter()
        .map(|k| format!("<a class=\"chip\" href=\"{prefix}index.html#{param}={}\">{}</a>", site_model::encode(k), esc(&k.replace('_', " "))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn grid(cards: &[String]) -> String {
    format!("<div class=\"grid\">{}</div>\n", cards.concat())
}

/// A circuit page's sections after the viewer: how to try it, the judge's test as a timing
/// diagram, the measurements with their meanings.
fn circuit_sections(c: &site_model::Circuit, has_viewer: bool) -> String {
    let mut s = format!("<section><h2>How to try it</h2>{}</section>\n", crate::explain::how_to_try(c, has_viewer));
    // the rows named as the 3D view names its buttons and tags (the lock: DIAL 1 to CODE 3, the
    // test's chart said D0 to K2 under a Try it naming the dials)
    let labeled = c.wave.as_ref().map(|w| {
        let mut w = w.clone();
        if c.input_labels.len() == c.inputs.len() {
            for r in &mut w.rows {
                if let Some(l) = c.inputs.iter().position(|n| *n == r.name).and_then(|i| c.input_labels.get(i)).filter(|l| !l.is_empty()) {
                    r.name = l.clone();
                }
            }
        }
        w
    });
    if let Some(w) = &labeled {
        let has_pos = w.rows.iter().any(|r| r.scale > 0);
        let pos_text = if !has_pos {
            String::new()
        } else if c.kind.starts_with("piston-extender") {
            " <span class=\"tag-pos\">Blue</span>: where the pushed block is, 0 = home, up to N = fully out (the dashed line); it must reach N while the input is on and come all the way back.".to_string()
        } else {
            " <span class=\"tag-pos\">Blue</span>: how far the machine has moved along its path, in blocks.".to_string()
        };
        let notes: String = w.rows.iter().filter(|r| !r.note.is_empty()).map(|r| format!("<p class=\"wave-note\"><span class=\"tag-pos\">{}</span>: {}.</p>", esc(&r.name.to_uppercase()), esc(&r.note))).collect();
        let has_marks = w.rows.iter().any(|r| !r.marks.is_empty());
        let intro = if has_marks {
            // a BUD switch: the judge changes its target cell, it has no inputs
            "The judge changes the target cell with this schedule (the TARGET row: <span class=\"mk-place-t\">green</span> = a block placed or changed, <span class=\"mk-clear-t\">red</span> = removed, <span class=\"mk-shape-t\">purple</span> = a shape-only change, <span class=\"mk-update-t\">blue</span> = a pure update with nothing changing; each phase's name says which test it is) and checks the output after every change; here it is simulated. <span class=\"tag-out\">Orange</span>: the output is on.".to_string()
        } else {
            // (a demo the view stops early does not play the same: said where it stops)
            let view = if c.view_halt.is_empty() {
                "the 3D view's demo plays the same".to_string()
            } else {
                match crate::explain::halt_words(&c.view_halt).0 {
                    Some(t) => format!("the 3D view stops at tick {t} of it"),
                    None => "the 3D view stops early in it".to_string(),
                }
            };
            format!(
                "The judge drives the input{} with this schedule and checks the output after every change; here it is simulated ({view}). <span class=\"tag-in\">Lime</span>: an input is on; <span class=\"tag-out\">orange</span>: the output is on (taller = a stronger signal, 1 to 15).{pos_text}",
                if c.inputs.len() == 1 { "" } else { "s" }
            )
        };
        s.push_str(&format!(
            "<section><h2>Our test in the real game</h2><p>{intro} Time runs left to right in game ticks (20 gt = 1 s).</p><div class=\"wave-wrap\">{}</div><p class=\"hint phone-only\">Swipe the diagram sideways: the whole test lasts {} ticks ({}).</p>{notes}{}</section>\n",
            crate::explain::wave_svg(w),
            w.ticks,
            site_model::real_time(w.ticks),
            if c.table.is_empty() {
                String::new()
            } else {
                format!("<details><summary>Our test's report, step by step ({} lines)</summary><pre class=\"design\">{}</pre></details>", c.table.len(), esc(&c.table.join("\n")))
            }
        ));
    }
    if !c.metrics.is_empty() {
        let rows: String = c
            .metrics
            .iter()
            .map(|(k, v)| {
                let def = crate::explain::metric_definition(&c.kind, k);
                // (a crop farm's "per cane" metrics are per plant: a bamboo stalk, a kelp plant)
                let plant = site_model::html::plant_word(c);
                let k = if c.kind.starts_with("harvester") && plant != "cane" { k.replace("cane", plant) } else { k.clone() };
                let k = &k;
                format!("<tr><th scope=\"row\">{}</th><td>{}</td><td class=\"def\">{}</td></tr>\n", esc(k), esc(&crate::explain::metric_value(&if k == "badges" { v.replace('+', ", ") } else { v.clone() })), esc(def))
            })
            .collect();
        s.push_str(&format!(
            "<section><h2>Measurements</h2><p class=\"hint\">{}</p><table class=\"props metrics\"><thead><tr><th>metric</th><th>value</th><th>what it means</th></tr></thead><tbody>{rows}</tbody></table>{}</section>\n",
            // (a whole minecart line: one end-to-end run, not 4 facings at 4 spots; sorter
            // 2026-10-22: the game and our simulator give the same numbers)
            if site_model::html::cart_line(c) {
                "Measured by our test of the whole line, run on our simulator, which gives the same numbers as the real game for this line: the supply double chest filled with 3,456 items, one empty hopper minecart started on the track, every item counted until the last one is in the storage chest."
            } else {
                "Measured in our test in the real game, facing all 4 ways at 4 spots in the world. gt = game tick = 1/20 s (20 gt = 1 second)."
            },
            // (what the kind's obvious design takes: the body only, the coordinator 2026-10-22)
            crate::explain::obvious_line(c).map(|l| format!("<p>{}</p>", esc(l))).unwrap_or_default()
        ));
    }
    s
}

/// The viewer of a build page (a live view when the viewer bundle is there; the interim picture
/// or the fallback text until then). Clocks run by themselves: their viewer starts playing.
fn viewer(prefix: &str, b: &Build) -> String {
    let trace = b.trace.as_ref().map(|t| format!(" data-trace=\"{}\"", esc(&format!("{prefix}{t}")))).unwrap_or_default();
    // circuits: the judge's test as the viewer's demo (webrender's contract: data-stimulus,
    // data-phases, data-input-mode), playing from the start
    let circuit = match &b.circuit {
        Some(c) => {
            let mut a = String::new();
            if !c.stimulus.is_empty() {
                a.push_str(&format!(" data-stimulus=\"{}\"", esc(&c.stimulus)));
            }
            // a strength input's values (the hex kinds: webrender 2e541dc02)
            if !c.levels.is_empty() {
                a.push_str(&format!(" data-levels=\"{}\"", esc(&c.levels)));
            }
            // the BUD's judge acts at its target cells, not at inputs: `data-events`
            if !c.events.is_empty() {
                a.push_str(&format!(" data-events=\"{}\"", esc(&c.events)));
            }
            if !c.stimulus.is_empty() || !c.events.is_empty() {
                a.push_str(&format!(" data-phases=\"{}\"", esc(&c.phases)));
            }
            a.push_str(&format!(" data-input-mode=\"{}\" data-circuit=\"{}\"", crate::explain::input_mode(&c.kind), esc(&c.kind)));
            // (a big crop farm's judge's speed: the view replays the judge's run at it, webrender
            // 2026-10-22)
            if let Some(rts) = c.farm_rts {
                a.push_str(&format!(" data-farm-rts=\"{rts}\""));
            }
            // (a design the build found the browser cannot run, the shulker study's own runs
            // aside: the view stays its still, nothing to find out on play; web stall, w86's
            // bamboo sweepers: one 550 ms task when play was pressed)
            if !c.unmodeled.is_empty() && !c.kind.starts_with("shulker") {
                a.push_str(" data-asbuilt=\"\"");
            }
            a
        }
        None => String::new(),
    };
    // nothing plays at once (vjeux 21:29Z, "Why is it simulating everything?"): the view starts on
    // play; the page says what to press (`plain::try_it`)
    let autoplay = "";
    let (pic, cls) = match &b.picture {
        Some(p) => (format!("<img class=\"pic contain\" src=\"{}\" alt=\"{} in the game\" decoding=\"async\">", esc(p), esc(&page_title(b))), " has-pic"),
        None => (String::new(), ""),
    };
    format!(
        "<div class=\"viewer k-{}\" data-mcview=\"full\" data-src=\"{}\"{trace}{circuit}{autoplay} data-kind=\"{}\"><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span><span class=\"fb-note\">{}</span></div></div>\n",
        esc(&b.kind),
        // (a display's lit still, when it has one: its view starts on the settled digit and
        // plays from there; the downloads below stay the design as built)
        esc(&format!("{prefix}{}", b.circuit.as_ref().map(|c| c.view_src.as_str()).filter(|s| !s.is_empty()).unwrap_or(&b.design))),
        esc(&b.kind),
        esc(&site_model::html::headline(b)),
        esc(&site_model::html::props(b).kind),
        if b.picture.is_some() { "picture from the real game; the live 3D view replaces it when available" } else { "3D view loading (or not available in this browser)" }
    )
}

/// An extended clock's line (critique: its page said only "a longer version of <id> (n = 15)"):
/// its tested base by name, linked; how many steps longer it is and what one step adds (the
/// family's rule, the ticks and blocks it adds); what checked this length.
fn extended_line(site: &Site, b: &Build, c: &site_model::ClockInfo, prefix: &str) -> String {
    let Some(e) = c.extend.as_ref().filter(|e| e.base != b.id) else { return String::new() };
    let Some(base) = site.builds.iter().find(|o| o.id == e.base) else { return String::new() };
    let Some(bc) = base.clock.as_ref() else { return String::new() };
    let steps = c.n.unwrap_or(0) - e.base_n;
    if steps <= 0 {
        return String::new();
    }
    let t = |n: i64| crate::plain::ticks(&format!("{n} gt"));
    let name = if e.title.is_empty() { format!("clock made of {}", site_model::html::human_blocks(&base.group)) } else { format!("{} clock", e.title) };
    let dt = c.every() as i64 - bc.every() as i64;
    let db = b.blocks as i64 - base.blocks as i64;
    let adds = if dt > 0 && dt % steps == 0 && db >= 0 && db % steps == 0 {
        if db == 0 { format!(", which adds {} (same blocks)", t(dt / steps)) } else { format!(", which adds {} and {}", t(dt / steps), site_model::html::blocks_word((db / steps) as u32)) }
    } else {
        String::new()
    };
    let rule = if e.rule.is_empty() { String::new() } else { format!(" Each step: {}{adds}.", e.rule) };
    let checked = if b.flags.iter().any(|f| f == "simulated") { "This length was simulated to exactly its period; the base design was tested in the real game." } else { "Tested in the real game too." };
    format!(
        "<p class=\"extended\">A longer version of the tested <a href=\"{prefix}{}\">{}</a>, which pulses every {} with {}; this one is {steps} step{} longer (n = {}).{} {checked}</p>\n",
        esc(&base.page),
        esc(&name),
        esc(&site_model::html::keep_units(&t(bc.every() as i64))),
        esc(&site_model::html::blocks_word(base.blocks)),
        if steps == 1 { "" } else { "s" },
        c.n.unwrap_or(0),
        esc(&site_model::html::keep_units(&rule))
    )
}

/// The page of one build.
/// The path from a page (`doors/x.html`, `circuits/gates/y.html`) back to the site root.
pub fn prefix_of(page: &str) -> String {
    "../".repeat(page.matches('/').count())
}

pub fn build_page(site: &Site, b: &Build, design_text: &str, repo_path: &str, scripts: Scripts) -> String {
    let prefix_s = prefix_of(&b.page);
    let prefix = prefix_s.as_str();
    let mut body = String::new();
    if let Some(c) = &b.circuit {
        // circuits: Circuits › study (the kind is the line right under: critique, "Piston
        // extender" three times at the top)
        let (study_title, _) = crate::explain::study(&c.study).unwrap_or(("Circuits", ""));
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › <a href=\"{prefix}circuits/{}/index.html\">{}</a></p>\n",
            esc(&c.study),
            esc(study_title)
        ));
    } else {
        // the last crumb: a door's category; a clock's period, as on the Clocks tab (critique: the
        // parts list was said again right under the breadcrumb)
        let (frag, label) = match (b.kind.as_str(), b.clock.as_ref()) {
            ("clock", Some(c)) => (format!("kind=clock&time={}", c.every()), format!("every {}", site_model::gt_time(c.every()))),
            ("door", _) => {
                let w = site_model::html::door_category(&b.group);
                (format!("kind=door&group={}", site_model::encode(&b.group)), if w.is_empty() { b.group.clone() } else { w.to_string() })
            }
            _ => (format!("kind={}&group={}", b.kind, site_model::encode(&b.group)), b.group.clone()),
        };
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind={}\">{}</a> › <a href=\"{prefix}index.html#{}\">{}</a></p>\n",
            esc(&b.kind),
            kind_title(&b.kind),
            esc(&frag),
            esc(&site_model::html::keep_units(&label))
        ));
    }
    body.push_str(&page_heading(b, design_text));
    let badges = site_model::html::badges(b);
    if !badges.is_empty() {
        body.push_str(&format!("<p class=\"badges\">{}</p>\n", badges.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect::<String>()));
    }
    // the redstone first (vjeux: "Put the redstone before the explanation"): the live view and
    // its controls, a tunable group's picker under it, then how to try it (no "why build it":
    // vjeux 23:52Z); the jargon, our test and the measurements fold into one "Details" (13:06Z)
    body.push_str(&viewer(prefix, b));
    let picker = group_picker(site, b, prefix);
    // the picker swaps versions in place (pkg/redstonepedia.js), keeping the viewer's camera
    let scripts = Scripts { app: scripts.ui && scripts.viewer && !picker.is_empty(), ..scripts };
    if b.circuit.is_some() {
        body.push_str(&picker);
    }
    // what it needs where you build it (a height, water, a loading recipe), right under the view
    // (critique b0fb9ae4e: above it, the sorter's recipe pushed the view off a phone's first
    // screen)
    let needs = crate::plain::needs(b, design_text);
    if !needs.is_empty() {
        body.push_str(&format!("<p class=\"needs\">{}</p>\n", esc(&needs.join(" "))));
    }
    // one door card; every clock a card of its own (vjeux 01:24Z): no clock stands for others
    if b.door.is_some() {
        body.push_str(&door_table(site, b, prefix));
    }
    if let Some(c) = &b.clock {
        body.push_str(&extended_line(site, b, c, prefix));
        // the other clocks of its period: on the Clocks tab, under the period's heading
        body.push_str(&format!(
            "<p class=\"hint\">Every clock that pulses every {}: <a href=\"{prefix}index.html#kind=clock&amp;time={}\">on the Clocks tab</a>, smallest first (the ones hard to wire into a circuit last).</p>\n",
            esc(&site_model::html::keep_units(&site_model::gt_time(c.every()))),
            c.every()
        ));
    }
    body.push_str(&crate::plain::intro_box(b, scripts.viewer));
    if let Some(c) = &b.clock {
        body.push_str(&clock_timeline(design_text, c));
    }
    if let Some(v) = b.door.as_ref().and_then(|d| d.video.as_ref()) {
        body.push_str(&format!(
            "<section><h2>Video (real game)</h2><video class=\"video\" controls preload=\"none\" src=\"{}\"></video><p><a href=\"{}\">download the video</a></p></section>\n",
            esc(v),
            esc(v)
        ));
    }
    body.push_str(&format!(
        "<section><h2>Build it</h2><p><a class=\"button\" href=\"{prefix}{}\" download>Download .litematic</a> <a class=\"button alt\" href=\"{prefix}{}\" download>Download the design as text</a></p>\n<p class=\"hint\">The .litematic opens in <a href=\"https://github.com/maruohon/litematica\">Litematica</a> (Minecraft 26.3): every block of the design in one region.{} The text file is the same build written as plain text, block by block.</p>\n<p>Blocks used: {}</p></section>\n",
        esc(&crate::generate::litematic_path(&b.design)),
        esc(&b.design),
        if b.kind == "door" || b.circuit.is_some() || b.clock.is_some() { " Verified in the real game." } else { "" },
        chips(&b.kinds, prefix, "with")
    ));
    if let Some(c) = &b.clock {
        body.push_str(&same_mechanism(site, b, c, prefix));
    }
    // a pick that stands higher than the ground: the kind's design that sits right on it (vjeux
    // 11:49Z); a pick with a device of its own (a TNT duper's on/off switch, a flying machine's
    // start lever): the kind's design without it (the coordinator 14:08Z, 16:10Z); one line when
    // one design is both
    let find = |id: Option<&String>| id.and_then(|g| site.builds.iter().find(|o| &o.id == g));
    let ground = find(b.circuit.as_ref().and_then(|c| c.ground.as_ref()));
    let plain = find(b.circuit.as_ref().and_then(|c| c.no_switch.as_ref()));
    let device = crate::kinds::device_words(b);
    let link = |o: &Build| format!("<a href=\"{prefix}{}\">{}</a>", esc(&o.page), esc(&page_title(o)));
    match (ground, plain) {
        (Some(g), Some(p)) if g.id == p.id => body.push_str(&format!("<p class=\"also-ground\">For one that sits right on the ground, with no {device}: {}.</p>\n", link(g))),
        (g, p) => {
            if let Some(g) = g {
                body.push_str(&format!("<p class=\"also-ground\">For one that sits right on the ground: {}.</p>\n", link(g)));
            }
            if let Some(p) = p {
                let fewer = b.blocks.saturating_sub(p.blocks);
                let smaller = if fewer > 0 { format!(", {fewer} block{} smaller", if fewer == 1 { "" } else { "s" }) } else { String::new() };
                body.push_str(&format!("<p class=\"also-ground\">Without the {device}{smaller}: {}.</p>\n", link(p)));
            }
        }
    }
    // a piston extender whose lever must not be flipped back early: its size's flip-safe design
    // (the coordinator 2026-10-21)
    if let Some(f) = find(b.circuit.as_ref().and_then(|c| c.flip_safe.as_ref())) {
        let more = f.blocks.saturating_sub(b.blocks);
        let bigger = if more > 0 { format!(", {more} block{} bigger", if more == 1 { "" } else { "s" }) } else { String::new() };
        body.push_str(&format!("<p class=\"also-ground\">Safe to flip back any time{bigger}: {}.</p>\n", link(f)));
    }
    // a piston bridge's gap with the other walkway (slime / stone), both linked (pext)
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.alt_walkway.as_ref())) {
        let word = o.circuit.as_ref().map(site_model::html::walkway_word).unwrap_or_default();
        let d = o.blocks as i64 - b.blocks as i64;
        let n = d.unsigned_abs();
        let diff = match d {
            0 => String::new(),
            d if d > 0 => format!(", {n} block{} more", if n == 1 { "" } else { "s" }),
            _ => format!(", {n} block{} fewer", if n == 1 { "" } else { "s" }),
        };
        body.push_str(&format!("<p class=\"also-ground\">With a {} walkway{diff}: {}.</p>\n", esc(&word), link(o)));
    }
    // an auto-smelter's size with the other furnace type, both linked: the blast-furnace version
    // twice as fast on ores, the plain one smelting anything (the coordinator 2026-10-22)
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.blast_twin.as_ref())) {
        let blast = o.circuit.as_ref().is_some_and(site_model::html::smelter_blast);
        let d = o.blocks as i64 - b.blocks as i64;
        let n = d.unsigned_abs();
        let diff = match d {
            0 => String::new(),
            d if d > 0 => format!(", {n} block{} more", if n == 1 { "" } else { "s" }),
            _ => format!(", {n} block{} fewer", if n == 1 { "" } else { "s" }),
        };
        let what = if blast { "With blast furnaces, twice as fast on ores and raw metals (they smelt nothing else)" } else { "With plain furnaces, which smelt anything (sand, cobblestone, logs and food too)" };
        // (the link's own title says its double chest's minutes)
        body.push_str(&format!("<p class=\"also-ground\">{what}{diff}: {}.</p>\n", link(o)));
    }
    // a crop farm whose pick sweeps its field with a flying machine and its crop's best farm
    // without one: each page says which to build and why (the coordinator 2026-10-22)
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.machine_twin.as_ref()))
        && let (Some(c), Some(oc)) = (b.circuit.as_ref(), o.circuit.as_ref())
    {
        let me_machine = c.flying_machines > 0;
        let (m, s, mb, sb) = if me_machine { (c, oc, b, o) } else { (oc, c, o, b) };
        // (a fuel farm: what each block's harvest smelts, the coordinator 2026-10-22; else the
        // harvest per block)
        let fuel = site_model::html::fuel_rates(m).is_some() && site_model::html::fuel_rates(s).is_some();
        let rate = |x: &site_model::Circuit| if fuel { site_model::html::fuel_rates(x).map(|r| r.0).unwrap_or(0.0) } else { site_model::html::farm_rates(x).map(|r| r.0).unwrap_or(0.0) };
        let mut gains: Vec<String> = Vec::new();
        if rate(m) > rate(s) {
            if fuel {
                gains.push(format!("fuels more for each block placed ({} items smelted an hour against {}, {:.1} times as much)", site_model::html::readable(rate(m)), site_model::html::readable(rate(s)), rate(m) / rate(s).max(1e-9)));
            } else {
                gains.push(format!("gathers more for each block placed ({} against {})", site_model::html::readable(rate(m)), site_model::html::readable(rate(s))));
            }
        }
        if mb.blocks < sb.blocks {
            gains.push(format!("takes fewer blocks ({} against {})", site_model::html::group_thousands(mb.blocks as u64), site_model::html::group_thousands(sb.blocks as u64)));
        }
        let gains = if gains.is_empty() { "ranks first".to_string() } else { gains.join(" and ") };
        let slime = site_model::html::bill_slime(m).filter(|n| *n >= 9).map(|n| format!("{n} slime and ")).unwrap_or_default();
        let still = if site_model::html::bill_slime(s).is_none_or(|n| n < 9) { "no flying machine and no slime blocks" } else { "no flying machine" };
        // (the farm without a machine is the obvious design, the bar the pick is measured against:
        // its caption says so, the coordinator 2026-10-22, "bamboo-line64 shown as the bar")
        let bar = sb.caption.starts_with("The obvious ");
        let text = if me_machine {
            let what = if bar { format!("The obvious {}, the bar it is measured against, has {still}", if fuel { "fuel farm" } else { "farm" }) } else { format!("For a farm with {still}") };
            format!("<b>Which to build:</b> this farm {gains}, but it costs {slime}a flying machine that travels its field. {what}: {}.", link(o))
        } else if bar {
            format!("<b>The bar:</b> this is the obvious design, the one the pick is measured against; it has {still}. The farm that sweeps its field with a flying machine {gains}, for {slime}its flying machine: {}.", link(o))
        } else {
            format!("<b>Which to build:</b> this farm has {still}. The farm that sweeps its field with a flying machine {gains}, for {slime}its flying machine: {}.", link(o))
        };
        body.push_str(&format!("<p class=\"also-ground\">{text}</p>\n"));
    }
    // an automatic door that closes on someone standing in the doorway and its kind's best door
    // that stays open for them: each page says which to build and why, then the plain door that
    // needs no pistons (the coordinator 2026-10-22, the reviewer's page must-haves)
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.doorway_twin.as_ref()))
        && let (Some(c), Some(oc)) = (b.circuit.as_ref(), o.circuit.as_ref())
    {
        let blocks = |x: &Build| site_model::html::keep_units(&site_model::html::blocks_word(x.blocks));
        // (how long you wait at the door, in words: "opens before you reach it, even at a sprint")
        let wait = |x: &site_model::Circuit| match site_model::html::door_open_wait(x) {
            Some(0) => "opens before you reach it, even at a sprint".to_string(),
            Some(w) => format!("makes you wait up to {} at the door", site_model::real_time(w)),
            None => "opens as you walk up".to_string(),
        };
        let text = if site_model::html::door_stays_open(c) == Some(true) {
            // (clk-obs's line for lock-v12's page, then the door to take when nobody will stand in
            // the doorway: its link says its blocks and its wait)
            format!(
                "<b>Which to build:</b> this door also stays open for someone standing in the doorway: {}. If nobody will stand in your doorway, the door that closes on them is smaller and {}: {}.",
                esc(&blocks(b)),
                esc(&wait(oc)),
                link(o)
            )
        } else {
            format!(
                "<b>Which to build:</b> this door takes {} and {}, but it closes on anyone who stops in the doorway. For a door that also stays open for someone standing in the doorway: {}.",
                esc(&blocks(b)),
                esc(&wait(c)),
                link(o)
            )
        };
        body.push_str(&format!("<p class=\"also-ground\">{text} {}</p>\n", esc(crate::plain::IRON_DOORS)));
    }
    // a whole minecart line: its stations, the loader first, each linked with what it does to
    // the cart; its track (sorter 2026-10-22)
    if let Some(c) = b.circuit.as_ref().filter(|c| site_model::html::cart_line(c)) {
        let rate = |o: &Build, k: &str| o.circuit.as_ref().and_then(|oc| oc.metrics.iter().find(|(n, _)| n == k)).and_then(|(_, v)| v.split_whitespace().next().map(str::to_string));
        let mut stations: Vec<(bool, String)> = Vec::new();
        for id in &c.parts {
            let Some(o) = find(Some(id)) else { continue };
            let unload = o.circuit.as_ref().is_some_and(|oc| oc.kind.starts_with("cart-unloader"));
            let what = match (unload, rate(o, if unload { "unload rate" } else { "load rate" })) {
                (true, Some(r)) => format!("the unloader, which empties the cart at {r} items a second"),
                (true, None) => "the unloader".to_string(),
                (false, Some(r)) => format!("the loader, which fills the cart at {r} items a second"),
                (false, None) => "the loader".to_string(),
            };
            stations.push((unload, format!("{} ({})", link(o), esc(&what))));
        }
        stations.sort_by_key(|(unload, _)| *unload);
        if !stations.is_empty() {
            // ("128 blocks, a powered rail every 8": the track's length, then its boosters)
            let track = c.metrics.iter().find(|(k, _)| k == "track").map(|(_, v)| match v.split_once(", ") {
                Some((len, rest)) => format!(", with {} of track between them, {} blocks", esc(len), esc(rest)),
                None => format!(", with {} of track between them", esc(v)),
            }).unwrap_or_default();
            body.push_str(&format!("<p class=\"also-ground\"><b>Its stations:</b> {}{track}.</p>\n", stations.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(" and ")));
        }
    }
    // a station of a published minecart line: the line, linked
    if let Some(l) = find(b.circuit.as_ref().and_then(|c| c.part_of.as_ref())) {
        body.push_str(&format!("<p class=\"also-ground\"><b>Part of a whole line:</b> {}.</p>\n", link(l)));
    }
    // a fuel farm and the super smelter it feeds, linked both ways (the coordinator 2026-10-22):
    // on the farm, the 8-furnace super smelter; on a super smelter, the fuel farm's pick and its
    // share of that smelter at full speed (a blast furnace burns its fuel twice as fast)
    if let Some(c) = b.circuit.as_ref()
        && site_model::html::fuel_rates(c).is_some()
        && let Some(s) = site.builds.iter().find(|o| o.circuit.as_ref().is_some_and(|oc| oc.kind == "smelter 8" && !site_model::html::smelter_blast(oc)))
    {
        // (its share of that smelter is its card's lead already)
        body.push_str(&format!("<p class=\"also-ground\"><b>For a super smelter:</b> run its lines into the fuel chest of {}.</p>\n", link(s)));
    }
    if let Some(c) = b.circuit.as_ref().filter(|c| c.kind.starts_with("smelter") && site_model::html::double_chest_minutes(c).is_some())
        && let Some(f) = site.builds.iter().find(|o| !o.flags.iter().any(|x| x == "size") && o.circuit.as_ref().is_some_and(|oc| oc.host.is_none() && site_model::html::fuel_rates(oc).is_some()))
        && let Some((_, total)) = f.circuit.as_ref().and_then(site_model::html::fuel_rates)
    {
        let furnaces: f64 = c.kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1.0);
        let blast = site_model::html::smelter_blast(c);
        let full = furnaces * 360.0 * if blast { 2.0 } else { 1.0 };
        let r = total / full;
        // (the farm's card measures it against the 8-furnace smelter: another size says its own
        // share)
        let share = if furnaces == 8.0 && !blast {
            String::new()
        } else if r >= 1.0 {
            ": enough to keep this smelter at full speed".to_string()
        } else if r >= 0.6 {
            format!(": {:.0}% of this smelter at full speed", (r * 100.0).floor())
        } else {
            format!(": {} of this smelter at full speed", site_model::html::share_words(r))
        };
        body.push_str(&format!("<p class=\"also-ground\"><b>Fuel it from a farm:</b> {}{}.</p>\n", link(f), esc(&share)));
    }
    // a crop farm's pick and its crop's farm on about half the field, both linked, each with what
    // it gathers (a fuel farm: what it smelts) and its blocks (the reviewer via the coordinator
    // 2026-10-22: "half the field: ~1,350 items smelted an hour · 1,468 blocks")
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.half_twin.as_ref()))
        && let (Some(c), Some(oc)) = (b.circuit.as_ref(), o.circuit.as_ref())
    {
        let what = |x: &site_model::Circuit, xb: &Build| {
            let rate = match (site_model::html::fuel_rates(x), site_model::html::farm_rates(x)) {
                (Some((_, t)), _) => format!("{} items smelted an hour", site_model::html::fuel_count(t)),
                (None, Some((_, t))) if t > 0.0 => format!("{} {} an hour", site_model::html::readable(t), site_model::html::harvest_words_at(x, t)),
                _ => String::new(),
            };
            let blocks = site_model::html::blocks_word(xb.blocks);
            if rate.is_empty() { blocks } else { format!("{rate} · {blocks}") }
        };
        let half = oc.crops < c.crops;
        let label = if half { "Half the field" } else { "The whole field" };
        body.push_str(&format!("<p class=\"also-ground\"><b>{label}:</b> <a href=\"{prefix}{}\">{}</a>.</p>\n", esc(&o.page), esc(&site_model::html::keep_units(&what(oc, o)))));
    }
    // a line's station beside its kind's pick, both linked: the faster one and the smaller one
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.station_twin.as_ref())) {
        let rate = |x: &Build| {
            let k = if x.circuit.as_ref().is_some_and(|c| c.kind.starts_with("cart-unloader")) { "unload rate" } else { "load rate" };
            x.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(n, _)| n == k)).and_then(|(_, v)| v.split_whitespace().next()?.parse::<f64>().ok())
        };
        let d = o.blocks as i64 - b.blocks as i64;
        let n = d.unsigned_abs();
        let blocks = format!("{n} block{}", if n == 1 { "" } else { "s" });
        let text = match (rate(o), rate(b)) {
            (Some(ro), Some(rb)) if ro > rb => format!("Faster, {ro} items a second against this one's {rb}{}: {}.", if d > 0 { format!(", for {blocks} more") } else { String::new() }, link(o)),
            (Some(ro), Some(rb)) => format!("Smaller{}, {ro} items a second against this one's {rb}: {}.", if d < 0 { format!(" by {blocks}") } else { String::new() }, link(o)),
            _ => format!("The other: {}.", link(o)),
        };
        body.push_str(&format!("<p class=\"also-ground\">{text}</p>\n"));
    }
    // a crop farm's full-size pick and its crop's best small module, both linked (the coordinator
    // via w86 2026-10-22: the modules stay as variants)
    if let Some(o) = find(b.circuit.as_ref().and_then(|c| c.farm_twin.as_ref())) {
        let full = b.circuit.as_ref().is_some_and(site_model::html::full_size_farm);
        // (the link's own title says its numbers)
        let what = if full { "For a small farm" } else { "The full-size farm" };
        body.push_str(&format!("<p class=\"also-ground\">{what}: {}.</p>\n", link(o)));
    }
    // a full-size farm that tiles: its family is its copies side by side (the coordinator
    // 2026-10-22: "the extendable tile is its family"; w86: the judge ran the copies side by side,
    // and the harvest per block stays one copy's)
    if let Some(c) = b.circuit.as_ref().filter(|c| site_model::html::full_size_farm(c))
        && let Some(step) = site_model::html::tile_step(c)
        && let Some((per_block, total)) = site_model::html::farm_rates(c).filter(|(_, t)| *t > 0.0)
    {
        let plant = site_model::html::plant_word(c);
        let harvest = site_model::html::harvest_words(c);
        let ku = site_model::html::keep_units;
        let mut rows = String::new();
        for n in [1u32, 2, 4, 8] {
            rows.push_str(&format!(
                "<tr{}><td>{n} cop{}</td><td>{} {plant}s</td><td>{}</td><td>{} {harvest}</td></tr>\n",
                if n == 1 { " class=\"on\"" } else { "" },
                if n == 1 { "y (this design)" } else { "ies" },
                site_model::html::group_thousands((c.crops * n) as u64),
                esc(&ku(&site_model::html::blocks_word(b.blocks * n))),
                site_model::html::readable(total * n as f64)
            ));
        }
        body.push_str(&format!(
            "<section class=\"group\" id=\"bigger\"><h2>Bigger farms</h2><p>Build copies side by side, one every {step} blocks (our test ran it that way too): each copy adds the same harvest, so every size gathers {} {harvest} an hour for each block placed.</p><div class=\"scroll-x\"><table class=\"list\"><thead><tr><th>copies</th><th>{plant}s</th><th>blocks</th><th>an hour</th></tr></thead><tbody>\n{rows}</tbody></table></div></section>\n",
            site_model::html::readable(per_block)
        ));
    }
    body.push_str("<details class=\"more\"><summary>Details: how it works in depth, our test in the real game, measurements and properties</summary>\n");
    if let Some(c) = &b.circuit {
        body.push_str(&format!("<section class=\"what\"><h2>In depth</h2>{}</section>\n", crate::explain::what_it_does(c)));
        body.push_str(&circuit_sections(c, scripts.viewer));
    }
    let notes = crate::explain::caption_plain(&b.caption);
    if !notes.is_empty() {
        body.push_str(&format!("<section><h2>Notes from its designer</h2><p>{notes}</p></section>\n"));
    }
    body.push_str("<section><h2>Properties</h2><table class=\"props\">\n");
    body.push_str(&properties(site, b, prefix, design_text));
    body.push_str("</table></section>\n");
    if let Some(d) = &b.door {
        if !d.props.is_empty() {
            body.push_str("<div class=\"allprops\"><details><summary>Every check of the community regulation</summary><table class=\"props small\">\n");
            for (k, v) in &d.props {
                if let Some((label, value)) = crate::explain::door_prop(k, v) {
                    body.push_str(&row(&label, &esc(&value)));
                }
            }
            body.push_str("</table></details></div>\n");
        }
    }
    body.push_str(&format!(
        "<section><h2>The design file</h2><p>{}</p><details><summary>Design file ({} lines)</summary><pre class=\"design\">{}</pre></details></section>\n",
        // (an extended clock's design is generated from its tested base at build time: no file
        // in the repository to link)
        if repo_path.is_empty() { "Generated from the tested design it extends, at build time.".to_string() } else { format!("<a href=\"{REPO_URL}/blob/main/{}\">on GitHub</a>", esc(repo_path)) },
        design_text.lines().count(),
        esc(design_text)
    ));
    body.push_str("</details>\n");
    if b.circuit.is_some() {
        body.push_str(&layouts(site, b, prefix));
    }
    let p = site_model::html::props(b);
    // (the caption a visitor reads: no track ids or tool logs in the search snippet either)
    let desc = format!("{}: {}. {}", p.kind, p.title, crate::explain::caption_text(&b.caption).chars().take(160).collect::<String>());
    shell_full(&page_title(b), &desc, prefix, &body, scripts, site, b.picture.as_deref(), Some(&b.kind))
}

/// A build page's `<title>`: its kind and properties first, the design's name after
/// (`Piston extender: N = 3 · 19 blocks (pext-n3-flat-cascade19)`).
pub fn page_title(b: &Build) -> String {
    let p = page_props(b);
    format!("{}: {}", p.kind, p.title)
}

/// A build's properties for its own page: the one clock card's page is that clock's page (its
/// own title, not the card's "a clock for every period"); the one door card's page drops the
/// card's "pick one on its page" (critique: it read odd on that very page).
fn page_props(b: &Build) -> site_model::html::Props {
    if b.clock.as_ref().is_some_and(|c| !c.kind_range.is_empty()) || b.door.as_ref().is_some_and(|d| d.records > 0) {
        let mut x = b.clone();
        if let Some(c) = x.clock.as_mut() {
            c.kind_range.clear();
        }
        if let Some(d) = x.door.as_mut() {
            d.records = 0;
        }
        return site_model::html::props(&x);
    }
    site_model::html::props(b)
}

/// A build page's heading (vjeux 12:09Z: lead with the properties; 13:06Z: plain words; 13:11Z:
/// no design id): its kind in words with the community's name (small, above), the property line
/// as the title, what it does in one plain sentence, then the secondary properties. The design's
/// name is only in the collapsed details, the URL and the downloads.
fn page_heading(b: &Build, design_text: &str) -> String {
    // the one clock card's page is still that clock's page: its own title, not the card's
    // "a clock for every period" (critique: 4 lines of giant type over a page about one clock)
    let p = page_props(b);
    let kind = match &b.circuit {
        Some(_) if b.group == "Sand elevator" => "Sand elevator: lifts a sand block and lets it fall back (gravity piston extender)".to_string(),
        // (flying machines by engine shape and the docking-station one: the card's own kind line)
        Some(c) if c.kind.starts_with("flying-machine") => p.kind.clone(),
        // (an adder: which one, from its kind key: every adder is `adder 1`; an automatic door's
        // size, a cart stop's line: `kinds::title_key`)
        Some(_) => crate::explain::kind_title(&crate::kinds::title_key(b)),
        None => p.kind.clone(),
    };
    let what = match (&b.circuit, b.kind.as_str()) {
        (Some(c), _) if b.group == "Sand elevator" => format!("Turn the input on and a block of sand is lifted {} blocks up; turn it off and it falls back into place.", c.kind.split_whitespace().nth(1).unwrap_or("2")),
        (Some(c), _) => crate::plain::one_liner(c),
        (None, "door") => crate::plain::door(b).0,
        // a family's base design says the range it makes, in its opening line (critique 41f1ec62a:
        // the hopper clock's "any period from 1 s to 4 min 15 s" sat at the bottom of its page)
        (None, "clock") => match crate::plain::slower_line(b) {
            Some(more) => format!("{} {more}", crate::plain::clock(b).0),
            None => crate::plain::clock(b).0,
        },
        _ => String::new(),
    };
    let warn = crate::plain::warning(b).map(|w| format!("<p class=\"warn\">{}</p>\n", esc(&w))).unwrap_or_default();
    // (what it needs where you build it: under the 3D view, critique b0fb9ae4e: the sorter's
    // loading recipe and its warning pushed the view off a phone's first screen; `needs_box`)
    let _ = design_text;
    // the secondary line without what the opening sentence already says (critique: "subtitles
    // restate the opening sentence"); a number with its unit, so "the first craft after 4 s" is
    // not taken for the "4 sand" of "every 5 gunpowder and 4 sand"
    let numbers = |s: &str| -> Vec<String> {
        let words: Vec<&str> = s.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',' || c == ';' || c == ':').filter(|w| !w.is_empty()).collect();
        words
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                let n = w.trim_matches(|c: char| !(c.is_ascii_digit() || c == '.')).trim_matches('.');
                (!n.is_empty() && n.starts_with(|c: char| c.is_ascii_digit())).then(|| format!("{n} {}", words.get(i + 1).map(|u| u.trim_end_matches(['.', ','])).unwrap_or("")))
            })
            .collect()
    };
    let said = numbers(&what);
    // (the claim, why this design is on the site, always stays: its "2 blocks bigger than the
    // pick of its size" is no repeat of the opening's "pushed 2 blocks out")
    let claim = b.circuit.as_ref().map(|c| c.claim.as_str()).unwrap_or("");
    let lede: Vec<&str> = p.sub.split(" · ").filter(|part| {
        if !claim.is_empty() && *part == claim {
            return true;
        }
        let n = numbers(part);
        n.is_empty() || !n.iter().all(|x| said.contains(x))
    }).collect();
    let lede = lede.join(" · ");
    // (no "Why build it" line: vjeux 23:52Z, "All these why build it have been super useless.
    // Please remove all of them")
    format!(
        "<p class=\"kindline\">{}</p>\n<h1>{}</h1>\n{}{warn}{}",
        esc(&kind),
        // (each number kept with its unit: critique, "2 / blocks" wrapped in the title)
        esc(&site_model::html::keep_units(&p.title)),
        if what.is_empty() { String::new() } else { format!("<p class=\"oneliner\">{}</p>\n", esc(&what)) },
        if lede.is_empty() { String::new() } else { format!("<p class=\"lede\">{}</p>\n", esc(&site_model::html::keep_units(&lede))) }
    )
}

fn link_build(site: &Site, id: &str, prefix: &str) -> String {
    match site.builds.iter().find(|b| b.id == id) {
        Some(b) => format!("<a href=\"{prefix}{}\">{}</a>", esc(&b.page), esc(&site_model::html::props(b).title)),
        None => esc(id.rsplit('/').next().unwrap_or(id)),
    }
}

/// How a clock's blocks differ from another's, in words: `copper bulb instead of note block`.
fn swap_words(own: &[String], other: &[String]) -> String {
    let words = |ids: &[String]| site_model::html::human_blocks(&ids.join("+"));
    let plus: Vec<String> = own.iter().filter(|k| !other.contains(k)).cloned().collect();
    let minus: Vec<String> = other.iter().filter(|k| !own.contains(k)).cloned().collect();
    match (plus.is_empty(), minus.is_empty()) {
        (false, false) => format!("{} instead of {}", words(&plus), words(&minus)),
        (false, true) => format!("{} too", words(&plus)),
        // (the same block count with a kind gone: another of its blocks fills that cell, "with no
        // piston · 7 blocks" read like a part missing)
        (true, false) => format!("another of its blocks in place of the {}", words(&minus)),
        (true, true) => "the same blocks, set another way".to_string(),
    }
}

/// A clock's mechanism (crates/site/src/mechanism.rs: the coordinator 11:33Z, "one card per
/// mechanism per period"): on its card's page, the clocks of the same period and mechanism with
/// other blocks, each with its own downloads; on a folded clock's page, the card it folds under.
fn same_mechanism(site: &Site, b: &Build, c: &site_model::ClockInfo, prefix: &str) -> String {
    if let Some(h) = c.host.as_ref().and_then(|h| site.builds.iter().find(|o| &o.id == h)) {
        return format!(
            "<section><h2>The same clock as another</h2><p>It is <a href=\"{prefix}{}\">this clock</a> ({}) with {}: the same mechanism and period. The Clocks tab lists it under that one.</p></section>\n",
            esc(&h.page),
            esc(&site_model::html::props(h).title),
            esc(&swap_words(&b.kinds, &h.kinds))
        );
    }
    let vs: Vec<&Build> = c.variants.iter().filter_map(|v| site.builds.iter().find(|o| &o.id == v)).collect();
    if vs.is_empty() {
        return String::new();
    }
    // the on-time per pulse (a variant whose cycle holds 2 pulses of the card's beat reads "on for
    // 18 of 38 ticks" beside the card's "9 of 19": the same pulse twice, not a slower clock)
    let per_pulse = |k: &site_model::ClockInfo| -> (u32, u32) {
        let n = k.edges.max(1);
        match k.pulse_every {
            Some(e) if n > 1 && k.on % n == 0 => (k.on / n, e),
            _ => (k.on, k.period),
        }
    };
    let rows: String = vs
        .iter()
        .map(|v| {
            let on = match v.clock.as_ref() {
                Some(vc) if vc.on > 0 && per_pulse(vc) != per_pulse(c) => {
                    let (on, of) = per_pulse(vc);
                    format!(" · on for {on} of {of} ticks")
                }
                _ => String::new(),
            };
            // (a variant with no spot to take its output into a circuit says so: the judge's
            // `usable`)
            let hard = if v.clock.as_ref().is_some_and(|vc| vc.usable.starts_with("no")) { format!(" · {}", site_model::html::HARD_TO_WIRE) } else { String::new() };
            let on = format!("{on}{hard}");
            format!(
                "<li><a href=\"{prefix}{}\">with {}</a> · {}{on} · <a href=\"{prefix}{}\" download>.litematic</a> · <a href=\"{prefix}{}\" download>.txt</a></li>",
                esc(&v.page),
                esc(&swap_words(&v.kinds, &b.kinds)),
                esc(&site_model::html::blocks_word(v.blocks)),
                esc(&crate::generate::litematic_path(&v.design)),
                esc(&v.design)
            )
        })
        .collect();
    let also = if c.swaps.is_empty() { String::new() } else { format!(": {}", esc(&site_model::html::swap_words(&c.swaps))) };
    format!("<section><h2>Also works with{also}</h2><p>The same clock with other blocks in some cells, or set another way (the same mechanism and period):</p><ul class=\"swaps\">{rows}</ul></section>\n")
}

/// The properties table rows of a build.
fn properties(site: &Site, b: &Build, prefix: &str, design_text: &str) -> String {
    let mut s = String::new();
    if let Some(d) = &b.door {
        s.push_str(&row("Size (community volume)", &esc(&match (b.volume, &b.dims) {
            (Some(v), Some(dm)) => format!("{v}b ({dm})"),
            (Some(v), None) => format!("{v}b"),
            _ => "not measured".into(),
        })));
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        s.push_str(&row("Category", &esc(&b.group)));
        if let (Some(o), Some(c)) = (d.open, d.close) {
            s.push_str(&row("Open / close", &format!("{o} / {c} gt ({} / {}), worst of 3 cycles after the lever flip", real_time(o), real_time(c))));
        }
        s.push_str(&row("Rotations", &esc(if d.rotation.is_empty() { "unknown" } else { &d.rotation })));
        s.push_str(&row("No slime or honey", yes_no(b.flags.iter().any(|f| f == "slime-free"))));
        s.push_str(&row("Flat (one block thick)", yes_no(b.flags.iter().any(|f| f == "flat"))));
        // (the community regulation's checks are measured on the 3x3 doors only: a 2x2 page says
        // nothing it was not tested for)
        let three = d.side() == 3;
        if three {
            s.push_str(&row("Seamless", &esc(&if d.seamless.is_empty() { "no".to_string() } else { site_model::html::seamless_words(&d.seamless) })));
        }
        if !d.frame.is_empty() {
            s.push_str(&row("Frame when open", &esc(&d.frame)));
        }
        s.push_str(&row("Hallway-compatible", yes_no(b.flags.iter().any(|f| f == "hallway"))));
        if three {
            s.push_str(&row("Record-eligible (community regulation)", yes_no(b.flags.iter().any(|f| f == "eligible"))));
        }
        if !d.boards.is_empty() {
            s.push_str(&row("Boards", &esc(&d.boards.join(", "))));
        }
        if !d.leads.is_empty() {
            s.push_str(&row(if three { "Best door for" } else { "Best 2×2 door for" }, &esc(&d.leads.iter().map(|l| site_model::html::door_lead_words(l, &d.seamless)).collect::<Vec<_>>().join("; "))));
        }
        if let Some(r) = d.record {
            s.push_str(&row("Record progression", &format!("took the solid record at {r} blocks placed")));
        }
        if let Some(v) = &d.buildable {
            s.push_str(&row("Buildable in survival", &esc(v)));
        }
        if d.entities > 0 {
            s.push_str(&row("Entities", &d.entities.to_string()));
        }
        if let Some(t) = &d.track {
            let link = if d.side() == 2 { format!("{REPO_URL}/tree/main/designs/piston-door-2x2/tracks/{}", esc(t)) } else { format!("{REPO_URL}/blob/main/designs/piston-door-3x3/tracks/{}/NOTES.md", esc(t)) };
            s.push_str(&row("Found by", &format!("agent track <a href=\"{link}\">{}</a>", esc(t))));
        }
        if !d.archived.is_empty() {
            s.push_str(&row("Archived", &esc(&d.archived)));
        }
        if let Some(v) = &d.video {
            s.push_str(&row("Video", &format!("<a href=\"{}\">watch (real game)</a>", esc(v))));
        }
        if !d.fill_of.is_empty() {
            s.push_str(&row("Filled from", &format!("<a href=\"{REPO_URL}/blob/main/{}\">{}</a> (<code>door fill</code>: the same mechanism, other blocks in its free cells)", esc(&d.fill_of), esc(d.fill_of.rsplit('/').next().unwrap_or(&d.fill_of)))));
        }
        if let Some(h) = &d.host {
            s.push_str(&row("Another version of", &format!("{} (the same size and speed, shown as one card)", link_build(site, h, prefix))));
        }
        if !d.variants.is_empty() {
            // the versions read alike (the same numbers): numbered, with their block count
            let shown: Vec<String> = d
                .variants
                .iter()
                .take(40)
                .enumerate()
                .map(|(k, v)| match site.builds.iter().find(|o| o.id == *v) {
                    Some(o) => format!("<a href=\"{prefix}{}\">version {} ({})</a>", esc(&o.page), k + 2, site_model::html::blocks_word(o.blocks)),
                    None => link_build(site, v, prefix),
                })
                .collect();
            let more = if d.variants.len() > 40 { format!(" and {} more", d.variants.len() - 40) } else { String::new() };
            s.push_str(&row(&format!("Other versions of this door ({})", d.variants.len()), &format!("{}{more}", shown.join(", "))));
        }
    }
    if let Some(c) = &b.clock {
        s.push_str(&row("Pulses every", &esc(&crate::plain::ticks(&format!("{} gt", c.every())))));
        if c.pulse_every.is_some() {
            s.push_str(&row("Full cycle (its parts repeat)", &esc(&crate::plain::ticks(&format!("{} gt", c.period)))));
        }
        s.push_str(&row("Made of (the blocks that change state)", &esc(&site_model::html::human_blocks(&b.group))));
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} ({v} blocks of space)", esc(&dm.replace('x', "×")))));
        }
        s.push_str(&row(
            "Output",
            &format!(
                "on for {} of every {} ticks{}",
                c.on,
                c.period,
                if c.edges > 1 { format!(", {} pulses per cycle", c.edges) } else { String::new() }
            ),
        ));
        s.push_str(&row("Settles after", &esc(&crate::plain::ticks(&format!("{} gt", c.warmup)))));
        s.push_str(&row("To start it", &esc(&start_words(&c.start, design_text))));
        s.push_str(&row("Works in any direction, anywhere", &esc(&crate::explain::direction_words(&c.robustness))));
        s.push_str(&row("Mirror image", if c.chiral { "a different design (listed separately)" } else { "the same design" }));
        if let Some(card) = &c.card {
            if let Some(cd) = site.cards.iter().find(|x| &x.id == card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}){}", esc(&cd.page), esc(&cd.title), esc(&cd.heading), c.n.map(|n| format!(", n = {n}")).unwrap_or_default())));
            }
        }
        if let Some(h) = &c.host {
            s.push_str(&row("Same mechanism as", &format!("{} (the same period, other blocks in some cells)", link_build(site, h, prefix))));
        }
        if !c.variants.is_empty() {
            // a wall of variants folds to a count (critique C10: 31 links in a row)
            let links: Vec<String> = c.variants.iter().map(|v| link_build(site, v, prefix)).collect();
            let cell = if links.len() <= 6 { links.join(", ") } else { format!("<details><summary>{} versions</summary>{}</details>", links.len(), links.join(", ")) };
            s.push_str(&row("The same mechanism with other blocks", &cell));
        }
        if !c.aliases.is_empty() {
            s.push_str(&row("Also found as", &esc(&c.aliases.join(", "))));
        }
        if c.composite {
            s.push_str(&row("Composite", "yes: sub-clocks side by side (a beat clock)"));
        }
    }
    if let Some(c) = &b.circuit {
        let study = crate::explain::study(&c.study).map(|s| s.0).unwrap_or(c.study.as_str());
        s.push_str(&row("Study", &format!("<a href=\"{prefix}circuits/{}/index.html\">{}</a>", esc(&c.study), esc(study))));
        s.push_str(&row("Kind", &esc(&crate::explain::kind_title(&crate::kinds::title_key(b)))));
        // the pictures and the 3D view add the test parts (critique C41: "3 blocks" next to a
        // picture of five), said only where they do (not on flying machines or extenders)
        let own_device = b.flags.iter().any(|f| f == "lever" || f == "button");
        let mut extra: Vec<&str> = Vec::new();
        if !c.inputs.is_empty() && !own_device {
            extra.push("the input levers or buttons");
        }
        if c.output_lamp {
            extra.push("the test lamp");
        }
        // (the crop and the chests our test adds: fly's QA, "not counting the crop and our test's
        // chests" where the view shows them)
        if c.kind.starts_with("harvester") {
            extra.push("the crop and our test's chests");
        } else if site_model::html::decimal_display(c) {
            // (the 0-9 display: its face's lamps and its input chest are our test's)
            extra.retain(|e| *e != "the test lamp" && *e != "the input levers or buttons");
            extra.push("the face's lamps and the input chest");
        } else if ["smelter", "brewer", "crafter", "item-elevator", "sorter", "composter"].iter().any(|k| c.kind.split_whitespace().next() == Some(*k)) {
            extra.push("our test's chests");
        } else if site_model::html::cart_line(c) {
            // (a whole minecart line: the supply and storage double chests are our test's; its
            // blocks count the track)
            extra.push("the two double chests");
        }
        let count = if extra.is_empty() { b.blocks.to_string() } else { format!("{} (not counting {} the view adds)", b.blocks, extra.join(" and ")) };
        s.push_str(&row("Blocks placed", &count));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} ({v} blocks of space)", esc(&dm.replace('x', "×")))));
        }
        if !c.family.is_empty() {
            s.push_str(&row("Made of (the blocks that change state)", &esc(&site_model::html::human_blocks(&c.family))));
        }
        if !c.robustness.is_empty() {
            s.push_str(&row("Works in any direction, anywhere", &esc(&crate::explain::direction_words(&c.robustness))));
        }
        if !c.inputs.is_empty() || !c.outputs.is_empty() || c.kind == "tnt-duper" || site_model::html::cart_line(c) {
            let ins: Vec<String> = c.inputs.iter().enumerate().map(|(i, n)| format!("{} ({})", n.to_uppercase(), c.devices.get(i).map(String::as_str).unwrap_or("redstone block"))).collect();
            let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
            let text = match (ins.is_empty(), outs.is_empty()) {
                // (the 0-9 display: its one input is a container, its 13 outputs the face's lamps;
                // "A (strength (a chest)) / A, AB, AF, B, BCG..." read as code)
                _ if site_model::html::decimal_display(c) => format!("a chest (or any container) read by a comparator / the {} lamps of its 3×5 face", c.outputs.len()),
                // (a whole minecart line: the chest it empties and the one it fills)
                _ if site_model::html::cart_line(c) => "the supply double chest over the loader / the storage double chest at the unloader".to_string(),
                (false, false) => format!("{} / {}", ins.join(", "), outs.join(", ")),
                (false, true) => format!("{} (no output: it moves blocks)", ins.join(", ")),
                (true, false) if !c.events.is_empty() => format!("changes at its target cell / {}", outs.join(", ")),
                (true, false) => format!("no input / {}", outs.join(", ")),
                (true, true) => "none: it runs by itself".to_string(),
            };
            s.push_str(&row("Inputs / outputs", &esc(&text)));
        }
        if let Some(h) = &c.host {
            s.push_str(&row("Variant of", &format!("{} (fewer blocks, a subset of its block kinds, same kind and value)", link_build(site, h, prefix))));
        }
        if let Some(h) = &c.layout_of {
            s.push_str(&row("Another layout of", &format!("{} (the same kind, value, block count and block kinds)", link_build(site, h, prefix))));
        }
        if !c.same_build.is_empty() {
            let links: Vec<String> = c.same_build.iter().map(|o| link_build(site, o, prefix)).collect();
            s.push_str(&row("The same build is also", &format!("{} (the same blocks and ports, judged under another kind)", links.join(", "))));
        }
        if !c.card.is_empty() {
            if let Some(cd) = site.cards.iter().find(|x| x.id == c.card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}: one design for every n, all generated)", esc(&cd.page), esc(&cd.title), esc(&cd.heading))));
            }
        }
        if !c.archived.is_empty() {
            s.push_str(&row("Archived", &esc(&c.archived)));
        }
    }
    let flags: Vec<String> = b.flags.iter().map(|f| flag_label(f).to_string()).collect();
    if !flags.is_empty() {
        s.push_str(&row("Flags", &esc(&flags.join(", "))));
    }
    // the design's file name: only here, in the collapsed details (vjeux 13:11Z: no ids in lists)
    s.push_str(&row("Design file", &format!("<code>{}</code>", esc(&b.name))));
    s
}

/// A clock's start in words: `kick: last 1,0,0` -> `place the observer at (1, 0, 0) last` (the
/// block read from the design).
fn start_words(start: &str, design_text: &str) -> String {
    let s = start.trim();
    if s.is_empty() || s == "self-starting" || s == "self-start" || s == "self" {
        return "nothing: it starts by itself once built".to_string();
    }
    if let Some(cell) = s.strip_prefix("kick: last ") {
        let p: Vec<i32> = cell.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let block = (p.len() == 3)
            .then(|| mcview::Design::parse(design_text).ok())
            .flatten()
            .and_then(|d| d.blocks.get(&[p[0], p[1], p[2]]).map(|st| st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")));
        return match block {
            Some(b) if !b.is_empty() => format!("place the {b} at ({}) last: it starts ticking when that block goes down", cell.replace(',', ", ")),
            _ => format!("place the block at ({}) last", cell.replace(',', ", ")),
        };
    }
    s.replace("kick", "a kick")
}

/// The door card's table (the coordinator 21:55Z): each board's record, on every record's page.
fn door_table(site: &Site, b: &Build, prefix: &str) -> String {
    // (the records of this page's door size: the 2x2 doors' own, the coordinator 2026-10-21)
    let side = site_model::door_side(b);
    let mut rows: Vec<&Build> = site.builds.iter().filter(|o| o.door.as_ref().is_some_and(|d| !d.leads.is_empty() && d.side() == side)).collect();
    if rows.len() < 2 {
        return String::new();
    }
    // the main record first, then (vjeux 13:11Z, every list) blocks placed, then the box's volume,
    // then speed
    rows.sort_by_key(|o| (o.flags.iter().any(|f| f == "size"), o.blocks, o.volume.unwrap_or(u32::MAX), o.door.as_ref().and_then(|d| d.open.zip(d.close)).map(|(a, c)| a + c).unwrap_or(u32::MAX), o.name.clone()));
    let ku = site_model::html::keep_units;
    let mut body = String::new();
    for o in &rows {
        let d = o.door.as_ref().unwrap();
        // one board per line; a board open to any category names the door's own (critique: "the
        // fastest door" 48 blocks next to "the fastest stone door" 107, nothing said why)
        let cat = match o.group.as_str() {
            "gravity" => "a gravity door (sand or gravel door blocks)",
            "sculk" => "a sculk door (a sculk sensor triggers it)",
            "entity" => "a door that uses entities",
            _ => "",
        };
        let boards: Vec<String> = d
            .leads
            .iter()
            .map(|l| {
                let w = first_upper(&site_model::html::door_lead_words(l, &d.seamless));
                if l.contains("(any category)") && !cat.is_empty() { format!("{w}: {cat}") } else { w }
            })
            .collect();
        let label = boards.iter().map(|w| esc(w)).collect::<Vec<_>>().join("<br>");
        let cur = o.id == b.id;
        let name = if cur { format!("<b>{label}</b><br>(this page)") } else { format!("<a href=\"{prefix}{}\">{label}</a>", esc(&o.page)) };
        let size = match &o.dims {
            Some(dims) => format!("{} blocks in a {} space", o.blocks, dims.replace('x', "×")),
            None => site_model::html::blocks_word(o.blocks),
        };
        let times = match (d.open, d.close) {
            (Some(a), Some(c)) => format!("opens in {}, closes in {}", site_model::gt_time(a), site_model::gt_time(c)),
            _ => String::new(),
        };
        body.push_str(&format!("<tr{}><td>{name}</td><td>{}</td><td>{}</td></tr>\n", if cur { " class=\"on\"" } else { "" }, esc(&ku(&size)), esc(&ku(&times))));
    }
    format!(
        "<section class=\"group kind\" id=\"boards\"><h2>Every record <span class=\"count\">{}</span></h2><p class=\"hint\">The best door for each thing builders ask for: pick one to see it.</p><div class=\"scroll-x\"><table class=\"list kind-sizes\"><thead><tr><th>best at</th><th>size</th><th>speed</th></tr></thead><tbody>\n{body}</tbody></table></div></section>\n",
        rows.len()
    )
}

/// A kind's table (vjeux 21:47Z: one card per kind, every size on its page): one row per size
/// (a flying machine: per direction and load), the best design of that size, its numbers, and
/// the generated families for the sizes in between and beyond.
fn kind_table(site: &Site, b: &Build, members: &[&Build], prefix: &str) -> String {
    let fly = members.iter().any(|o| o.circuit.as_ref().is_some_and(|oc| oc.kind.starts_with("flying-machine")));
    let mut rows: Vec<&Build> = members.to_vec();
    if fly {
        rows.sort_by_key(|o| (crate::kinds::row_of(o).map(|r| r.0).unwrap_or(i64::MAX), o.name.clone()));
    }
    let mut body = String::new();
    for o in &rows {
        let oc = o.circuit.as_ref();
        let label = if fly {
            oc.map(|oc| oc.variant_label.clone()).unwrap_or_default()
        } else {
            oc.map(|oc| site_model::html::value_words(&oc.kind, &oc.axis)).unwrap_or_default()
        };
        let p = site_model::html::props(o);
        let numbers = p.sub.split(" · ").filter(|s| !s.is_empty() && !s.starts_with("made of") && !s.starts_with("fits in") && !s.starts_with("box ")).collect::<Vec<_>>().join(" · ");
        // a super smelter's row leads with the number players compare (the reviewer via the
        // coordinator 2026-10-22): its title without the blocks, which have their own column
        let numbers = match oc.filter(|oc| oc.kind.starts_with("smelter") && site_model::html::double_chest_minutes(oc).is_some()) {
            Some(_) => {
                let lead = p.title.split(" · ").filter(|s| *s != site_model::html::blocks_word(o.blocks)).collect::<Vec<_>>().join(" · ");
                if numbers.is_empty() { lead } else { format!("{lead} · {numbers}") }
            }
            None => numbers,
        };
        let cur = o.id == b.id;
        let ku = site_model::html::keep_units;
        let name = if cur { format!("<b>{}</b> (this page)", esc(&ku(&label))) } else { format!("<a href=\"{prefix}{}\">{}</a>", esc(&o.page), esc(&ku(&label))) };
        body.push_str(&format!("<tr{}><td>{name}</td><td>{}</td><td>{}</td></tr>\n", if cur { " class=\"on\"" } else { "" }, esc(&ku(&site_model::html::blocks_word(o.blocks))), esc(&ku(&numbers))));
    }
    // the generated families of these designs and of the kind's generators: the sizes no
    // archived design has
    let key = crate::kinds::kind_key(b);
    let study = b.circuit.as_ref().map(|c| c.study.clone()).unwrap_or_default();
    let gens: Vec<&Build> = site.builds.iter().filter(|o| o.flags.iter().any(|f| f == "generator") && o.circuit.as_ref().is_some_and(|oc| oc.study == study) && crate::kinds::kind_key(o) == key).collect();
    let mut fams = String::new();
    for o in rows.iter().chain(gens.iter()) {
        let Some(card) = o.circuit.as_ref().map(|oc| oc.card.clone()).filter(|x| !x.is_empty()) else { continue };
        let Some(cd) = site.cards.iter().find(|x| x.id == card) else { continue };
        if fams.contains(&format!("families/{}.html", cd.id)) {
            continue;
        }
        // one line per family card: every size it builds, its block range, what it is made of
        // (the base's own title read "Reach 2 · 15 blocks" next to sizes 2 to 16)
        let lo = cd.families.iter().map(|f| f.period.eval(f.n_lo)).min().unwrap_or(0);
        let hi = cd.families.iter().map(|f| f.period.eval(f.n_hi)).max().unwrap_or(0);
        let lo_b = cd.families.iter().map(|f| f.blocks.eval(f.n_lo)).min().unwrap_or(0);
        let hi_b = cd.families.iter().map(|f| f.blocks.eval(f.n_hi)).max().unwrap_or(0);
        let made = o.circuit.as_ref().map(|oc| oc.family.clone()).filter(|f| !f.is_empty()).map(|f| format!(", made of {}", site_model::html::human_blocks(&f))).unwrap_or_default();
        fams.push_str(&format!("<li><a href=\"{prefix}families/{}.html\">every size from {lo} to {hi}</a> ({lo_b} to {hi_b} blocks{})</li>", esc(&cd.id), esc(&made)));
    }
    let lock = b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("lock"));
    let (head, what) = if fly {
        ("Every direction and load", "direction or load")
    } else if lock {
        // (the combination lock's sizes: 1 to 4 dials, the coordinator 06:56Z)
        ("Every number of dials", "number of dials")
    } else {
        ("Every size", "size")
    };
    let fam_part = if fams.is_empty() { String::new() } else { format!("<p>More sizes, built step by step from these designs (every size ready to download):</p><ul class=\"fams\">{fams}</ul>") };
    format!(
        "<section class=\"group kind\" id=\"sizes\"><h2>{head} <span class=\"count\">{}</span></h2><p class=\"hint\">The best design we have for each {what}: pick one to see it.</p><div class=\"scroll-x\"><table class=\"list kind-sizes\"><thead><tr><th>{what}</th><th>blocks</th><th>numbers</th></tr></thead><tbody>\n{body}</tbody></table></div>{fam_part}</section>\n",
        rows.len()
    )
}

/// A tunable group's picker (every member page and the shown design's): one link per member,
/// its value and block count, the current one marked; following one swaps the 3D view, the
/// numbers and the downloads to that member. A kind's group: its table of sizes instead.
fn group_picker(site: &Site, b: &Build, prefix: &str) -> String {
    let Some(c) = &b.circuit else { return String::new() };
    if c.tunable.is_empty() {
        return String::new();
    }
    let mut members: Vec<&Build> = site
        .builds
        .iter()
        .filter(|o| o.circuit.as_ref().is_some_and(|oc| oc.tunable == c.tunable && oc.host.is_none() && oc.layout_of.is_none()))
        .collect();
    // (by size; within a size, vjeux 13:11Z: blocks placed, then the box's volume)
    members.sort_by_key(|o| (o.circuit.as_ref().and_then(|oc| oc.value).unwrap_or(i64::MAX), o.blocks, o.volume.unwrap_or(u32::MAX), o.name.clone()));
    if members.len() < 2 {
        return String::new();
    }
    if c.tunable.starts_with("kind:") {
        return kind_table(site, b, &members, prefix);
    }
    let span = members.iter().find_map(|o| o.circuit.as_ref().map(|oc| oc.span.clone()).filter(|s| !s.is_empty())).unwrap_or_default();
    let title = {
        let t = c.group_title.trim();
        let mut ch = t.chars();
        match ch.next() {
            Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
            None => String::new(),
        }
    };
    let same_axis = members.iter().all(|m| m.circuit.as_ref().map(|x| &x.axis) == Some(&c.axis));
    // a chip: the member's value in words when the values differ (12 ticks, reach 3), else what
    // sets it apart (stone button, else its box); its second line the block count. No design
    // names (vjeux 13:11Z), no letters (critique P6).
    let chip_text = |o: &Build| -> (String, String) {
        let oc = o.circuit.as_ref();
        let value = oc.map(|oc| site_model::html::value_words(&oc.kind, &oc.axis)).unwrap_or_default();
        let what = oc.map(|oc| oc.variant_label.clone()).unwrap_or_default();
        if same_axis {
            // what sets it apart: its label, else its property line, else its box
            let line = site_model::html::props(o).title;
            let line = line.rsplit_once(" · ").map(|(a, _)| a.to_string()).unwrap_or_default();
            let big = if !what.is_empty() { what } else if !line.is_empty() { line } else { o.dims.as_deref().map(|d| format!("box {}", d.replace('x', "×"))).unwrap_or_default() };
            (big, site_model::html::blocks_word(o.blocks))
        } else {
            (value, site_model::html::blocks_word(o.blocks))
        }
    };
    // two chips that would read the same are numbered (critique: "3 blocks / 3 blocks")
    let mut texts: Vec<(String, String)> = members.iter().map(|o| chip_text(o)).collect();
    let all = texts.clone();
    for (k, t) in texts.iter_mut().enumerate() {
        if t.0.is_empty() || all.iter().filter(|x| **x == *t).count() > 1 {
            t.0 = if t.0.is_empty() { format!("version {}", k + 1) } else { format!("{} · version {}", t.0, k + 1) };
        }
    }
    let chip_text = |o: &Build| -> (String, String) { members.iter().position(|m| m.id == o.id).map(|i| texts[i].clone()).unwrap_or_default() };
    let chips: String = members
        .iter()
        .map(|o| {
            let cur = o.id == b.id;
            let (big, small) = chip_text(o);
            format!(
                "<a class=\"pick{}\" href=\"{prefix}{}\"{}><b>{}</b><span>{}</span></a>",
                if cur { " on" } else { "" },
                esc(&o.page),
                if cur { " aria-current=\"page\"" } else { "" },
                esc(&site_model::html::keep_units(&big)),
                esc(&site_model::html::keep_units(&small))
            )
        })
        .collect();
    let differs = if c.group_differs.is_empty() { "their layout".to_string() } else { c.group_differs.clone() };
    let _ = (&title, &span);
    format!(
        "<section class=\"group\"><h2>Other versions <span class=\"count\">{}</span></h2><p class=\"hint\">The same mechanism; the versions differ only in {}. Pick one: the 3D view (same camera), the numbers and the downloads follow.</p><nav class=\"picker\" aria-label=\"versions\">{chips}</nav></section>\n",
        esc(&if same_axis || span.is_empty() { format!("{} versions", members.len()) } else { site_model::html::span_words(&span) }),
        esc(&differs)
    )
}

/// A clock's timeline (vjeux 13:46Z: "Why is there no judge timeline for clocks?"): its output
/// over its start-up and three periods (two when long), simulated from the design at build time
/// the way the 3D view runs it ([`clock_view`], cached); the start-up shaded, each period marked.
/// When the browser's simulator cannot run the design (a boat, a minecart), it says why.
fn clock_timeline(design_text: &str, c: &site_model::ClockInfo) -> String {
    use crate::simulate::ChartErr;
    let why = match clock_view(design_text, c).chart {
        Ok(ch) => return clock_chart(&ch, c),
        // never silently missing (the coordinator): say why
        Err(ChartErr::Entered) => return "<section><h2>Its output over time</h2><p class=\"hint\">Not charted: blocks move through its output cell, where a lamp or dust would sit, so there is no steady output to draw.</p></section>\n".to_string(),
        Err(ChartErr::NotRun(raw)) => crate::explain::plain_reason(&raw),
        Err(ChartErr::NoPeriod) => "no period measured".to_string(),
        Err(ChartErr::NoBlock) => "no block of it turns on and off in the browser's simulation".to_string(),
        Err(ChartErr::StayedOff(total)) => format!("its output stayed off for the first {total} ticks in the browser's simulation"),
    };
    format!("<section><h2>Its output over time</h2><p class=\"hint\">Not simulated: {}.</p></section>\n", esc(&why))
}

/// A clock's run in the 3D view's session, for its chart and its lamp check (one run, cached by
/// the design's text and the judge's period and warm-up: src/cache.rs).
pub fn clock_view(design_text: &str, c: &site_model::ClockInfo) -> crate::simulate::ClockView {
    crate::cache::get_or(crate::cache::Domain::View, &[design_text.as_bytes(), &c.period.to_le_bytes(), &c.warmup.to_le_bytes()], || crate::simulate::clock_view(design_text, c.period, c.warmup))
}

fn clock_chart(ch: &crate::simulate::Chart, c: &site_model::ClockInfo) -> String {
    let (warm, total) = (ch.warm, ch.ticks);
    let mut phases: Vec<(u32, u32, String)> = Vec::new();
    if warm > 0 {
        phases.push((0, warm, "starting up".to_string()));
    }
    // every cycle labelled the same way (critique: "period 2", "period 3" but no "period 1")
    let mut t = warm;
    let mut k = 1;
    while t + c.period <= total {
        phases.push((t, t + c.period, format!("cycle {k}")));
        t += c.period;
        k += 1;
    }
    let row = site_model::WaveRow { name: "out".into(), input: false, steps: ch.runs.clone(), ..Default::default() };
    let wave = site_model::Wave { ticks: total, rows: vec![row], phases };
    let picked = if ch.picked.is_empty() { String::new() } else { format!(" (the design names no output, so this is its {} that turns on most often)", ch.picked) };
    let on = if c.pulse_every.is_some() {
        format!(": its output pulses every {}, {} times in each cycle", crate::plain::ticks(&format!("{} gt", c.every())), c.edges)
    } else if c.on > 0 && c.edges <= 1 {
        format!(", on for {} of them", crate::plain::ticks(&format!("{} gt", c.on)))
    } else if c.edges > 1 {
        format!(", {} pulses in each", c.edges)
    } else {
        String::new()
    };
    // the grey start-up is said only when there is one (critique: "no grey is drawn"); no brackets
    // inside brackets (critique: "(10 ticks (0.5 s))")
    let start = if warm > 0 { format!("the grey part is the clock starting up, for {}; then ", crate::plain::ticks(&format!("{warm} gt"))) } else { String::new() };
    format!(
        // (as the 3D view plays it: critique, "simulated" right after "tested in the real game")
        "<section><h2>Its output over time</h2><p class=\"hint\">The output (orange) over the first {total} ticks, as the 3D view plays it{picked}: {start}it repeats every {}{on}. 20 ticks = 1 second.</p><div class=\"wave-wrap fit\">{}</div></section>\n",
        crate::plain::ticks(&format!("{} gt", c.period)),
        crate::explain::wave_svg_fit(&wave, 340.0)
    )
}

/// The equivalent layouts of a shown design (same kind, value, blocks and block kinds),
/// collapsed.
fn layouts(site: &Site, b: &Build, prefix: &str) -> String {
    let list: Vec<&Build> = site.builds.iter().filter(|o| o.circuit.as_ref().is_some_and(|c| c.layout_of.as_deref() == Some(b.id.as_str()))).collect();
    if list.is_empty() {
        return String::new();
    }
    let cards: Vec<String> = list.iter().map(|o| build_card(o, prefix)).collect();
    format!(
        "<section><details class=\"layouts\"><summary>Also {} other layout{} (the same kind, value, block count and block kinds)</summary>{}</details></section>\n",
        list.len(),
        if list.len() == 1 { "" } else { "s" },
        grid(&cards)
    )
}

/// The page of an extendable family card: its formula, its two smallest members, its families
/// and every member n with its period, blocks and status.
pub fn card_page(site: &Site, card: &Card, members: &[&Member], scripts: Scripts) -> String {
    let prefix = "../";
    let circuit = card.kind == "circuit";
    let mut body = String::new();
    if circuit {
        let (study_title, _) = crate::explain::study(&card.study).unwrap_or(("Circuits", ""));
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › <a href=\"{prefix}circuits/{}/index.html\">{}</a> › Extendable family</p>\n",
            esc(&card.study),
            esc(study_title)
        ));
    } else {
        // (the families index is gone: vjeux 13:35Z) the Clocks tab
        body.push_str(&format!("<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind=clock\">Clocks</a></p>\n"));
    }
    let base = card.families.first().and_then(|f| site.builds.iter().find(|b| b.id == f.build));
    // a title in words, the range it covers and its step (critique C14 / P10 / C51: the H1 was a
    // formula, "n" and "N" mixed); the formula goes to a details line
    let mut sorted: Vec<&&Member> = members.iter().filter(|m| card.families.first().is_some_and(|f| m.family == f.id)).collect();
    sorted.sort_by_key(|m| m.n);
    let value_of = |m: &Member| -> i64 { if m.axis.is_empty() { m.period as i64 } else { m.axis.split_whitespace().find_map(|w| w.parse::<i64>().ok()).unwrap_or(0) } };
    let steps: Vec<i64> = sorted.windows(2).map(|w| value_of(w[1]) - value_of(w[0])).collect();
    let step = steps.first().copied().filter(|s| *s != 0 && steps.iter().all(|x| x == s));
    let (lo_m, hi_m) = (sorted.first().copied(), sorted.last().copied());
    let kind_of_base = base.and_then(|b| b.circuit.as_ref()).map(|c| c.kind.clone()).unwrap_or_default();
    let what = match base.and_then(|b| b.circuit.as_ref()) {
        Some(c) => site_model::html::kind_words(&c.kind),
        None => format!("Clock made of {}", base.map(|b| site_model::html::human_blocks(&b.group)).unwrap_or_default()),
    };
    let title = match (lo_m, hi_m) {
        // (short, the seconds in the line under it: critique, a huge title on a phone)
        (Some(a), Some(b)) if !circuit => format!("Every {} to {} ticks", a.period, b.period),
        (Some(a), Some(b)) => format!("Every size, from {} to {}", member_value(a, &kind_of_base), member_value(b, &kind_of_base)),
        _ => card.title.clone(),
    };
    body.push_str(&format!("<p class=\"kindline\">{}</p>\n<h1>{}</h1>\n", esc(&what), esc(&site_model::html::keep_units(&title))));
    let mut lede = format!("{} sizes, all ready to download", sorted.len());
    // (a clock family's range in seconds, under its short title)
    if let (Some(a), Some(b), false) = (lo_m, hi_m, circuit) {
        lede = format!("A pulse every {} to {}: {lede}", site_model::real_time(a.period), site_model::real_time(b.period));
    }
    if let Some(s) = step {
        let more = match kind_of_base.split_whitespace().next().unwrap_or("") {
            "" => format!("each size pulses {s} ticks slower"),
            "piston-extender" => format!("each size reaches {s} block{} further", if s == 1 { "" } else { "s" }),
            "piston-bridge" => format!("each size spans {s} block{} more", if s == 1 { "" } else { "s" }),
            "counter" | "divider" => format!("each size counts {s} more"),
            _ if lo_m.is_some_and(|m| m.axis.contains(" gt")) => format!("each size is {s} ticks longer"),
            _ => format!("each size adds {s}"),
        };
        lede.push_str(&format!("; {more}"));
    }
    body.push_str(&format!("<p class=\"oneliner\">{}.</p>\n", esc(&lede)));
    if circuit {
        if let Some(c) = base.and_then(|b| b.circuit.as_ref()) {
            body.push_str(&format!("<p class=\"why\"><b>What it does:</b> {}. Pick the size you need below.</p>\n", esc(&first_upper(&crate::explain::kind_summary(&c.kind)))));
        }
    } else {
        body.push_str("<p class=\"why\"><b>What it does:</b> a clock you can make slower one step at a time: pick the period you need below.</p>\n");
    }
    if let Some(f) = card.families.first() {
        // one live view of the members (a row below swaps its design in place, same camera)
        if let Some(m) = members.iter().find(|m| m.family == f.id && !m.design.is_empty()).or_else(|| members.iter().find(|m| !m.design.is_empty())) {
            let kind = if circuit { "circuit" } else { "clock" };
            body.push_str(&format!(
                "<div class=\"viewer k-{kind}\" id=\"member-view\" data-mcview=\"full\" data-src=\"{prefix}{}\" data-kind=\"{kind}\"{}><div class=\"fallback\"><span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>\n<p id=\"member-caption\" class=\"hint\">{}</p>\n",
                esc(&m.design),
                // (no autoplay: the view starts on play, vjeux 21:29Z)
                "",
                esc(&member_value(m, &kind_of_base)),
                esc(&site_model::html::blocks_word(m.blocks)),
                esc(&member_caption(m, &kind_of_base))
            ));
        }
        // (a design's own label for a part, "hopper A", means nothing in the view: dropped)
        let rule: String = {
            let words: Vec<&str> = f.rule.split(' ').collect();
            let keep: Vec<&str> = words
                .iter()
                .enumerate()
                .filter(|(k, w)| {
                    let letter = w.trim_end_matches([',', '.', ';', ':']);
                    !(letter.len() == 1 && letter.chars().all(|c| c.is_ascii_uppercase()) && *k > 0 && words[k - 1].chars().all(|c| c.is_ascii_lowercase()))
                })
                .map(|(_, w)| *w)
                .collect();
            keep.join(" ")
        };
        body.push_str(&format!("<p><b>Each step adds:</b> {}. Pick a size in the table to see it in the 3D view.</p>\n", esc(&rule)));
    }
    if !card.works_with.is_empty() {
        body.push_str(&format!("<p>Works with any one of these {} blocks in place of the {}: {}.</p>\n", card.works_with.len(), esc(&site_model::html::human_blocks(&card.works_with[0])), esc(&card.works_with.iter().map(|w| site_model::html::human_blocks(w)).collect::<Vec<_>>().join(", "))));
    }
    let value_head = if circuit { "size" } else { "pulses every" };
    if card.families.len() > 1 {
        body.push_str(&format!("<section><h2>The designs in this family</h2><table class=\"list\"><thead><tr><th>design</th><th>{}</th><th>blocks</th><th>sizes</th><th>sizes tested in the game</th></tr></thead><tbody>\n", if circuit { "sizes from" } else { "periods from" }));
        for f in &card.families {
            let extra = if f.extra.is_empty() { String::new() } else { format!(" (+{})", esc(&f.extra.join(", +"))) };
            let fm: Vec<&&Member> = members.iter().filter(|m| m.family == f.id).collect();
            let range = match (fm.iter().min_by_key(|m| m.n), fm.iter().max_by_key(|m| m.n)) {
                (Some(a), Some(b)) => format!("{} to {}", member_value(a, &kind_of_base), member_value(b, &kind_of_base)),
                _ => "-".into(),
            };
            let tested = fm.iter().filter(|m| m.status == site_model::STATUS_JUDGED).count();
            body.push_str(&format!(
                "<tr><td>{}{extra}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                link_build(site, &f.build, prefix),
                esc(&range),
                esc(&f.blocks_text.replace('n', " per step")),
                fm.len(),
                if tested == 0 { "-".to_string() } else { tested.to_string() }
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    let screen = if circuit { "gives exactly its value in simulation" } else { "runs at exactly its period in simulation" };
    body.push_str(&format!("<section><h2>Every size</h2><p>Each size is generated from the smallest design, one step at a time. <span class=\"badge st-judged\">tested in the game</span> = tested in the real game; <span class=\"badge st-redsim\">simulated</span> = it {screen}, not tested in the game; <span class=\"badge st-formula\">formula only</span> = not checked one by one.</p>\n"));
    let row_of = |m: &Member| -> String {
        let anchor = site_model::html::member_anchor(m, site);
        // the value is the row's name (no family id or n: vjeux 13:11Z); it swaps the view in place
        let value_cell = if m.design.is_empty() {
            esc(&member_value(m, &kind_of_base))
        } else {
            // (a clock's row: the period alone, its column says "pulses every")
            let shown = if m.axis.is_empty() { format!("{} ticks ({})", m.period, site_model::real_time(m.period)) } else { member_value(m, &kind_of_base) };
            format!("<a class=\"view\" href=\"#{}\" data-src=\"{prefix}{}\" data-caption=\"{}\">{}</a>", esc(&anchor), esc(&m.design), esc(&member_caption(m, &kind_of_base)), esc(&site_model::html::keep_units(&shown)))
        };
        let download = if m.design.is_empty() {
            "-".to_string()
        } else {
            format!("<a href=\"{prefix}{}\" download>.litematic</a> · <a href=\"{prefix}{}\" download>.txt</a>", esc(&crate::generate::litematic_path(&m.design)), esc(&m.design))
        };
        let page_link = if m.build.is_some() { format!(" · <a href=\"{prefix}{}\">page</a>", esc(&member_href(m, site))) } else { String::new() };
        format!(
            "<tr id=\"{}\"><td>{value_cell}</td><td>{}</td><td>{}</td><td><span class=\"badge st-{}\">{}</span></td><td>{download}{page_link}</td></tr>\n",
            esc(&anchor),
            m.blocks,
            esc(&m.dims.as_deref().unwrap_or("").replace('x', "×")),
            esc(&m.status),
            esc(status_label(&m.status)),
        )
    };
    let head = format!("<table class=\"list\"><thead><tr><th>{value_head}</th><th>blocks</th><th>box</th><th>status</th><th>download</th></tr></thead><tbody>\n");
    // a big family: the tested and simulated sizes and every 10th, the rest folded (critique: 319
    // rows, 30 phone screens, most of them "formula only")
    if members.len() > 40 {
        let in_key = |k: usize, m: &Member| m.status != "formula" || k % 10 == 0 || k + 1 == members.len();
        body.push_str(&format!("<p>The sizes tested in the game or simulated, and every 10th size:</p>{head}"));
        for (k, m) in members.iter().enumerate() {
            if in_key(k, m) {
                body.push_str(&row_of(m));
            }
        }
        body.push_str(&format!("</tbody></table>\n<details><summary>All {} sizes</summary>{head}", members.len()));
        for (k, m) in members.iter().enumerate() {
            if !in_key(k, m) {
                body.push_str(&row_of(m));
            }
        }
        body.push_str("</tbody></table></details></section>\n");
    } else {
        body.push_str(&head);
        for m in members {
            body.push_str(&row_of(m));
        }
        body.push_str("</tbody></table></section>\n");
    }
    // the formula, for the curious (critique P10: "keep them in details")
    let formula = if card.note.is_empty() { card.heading.clone() } else { format!("{}; {}", card.heading, card.note) };
    body.push_str(&format!("<details class=\"more\"><summary>The formula behind the sizes</summary><p><code>{}</code></p><p class=\"hint\">n counts the steps from the smallest size.</p></details>\n", esc(&formula)));
    let desc = format!("{what}: {title}, every size ready to download");
    // the member rows swap the view in place (pkg/redstonepedia.js)
    let scripts = Scripts { app: scripts.ui && scripts.viewer, ..scripts };
    shell_full(&format!("{what}: {title}"), &desc, prefix, &body, scripts, site, None, Some(if circuit { "circuit" } else { "clock" }))
}

/// A family member's value: a clock's period, a circuit's axis value, in words.
fn member_value(m: &Member, kind: &str) -> String {
    if m.axis.is_empty() { format!("every {} ticks ({})", m.period, site_model::real_time(m.period)) } else { site_model::html::value_words(kind, &m.axis) }
}

/// The caption under a family page's view: which member it shows.
fn member_caption(m: &Member, kind: &str) -> String {
    format!("Showing: {} · {} · {}", member_value(m, kind), site_model::html::blocks_word(m.blocks), status_label(&m.status))
}

/// index.html: the filtering UI's shell (the UI itself is pkg/redstonepedia.js).
pub fn index_page(site: &Site, scripts: Scripts) -> String {
    // what the tabs' cards hold (critique 91a98b26c: "1 piston doors, 1 clocks" read as one door
    // and one clock); the app says each tab's own line (critique: the Clocks tab's intro spoke of
    // doors and circuits)
    let lede = site_model::html::lede(site, "");
    let data_v = versions().data;
    let data_src = if data_v.is_empty() { "data/site.json".to_string() } else { format!("data/site.json?v={data_v}") };
    let body = format!(
        "<h1 class=\"sr-only\">{SITE_NAME}</h1><p id=\"lede\" class=\"lede intro\">{}</p>\n\
         <div id=\"app\" class=\"app\"><aside id=\"filters\" class=\"filters\" data-src=\"{data_src}\"><p class=\"loading\">Loading the filters…</p></aside>\
         <div class=\"results\"><div id=\"summary\" class=\"summary\"></div><div id=\"grid\" class=\"grid\"></div><p class=\"more-wrap\"><button id=\"more\" type=\"button\" hidden>Show more</button></p></div></div>\n\
         <noscript><p class=\"notice\">The filters need JavaScript and WebAssembly. Without them, browse <a href=\"circuits/index.html\">the circuits</a>: every design page works without them.</p></noscript>\n",
        esc(&lede)
    );
    shell(SITE_NAME, "Redstone builds you can download: piston doors, clocks and circuits, each tested in the real game, filterable by their properties", "", &body, Scripts { app: true, ..scripts }, site)
}

/// The circuits of one study folder (`pulse`, `tff`, ...), representatives first.
fn study_builds<'a>(site: &'a Site, folder: &str) -> Vec<&'a Build> {
    // (a family's base design kept only as its generator is no card: one card per kind)
    let mut v: Vec<&Build> = site.builds.iter().filter(|b| b.page.starts_with(&format!("circuits/{folder}/")) && !b.flags.iter().any(|f| f == "generator")).collect();
    v.sort_by(|a, b| {
        let key = |x: &Build| {
            let c = x.circuit.as_ref();
            // (gates: the most common function first, critique)
            let value = c.and_then(site_model::html::sort_value).unwrap_or(i64::MAX);
            (c.map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default(), value, x.blocks, x.volume.unwrap_or(0), x.name.clone())
        };
        key(a).cmp(&key(b))
    });
    v
}

/// circuits/<study>/index.html: what the study collects, how designs are ranked, then each kind
/// with its plain explanation and its designs (variants folded under a count).
pub fn study_page(site: &Site, folder: &str, scripts: Scripts) -> String {
    let prefix = "../../";
    let (title, intro) = match crate::explain::study(folder) {
        Some((t, i)) => (t.to_string(), i.to_string()),
        None => (format!("Circuits: {folder}"), String::new()),
    };
    let uses = crate::explain::study_uses(folder);
    let list = study_builds(site, folder);
    // the gates whose designs are being re-tested are named instead of promised (critique: the
    // intro promised AND, XOR... while R7's re-judge held every one of them back)
    let missing = if folder == "gates" {
        let have: Vec<String> = list.iter().filter_map(|b| b.circuit.as_ref()).map(|c| c.kind.split_whitespace().nth(1).unwrap_or("").to_string()).collect();
        let gone: Vec<String> = ["not", "and", "or", "xor", "xnor", "nand", "nor", "imply", "nimply"].iter().filter(|f| !have.iter().any(|h| h == *f)).map(|f| site_model::html::gate_label(f)).collect();
        if gone.is_empty() {
            String::new()
        } else {
            format!(" Not on the site right now: {}. Their designs are being re-tested with real levers and buttons in the game, and come back once they pass.", gone.join(", "))
        }
    } else {
        String::new()
    };
    // what you would use them for first; the designs right after (critique C6 / P5); how we test
    // and rank them folds at the end
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}{}</p>\n<p><a class=\"button alt\" href=\"{prefix}index.html#kind=circuit&amp;q={}\">Filter them in the index</a></p>\n{}",
        esc(&title),
        esc(&title),
        esc(if uses.is_empty() { &intro } else { uses }),
        esc(&missing),
        site_model::encode(folder),
        // (only where the view adds a test lamp: critique, not on flying machines or extenders; a
        // smelter's or a crop farm's output is the judge's chest, which the view shows as a chest)
        {
            let chest_out = |c: &site_model::Circuit| ["smelter", "brewer", "composter", "harvester", "crafter", "item-elevator"].iter().any(|k| c.kind.starts_with(k));
            if !list.is_empty() && list.iter().all(|b| b.circuit.as_ref().is_some_and(site_model::html::decimal_display)) {
                // (the 0-9 display: its face's lamps and its input chest are our test's)
                "<p class=\"hint\">The pictures show the display with the chest and the face's lamps our test adds, settled at the digit 5; the block counts leave those out.</p>\n"
            } else if list.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.output_lamp && !chest_out(c))) {
                "<p class=\"hint\">The pictures show each design with its inputs and a test lamp; the block counts leave those out.</p>\n"
            } else if list.iter().any(|b| b.circuit.as_ref().is_some_and(chest_out)) {
                "<p class=\"hint\">The pictures show each design with the chests our test adds; the block counts leave those out.</p>\n"
            } else {
                ""
            }
        }
    );
    let mut after = String::new();
    if !intro.is_empty() {
        after.push_str(&format!("<details class=\"more\"><summary>How we test and rank them</summary><p>{}</p></details>\n", esc(&intro)));
    }
    // one section per kind family (counter 2..24 together, every gate together)
    let mut kinds: Vec<String> = Vec::new();
    // one section per kind; the basics of a study get their own section at the end (the gates
    // with their own levers: the coordinator's decision (c)) and so do the sand elevators
    let key_of = |b: &Build| -> String {
        let base = b.circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        match b.circuit.as_ref() {
            Some(c) if c.order % 100 >= 50 => format!("{base}:basics"),
            _ => base,
        }
    };
    let mut ordered: Vec<&&Build> = list.iter().collect();
    ordered.sort_by_key(|b| b.circuit.as_ref().map(|c| c.order % 100).unwrap_or(0));
    for b in ordered {
        let k = key_of(b);
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    let mut any_sizes = false;
    let mut cells: Vec<String> = Vec::new();
    for k in &kinds {
        let of: Vec<&&Build> = list.iter().filter(|b| key_of(b) == *k).collect();
        let k = &k.trim_end_matches(":basics").to_string();
        let basics = of.first().and_then(|b| b.circuit.as_ref()).is_some_and(|c| c.order % 100 >= 50);
        let shown = |c: &site_model::Circuit| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty());
        // (a design kept beside its kind's pick, the ground or the no-switch TNT duper: its page,
        // linked from the pick's, no card: the coordinator 14:08Z)
        let reps: Vec<&&Build> = of.iter().copied().filter(|b| b.circuit.as_ref().is_none_or(shown) && !b.flags.iter().any(|f| f == "size")).collect();
        // (a kind with sizes: its card's page has their table; said once per page, after the
        // cards: critique, "Also N other versions..." repeated under every card)
        any_sizes |= of.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.tunable.is_empty() && c.span.is_empty()));
        // the kind's title: what it does first, the community's name in parentheses (critique P1);
        // a study's basics: their own heading
        let (heading, summary) = match (k.as_str(), basics) {
            ("gate", true) => ("Basics: the simplest gates, with their own levers".to_string(), "the classic first circuits: a stone block, a torch, a couple of levers; handy to learn how the logic works".to_string()),
            ("piston-extender", true) => ("Sand elevator: lifts a sand block and lets it fall back (gravity piston extender)".to_string(), "only for sand or gravel, only upward: a sand elevator, a hidden floor".to_string()),
            // (the 0-9 display: its own words, not the 4-input decoder's "0 to F")
            _ if of.iter().all(|b| b.circuit.as_ref().is_some_and(site_model::html::decimal_display)) => {
                let full = of.first().and_then(|b| b.circuit.as_ref()).map(|c| c.kind.clone()).unwrap_or_default();
                (crate::explain::kind_title(&full), crate::explain::kind_summary(&full))
            }
            _ => (crate::explain::kind_title(k), crate::explain::kind_summary(k)),
        };
        // under the kind's heading the cards drop the kind line, and the chips most of them share
        // are said once above them (critique C43)
        let shown: Vec<&Build> = reps.iter().map(|b| **b).collect();
        let shared = site_model::html::common_chips(&shown);
        let all_share = site_model::html::shared_chips(&shown);
        let cards: Vec<String> = reps.iter().map(|b| site_model::html::build_card_opts(b, prefix, false, &shared)).collect();
        let most = if shared.is_empty() {
            String::new()
        } else {
            // in words: "Most have their own lever and are 1 block wide." (critique)
            let parts: Vec<String> = shared.iter().map(|c| site_model::html::chip_plural(c)).collect();
            let list = match parts.len() {
                1 => parts[0].clone(),
                n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
            };
            format!(" <span class=\"hint\">{} {}.</span>", if shared.iter().all(|c| all_share.contains(c)) { "All" } else { "Most" }, esc(&list))
        };
        // a section whose kind is the study itself (the piston extenders): no second heading and
        // intro under the page's own (critique: the heading and intro repeated the page's)
        let study_title = crate::explain::study(folder).map(|s| s.0.to_lowercase()).unwrap_or_default();
        let same_as_study = !basics && study_title.starts_with(&heading.to_lowercase());
        let head = if same_as_study { if most.is_empty() { String::new() } else { format!("<p>{}</p>", most.trim()) } } else { format!("<h2>{}</h2><p>{}.{most}</p>", esc(&heading), esc(&first_upper(&summary))) };
        // a kind with one card (every kind now: vjeux 21:47Z): a cell of the page's grid of
        // kinds, its words above its card (critique, desktop: a card beside a short text left a
        // wide blank column)
        if cards.len() == 1 {
            cells.push(format!("<section class=\"kind-cell\"><h2>{}</h2><p>{}.{most}</p>{}</section>", esc(&heading), esc(&first_upper(&summary)), cards[0]));
        } else {
            if !cells.is_empty() {
                body.push_str(&format!("<div class=\"kind-grid\">{}</div>\n", cells.concat()));
                cells.clear();
            }
            body.push_str(&format!("<section>{head}{}</section>\n", grid(&cards)));
        }
    }
    if !cells.is_empty() {
        body.push_str(&format!("<div class=\"kind-grid\">{}</div>\n", cells.concat()));
    }
    if any_sizes {
        body.push_str("<p class=\"hint\">A card that comes in several sizes lists every size, with the best design of each, on its page.</p>\n");
    }
    if list.is_empty() {
        body.push_str("<p>No design archived yet.</p>\n");
    }
    body.push_str(&after);
    shell_full(&title, if uses.is_empty() { &intro } else { uses }, prefix, &body, scripts, site, None, Some("circuit"))
}

fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// circuits/index.html: a compact list of the studies (critique C7: it was 27 phone screens): one
/// card each, its best design's picture, what you would use it for, its count.
pub fn circuits_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Circuits</p>\n<h1>Circuits</h1>\n<p class=\"lede\">The classic redstone contraptions, one collection each. Every design was tested in the real game; each page says what it does, why you would build it, and lets you try it in 3D.</p>\n"
    );
    let mut cards: Vec<String> = Vec::new();
    let mut pictured: Vec<String> = Vec::new();
    for (folder, title, _) in crate::explain::studies().iter() {
        let list = study_builds(site, folder);
        // the designs the study page shows (critique C1: tiles added up to more than the tab)
        let shown: Vec<&&Build> = list.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty()))).collect();
        // the picture: a design of 3 blocks or more (a 1-block one is mostly the test lamp's
        // outline), not one another tile already shows, by name or as the same build (critique:
        // the Toggle and Memory tiles showed one picture, counter2-bulb is tff-bulb); memory
        // shows a set/reset memory first
        let key = |b: &Build| b.circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, b.name)).unwrap_or_default();
        let used = |b: &Build| pictured.contains(&key(b)) || b.circuit.as_ref().is_some_and(|c| c.same_build.iter().any(|s| pictured.contains(s)));
        let preferred = if *folder == "memory" { shown.iter().find(|b| !used(b) && b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("rs-latch"))) } else { None };
        // (a study's basics, the lever gates and the sand elevator, never picture it)
        let basic = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.order % 100 == 50);
        let Some(first) = preferred
            .or_else(|| shown.iter().find(|b| b.blocks >= 3 && !used(b) && !basic(b)))
            .or_else(|| shown.iter().find(|b| b.blocks >= 3 && !used(b)))
            .or_else(|| shown.iter().find(|b| !used(b)))
            .or_else(|| shown.first())
        else {
            continue;
        };
        pictured.push(key(first));
        cards.push(format!(
            "<a class=\"card study\" href=\"{prefix}circuits/{folder}/index.html\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{}</div></div></a>",
            esc(title),
            // (no fallback words: the tile's title is right under it; critique, the BUD tile's
            // title spilled as giant white words while its view loaded)
            site_model::html::thumb(prefix, site_model::html::card_src(first), first.trace.as_deref(), first.still.as_deref(), first.picture.as_deref(), &first.kind, "", ""),
            esc(title),
            esc(crate::explain::study_blurb(folder))
        ));
    }
    body.push_str(&grid(&cards));
    shell_full("Circuits", "The classic redstone contraptions, tested in the real game: pulse shapers, toggle switches, delays, logic gates, memory, piston extenders, flying machines, shulker box loaders, TNT dupers, block change detectors, timers", prefix, &body, scripts, site, None, Some("circuit"))
}

pub fn not_found_page(site: &Site) -> String {
    // GitHub Pages serves this file at any depth: absolute links to the project's root
    let root = crate::deploy::SITE_PATH;
    let body = format!("<h1>Not found</h1><p>This page does not exist (any more). Start from <a href=\"{root}index.html\">the index</a>.</p>");
    shell("Not found", "Page not found", root, &body, Scripts::default(), site)
}

/// `<folder>/index.html` for the obvious per-kind URLs (`/clocks/`, `/doors/`, `/circuits/`): a
/// redirect to the index filtered on that kind, with a plain link for clients without refresh.
pub fn redirect_page(site: &Site, prefix: &str, title: &str, fragment: &str) -> String {
    let target = format!("{prefix}index.html#{fragment}");
    let body = format!("<h1>{}</h1><p>Opening <a href=\"{}\">{} in the index</a>…</p>", esc(title), esc(&target), esc(title));
    let page = shell(title, &format!("Every {} of REDSTONEPEDIA", title.to_lowercase()), prefix, &body, Scripts::default(), site);
    page.replacen("</head>", &format!("<meta http-equiv=\"refresh\" content=\"0; url={}\">\n</head>", esc(&target)), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use site_model::{ClockInfo, Door};

    #[test]
    fn a_fast_clock_chart_draws_its_pulses() {
        // critique 205ae4e74: a 4-tick clock (on 2 of 4) drew one solid bar: the judge's lamp
        // never went dark between its pulses (the lamp's own 4-tick off-delay); the chart reads the
        // power into the output's cell, as the judge does
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-obs-p4-obs-4b.txt")).unwrap();
        let c = ClockInfo { period: 4, warmup: 5, on: 2, edges: 1, ..Default::default() };
        let html = clock_timeline(&text, &c);
        let points = html.split("class=\"wout\" points=\"").nth(1).and_then(|s| s.split('"').next()).unwrap_or_else(|| panic!("a chart: {html}"));
        let ys: Vec<f64> = points.split_whitespace().filter_map(|p| p.split(',').nth(1)?.parse().ok()).collect();
        let rises = ys.windows(2).filter(|w| w[1] < w[0]).count();
        assert!(rises >= 3, "one rise per cycle, 3 cycles: {rises} in {points}");
        // a lamp on it stays lit (critique a417977c3): off 2 gt at a time, a lamp needs more than 4
        assert_eq!(clock_view(&text, &c).lamp, Some(crate::simulate::Lamp::Fast("stays lit".into(), 2)), "the 4-tick clock keeps a lamp lit");
    }

    pub fn tiny_site() -> Site {
        let clock = |id: &str, p: u32, blocks: u32| Build {
            id: format!("clocks/{id}"),
            kind: "clock".into(),
            group: "observer".into(),
            name: id.into(),
            page: format!("clocks/{id}.html"),
            design: format!("designs/clocks/{id}.txt"),
            blocks,
            volume: Some(blocks),
            dims: Some("2x1x1".into()),
            kinds: vec!["observer".into()],
            caption: "Two observers face to face <3".into(),
            flags: vec!["robust".into()],
            clock: Some(ClockInfo { period: p, on: 2, edges: 1, ..Default::default() }),
            ..Default::default()
        };
        let door = Build {
            id: "doors/fc2-40b".into(),
            kind: "door".into(),
            group: "solid".into(),
            name: "fc2-40b".into(),
            page: "doors/fc2-40b.html".into(),
            design: "designs/doors/fc2-40b.txt".into(),
            blocks: 40,
            volume: Some(144),
            dims: Some("8x9x2".into()),
            kinds: vec!["observer".into(), "sticky_piston".into()],
            flags: vec!["rotation-safe".into()],
            door: Some(Door { open: Some(14), close: Some(16), rotation: "safe".into(), seamless: "FULL".into(), frame: "12/12".into(), props: vec![("volume".into(), "144".into())], ..Default::default() }),
            ..Default::default()
        };
        Site { version: 1, generated: "2026-10-19T23:00Z".into(), commit: "0123456789abcdef".into(), builds: vec![clock("obs2", 6, 2), clock("obs3", 6, 3), door], ..Default::default() }
    }

    #[test]
    fn build_page_has_the_parts() {
        let s = tiny_site();
        let p = build_page(&s, &s.builds[0], "name: obs2\n", "designs/clocks/archive/obs2.txt", Scripts { viewer: true, app: false, ui: false });
        assert!(p.starts_with("<!DOCTYPE html>"));
        // the kind and the properties lead; the design's name only in the collapsed details
        assert!(p.contains("<title>Clock: every 6 ticks (0.3 s) · 2 blocks · REDSTONEPEDIA</title>"), "{p}");
        let details = p.find("<details class=\"more\">").expect("the details");
        assert!(!p[..details].contains("obs2\n") && !p[..details].contains(">obs2<"), "no design name before the details");
        assert!(p[details..].contains("<code>obs2</code>"), "{p}");
        assert!(p.contains("<p class=\"oneliner\">Its output pulses by itself every 6 ticks (0.3 s), forever."), "{p}");
        assert!(p.contains("data-mcview=\"full\" data-src=\"../designs/clocks/obs2.txt\" data-kind=\"clock\""), "{p}");
        assert!(p.contains("<html lang=\"en\" data-mcview-textures=\"../textures/26.3\">"), "the viewer fetches the vendored textures from the site: {p}");
        assert!(!p.contains("data-autoplay"), "nothing plays at once (vjeux 21:29Z): {p}");
        // ... and no text promises it does (the door page's Try it said "the view flips the
        // lever for you" while the view sat paused: e78k-h5-41b)
        assert!(!p.contains("starts it for you") && !p.contains("the view plays it"), "the clock's Try it must say press play: {p}");
        assert!(p.contains("Press play under the view: it runs by itself"), "{p}");
        assert!(p.contains("Two observers face to face &lt;3"));
        assert!(p.contains("index.html#kind=clock&amp;time=6\">on the Clocks tab</a>"), "the other clocks of its period: on the Clocks tab: {p}");
        assert!(p.contains("import init from \"../viewer/mcview.js?v="), "{p}");
        assert!(p.contains("init({module_or_path: \"../viewer/mcview_bg.wasm?v="), "the wasm by its own versioned URL");
        assert!(!p.contains("pkg/redstonepedia.js"));
        let d = build_page(&s, &s.builds[2], "name: fc2\n", "designs/piston-door-3x3/archive/fc2-40b.txt", Scripts::default());
        assert!(d.contains("<h1>40\u{a0}blocks in a 8×9×2\u{a0}space</h1>"), "{d}");
        assert!(d.contains("14 / 16 gt"));
        assert!(!d.contains("flips the lever for you"), "the door's Try it must say press play: {d}");
        assert!(d.contains("Press play under the view: the door opens, stays open a moment and closes"), "{d}");
        assert!(!d.contains("mcview.js"), "no viewer script without the bundle");
    }

    #[test]
    fn index_loads_the_app() {
        let s = tiny_site();
        let p = index_page(&s, Scripts { viewer: true, app: false, ui: false });
        assert!(p.contains("import init from \"./pkg/redstonepedia.js?v="), "{p}");
        assert!(p.contains("id=\"grid\""));
        // (two clocks pulsing every 6 ticks: one period)
        assert!(p.contains("Redstone builds you can download: the best 3×3 piston door and every clock that works (2 clocks for 1 period). Every base design was tested"), "{p}");
    }
}
