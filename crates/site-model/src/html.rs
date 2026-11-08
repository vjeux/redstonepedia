//! HTML fragments shared by the static pages (crates/site) and the browser UI (web/site): the
//! result cards are built by the same code on both sides, so a card looks the same in a build
//! page's "similar builds" and in the filter grid.

use crate::{Build, Hit, Member, Site, gt_time, status_label};

/// Text escaped for HTML content and double-quoted attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The kind's display name, plural.
pub fn kind_title(kind: &str) -> &'static str {
    match kind {
        "door" => "Doors",
        "clock" => "Clocks",
        "circuit" => "Circuits",
        _ => "Builds",
    }
}

/// The size line of a build: `63b (7x9x1) · 43 blocks` (doors: the community's volume first).
pub fn size_line(b: &Build) -> String {
    match (b.kind.as_str(), b.volume, b.dims.as_deref()) {
        ("door", Some(v), Some(d)) => format!("{v}b ({d}) · {}", blocks_word(b.blocks)),
        ("door", Some(v), None) => format!("{v}b · {}", blocks_word(b.blocks)),
        _ => blocks_word(b.blocks),
    }
}

/// The headline number of a build (the fallback card's big text): a door's volume, a clock's
/// period, a circuit's axis value.
pub fn headline(b: &Build) -> String {
    // the big text over a picture or a loading view: in words, like the card's title (critique:
    // "N = 2 gt" badges over thumbnails whose card says "2-tick delay")
    if let Some(c) = &b.clock {
        return format!("every {} ticks", c.every());
    }
    if let Some(c) = &b.circuit {
        let (main, _) = circuit_props(c);
        return if main.is_empty() { kind_words(&c.kind) } else { main.split(" · ").next().unwrap_or(&main).to_string() };
    }
    match &b.dims {
        Some(d) => d.replace('x', "×"),
        None => blocks_word(b.blocks),
    }
}

/// The card's second line.
pub fn subline(b: &Build) -> String {
    match b.kind.as_str() {
        "door" => {
            let mut s = format!("{} · {}", size_line(b), b.group);
            if let Some(d) = &b.door {
                if let (Some(o), Some(c)) = (d.open, d.close) {
                    s.push_str(&format!(" · {o}/{c} gt"));
                }
            }
            s
        }
        "clock" => {
            let p = b.clock.as_ref().map(|c| gt_time(c.every())).unwrap_or_default();
            format!("{p} · {} · {}", blocks_word(b.blocks), b.group.replace('+', " + "))
        }
        _ => match &b.circuit {
            // `rising-edge · L = 2 gt · 3 blocks`, `flying-machine · up · 10 gt/block · 6 blocks`:
            // the axis value whenever the kind does not already say it
            Some(c) => {
                let mut parts = vec![b.group.clone()];
                if !c.direction.is_empty() {
                    parts.push(c.direction.clone());
                }
                if !c.axis.is_empty() && c.axis != c.kind && !c.kind.ends_with(c.axis.as_str()) && !c.axis.starts_with("detects") {
                    if let Some(a) = axis_shown(&c.kind, &c.axis) {
                        parts.push(a);
                    }
                }
                // a machine's speed: a shulker loader's items per second
                if let Some((_, v)) = c.metrics.iter().find(|(n, _)| n == "items/s") {
                    parts.push(format!("{v} items/s"));
                }
                parts.push(blocks_word(b.blocks));
                parts.join(" · ")
            }
            None => format!("{} · {}", b.group, blocks_word(b.blocks)),
        },
    }
}

/// `1 block`, `3 blocks`.
/// The axis as a card shows it: `L = 12 gt` of `off L = 12 gt` or `L = 12 gt retriggerable` (the
/// kind already says on / off / retriggerable), nothing when the kind already carries the number
/// (`divider 3` with `N = 3`).
pub fn axis_shown(kind: &str, axis: &str) -> Option<String> {
    // (a lock's size in words: "1 dial", "3 dials"; the archive writes "1 dials"; a display
    // decoder's: its input lines)
    if kind.starts_with("lock") || kind.starts_with("seven-segment") {
        return Some(value_words(kind, axis));
    }
    let Some((head, value)) = axis.split_once(" = ") else {
        return Some(axis.to_string());
    };
    let letter = head.split_whitespace().last().unwrap_or(head);
    let mut v = value.split_whitespace();
    let n = v.next().unwrap_or("");
    let unit = match v.next() {
        Some("gt") => " gt",
        _ => "",
    };
    if kind.split_whitespace().nth(1) == Some(n) {
        return None;
    }
    Some(format!("{letter} = {n}{unit}"))
}

pub fn blocks_word(n: u32) -> String {
    // (thousands grouped: the coordinator's cards, "1,234 blocks")
    if n == 1 { "1 block".to_string() } else { format!("{} blocks", group_thousands(n as u64)) }
}

/// Badges shown on a card (short).
pub fn badges(b: &Build) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(d) = &b.door {
        if !d.leads.is_empty() {
            v.push(door_lead_words(&d.leads[0], &d.seamless));
        }
        if !d.seamless.is_empty() {
            v.push(seamless_words(&d.seamless));
        }
    }
    // a longer version of another clock says so first (vjeux 01:24Z: "extended n = k"; in words,
    // 2026-11-08: no "n")
    if b.flags.iter().any(|x| x == "extended") && b.clock.as_ref().and_then(|c| c.n).is_some() {
        v.push("a longer version".to_string());
    }
    for (f, words) in [("slime-free", "no slime"), ("flat", "1 block thick"), ("eligible", "record-eligible"), ("extendable", "comes in every size"), ("self-starting", "starts by itself"), ("variant", "a bigger version of another design"), ("composite", "several clocks in one"), ("layout", "another layout"), ("flip-safe", "safe to flip back any time")] {
        if b.flags.iter().any(|x| x == f) {
            v.push(words.to_string());
        }
    }
    // its own input device (a TNT duper's lever is its on/off switch: the coordinator 14:08Z; a
    // one-way flying machine's starts it, 16:10Z; a docking machine's title says its own)
    if b.flags.iter().any(|x| x == "lever") {
        let c = b.circuit.as_ref();
        let kind = c.map(|c| c.kind.as_str()).unwrap_or("");
        let two_way = c.is_some_and(|c| c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way"));
        if kind.starts_with("tnt-duper") && kind != "tnt-duper-triggered" {
            v.push("on/off lever".into());
        } else if kind.starts_with("flying-machine") && !two_way {
            v.push("start lever".into());
        } else if !kind.starts_with("flying-machine") {
            v.push("has its own lever".into());
        }
    } else if b.flags.iter().any(|x| x == "button") {
        v.push("has its own button".into());
    }
    // a tunable group's shown design: the group's span (`13 versions, 12 to 36 ticks`)
    if let Some(c) = &b.circuit {
        if !c.span.is_empty() {
            v.push(span_words(&c.span));
        }
        // the machines' variations: a shulker loader's badges, what a BUD switch detects
        let m = |k: &str| c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let badges: Vec<&str> = m("badges").split('+').filter(|s| !s.is_empty()).collect();
        for x in badges.iter().take(4) {
            v.push(badge_words(x));
        }
        if badges.len() > 4 {
            v.push(format!("+{} more", badges.len() - 4));
        }
        // (what a BUD switch notices is its headline)
    }
    v
}

/// A thumbnail's `data-still` at tick 0: the still search's `out=` and `box=` kept, its tick
/// dropped (`5428 out=1,0,2 box=0,0,-16,9,1,0` -> `0 out=1,0,2 box=0,0,-16,9,1,0`; none -> `0`).
pub fn still_at_zero(still: Option<&str>) -> String {
    let rest: Vec<&str> = still.unwrap_or("").split_whitespace().skip(1).collect();
    std::iter::once("0").chain(rest).collect::<Vec<_>>().join(" ")
}

/// The design file a build's card loads (from the site root): its own card still
/// (`Circuit::card_src`, a crop farm's field grown), else its view's (`Circuit::view_src`, a
/// display's lit still, a minecart line's chests), else the design.
pub fn card_src(b: &Build) -> &str {
    b.circuit
        .as_ref()
        .map(|c| if c.card_src.is_empty() { c.view_src.as_str() } else { c.card_src.as_str() })
        .filter(|s| !s.is_empty())
        .unwrap_or(&b.design)
}

pub fn thumb(prefix: &str, design: &str, trace: Option<&str>, still: Option<&str>, picture: Option<&str>, kind: &str, big: &str, small: &str) -> String {
    let trace = trace.map(|t| format!(" data-trace=\"{}\"", esc(&format!("{prefix}{t}")))).unwrap_or_default();
    // stills only on the grid (vjeux 21:29Z, "Why is it simulating everything?"): the build as
    // placed, tick 0, framed by the box and the output the build's still search found
    // (webrender's data-still: `go_to_still_from` runs no tick for 0); the viewer runs nothing,
    // not even its own still search (an absent data-still made it simulate up to 20000 ticks)
    let still = format!(" data-still=\"{}\"", esc(&still_at_zero(still)));
    // the interim picture (until the 3D viewer renders): lazy, under the headline label
    let (pic, cls) = match picture {
        Some(p) => (format!("<img class=\"pic\" src=\"{}\" alt=\"\" loading=\"lazy\" decoding=\"async\">", esc(p)), " has-pic"),
        None => (String::new(), ""),
    };
    // no design file (a member listed from its formula only): no 3D view
    let view = if design.is_empty() { String::new() } else { format!(" data-mcview=\"thumb\" data-src=\"{}\"{trace}{still} data-kind=\"{kind}\"", esc(&format!("{prefix}{design}"))) };
    format!(
        "<div class=\"thumb k-{kind}\"{view}><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>",
        esc(big),
        esc(small)
    )
}

/// The fallback card's small line (under the headline): a door's box and category, a clock's
/// family.
pub fn fallback_line(b: &Build) -> String {
    match (b.kind.as_str(), &b.dims) {
        ("door", Some(d)) => format!("{} · {}", d.replace('x', "×"), door_category(&b.group)),
        ("clock", _) => format!("made of {}", human_blocks(&b.group)),
        _ => b.group.clone(),
    }
}

/// A circuit kind's base in words, singular (vjeux 12:15Z: "Circuit kind should be piston
/// extender, not one per N"), in everyday words (13:06Z: "I have no clue what a monostable
/// retriggerable system means"): `piston-extender 5` -> `Piston extender`, `monostable
/// retriggerable` -> `One-shot timer`, `bud` -> `Block change detector`. The number is a
/// property, not a kind; the jargon name is in the kind's title (crates/site explain::kind_title).
pub fn kind_words(kind: &str) -> String {
    let base = kind.split_whitespace().next().unwrap_or("");
    let words = match base {
        "rising-edge" => "One pulse when switched on",
        "falling-edge" => "One pulse when switched off",
        "dual-edge" => "One pulse on every switch",
        "pulse-limiter" => "Signal shortener",
        "pulse-extender" => "Signal stretcher",
        "tff" => "Toggle switch",
        "tff-reset" => "Toggle switch with reset",
        "delay" => "Delay",
        "gate" => "Logic gate",
        "rs-latch" => "On/off memory",
        "d-latch" => "Copy-and-hold memory",
        "dff" => "Clocked memory",
        "dff-enable" => "Clocked memory with enable",
        "shift" => "Shift register",
        "counter" => "Counter",
        "adder" => "Adder",
        "binary-counter" => "Binary counter",
        "decoder" => "Decoder",
        "mux" => "Multiplexer",
        "comparator" => "Number comparator",
        "randomizer" => "Randomizer",
        "harvester" => "Crop farm",
        "item-elevator" => "Item elevator",
        "elevator" => "Player elevator",
        "station" => "Minecart station",
        "cart-unloader" => "Minecart unloader",
        "cart-loader" => "Minecart loader",
        "cart-line" => "Minecart line",
        "auto-door" => "Automatic door",
        "crafter" => "Auto-crafter",
        "seven-segment" => "7-segment display decoder",
        "piston-extender" => "Piston extender",
        "piston-bridge" => "Piston bridge",
        "flying-machine" => "Flying machine",
        "shulker-loader" => "Shulker box loader",
        "shulker-unloader" => "Shulker box unloader",
        "bud" => "Block change detector",
        "tnt-duper" => "TNT duper",
        "tnt-duper-triggered" => "TNT duper, one at a time",
        "burst" => "Pulses per press",
        "divider" => "Clock slower",
        "monostable" => "One-shot timer",
        "off-delay" => "Stay-on timer",
        "on-delay" => "Hold-to-activate",
        "pulse-train" => "Rapid fire while on",
        "sorter" => "Item sorter",
        "lock" => "Combination lock",
        "sequence-lock" => "Button sequence lock",
        "smelter" => "Auto-smelter",
        "brewer" => "Auto-brewer",
        "composter" => "Auto-composter",
        // (an iron farm: its product names it; a mob farm of another product: its base)
        "mob-farm" if kind.split_whitespace().nth(1).is_none_or(|i| i == "iron_ingot") => "Iron farm",
        _ => "",
    };
    if !words.is_empty() {
        return words.to_string();
    }
    // a kind this list does not know yet: its base, dashes as spaces, capitalized
    let s = base.replace('-', " ");
    let mut ch = s.chars();
    ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
}

/// A circuit's value in words, from its axis (`L = 12 gt` -> `12 ticks`; a piston extender's
/// `N = 3` -> `reach 3`; a counter's -> `every 3rd`): a picker chip, a member's value.
pub fn value_words(kind: &str, axis: &str) -> String {
    let v = axis.split_once(" = ").map(|(_, v)| v.trim()).unwrap_or(axis.trim());
    // `12 gt` or `12 gt retriggerable` (the kind's qualifier is the same for every version)
    let w: Vec<&str> = v.split_whitespace().collect();
    if w.len() >= 2 && w[1] == "gt" {
        return tk(&format!("{} gt", w[0]));
    }
    // the value's number: the first numeric word (`counter 2` -> `2`; `counter 11, one pulse per
    // 22 inputs` -> `11`, its comma off: the counters' table read "every 0th"); a randomizer's axis
    // is `1 of N`: its N is the last
    let n = v.split_whitespace().find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit())).map(|t| t.trim_end_matches(|c: char| !c.is_ascii_digit())).unwrap_or(v);
    match kind.split_whitespace().next().unwrap_or("") {
        "randomizer" => {
            let n = v.split_whitespace().filter(|t| t.chars().all(|c| c.is_ascii_digit())).last().unwrap_or(n);
            format!("1 of {n} outputs")
        }
        "item-elevator" => format!("{n} blocks up"),
        "elevator" => format!("{n} blocks up"),
        "piston-extender" => format!("reach {n}"),
        "piston-bridge" => format!("a {n}-block gap"),
        "counter" | "divider" => format!("every {}", nth(n)),
        "burst" => format!("{n} pulses"),
        "shift" | "adder" | "binary-counter" => format!("{n} bits"),
        // (w86's computational circuits: a decoder of N inputs lights one of 2^N outputs, a
        // multiplexer picks one of 2^N inputs, a comparator compares two N-bit numbers)
        "decoder" => n.parse::<u32>().map(|k| format!("{k} inputs to {} outputs", 1u32 << k.min(16))).unwrap_or_else(|_| v.to_string()),
        "mux" => n.parse::<u32>().map(|k| format!("{} inputs to 1", 1u32 << k.min(16))).unwrap_or_else(|_| v.to_string()),
        "comparator" => format!("{n}-bit numbers"),
        "delay" => tk(&format!("{n} gt")),
        "lock" => format!("{n} dial{}", if n == "1" { "" } else { "s" }),
        // (a display decoder's number is its input lines: "4 · 566 blocks" said nothing; the 0-9
        // display, one strength input: its one digit)
        "seven-segment" if kind.split_whitespace().any(|w| w == "decimal") => "one digit, 0 to 9".to_string(),
        "seven-segment" => format!("{n} inputs, digits 0 to F"),
        _ => v.to_string(),
    }
}

/// clk-obs's readable 0-9 display (`seven-segment 4 hex decimal`): one comparator reads a
/// container, a 3×5 lamp face with lit corners shows the digit (designs/briefs/display-7seg.md).
pub fn decimal_display(c: &crate::Circuit) -> bool {
    c.kind.starts_with("seven-segment") && c.kind.split_whitespace().any(|w| w == "decimal")
}

/// An automatic door's `stop in doorway` (clk-obs's judge, 2026-10-22: reported, not required):
/// Some(true) when the door stays open for someone standing in the doorway, Some(false) when it
/// closes on them, None for a verdict without it.
pub fn door_stays_open(c: &crate::Circuit) -> Option<bool> {
    if !c.kind.starts_with("auto-door") {
        return None;
    }
    match cm(c, "stop in doorway").trim() {
        "the door stays open" => Some(true),
        "the door closes on them" => Some(false),
        _ => None,
    }
}

/// An automatic door's open wait, in game ticks (the longest a walker stood at the closed door).
pub fn door_open_wait(c: &crate::Circuit) -> Option<u32> {
    if !c.kind.starts_with("auto-door") {
        return None;
    }
    first_ticks(cm(c, "open wait"))
}

/// An automatic door's wait in a player's words: `no waiting, even sprinting` (0 gt: it opens
/// before a sprinter reaches it), else `you wait up to 0.65 s at the door`.
pub fn door_wait_words(c: &crate::Circuit) -> Option<String> {
    door_open_wait(c).map(|w| if w == 0 { "no waiting, even sprinting".to_string() } else { format!("you wait up to {} at the door", crate::real_time(w)) })
}

/// A whole minecart line (the site's `cart-line`: sorter's crate::cartsys designs, a loader, the
/// track and an unloader in one design).
pub fn cart_line(c: &crate::Circuit) -> bool {
    c.kind == "cart-line"
}

/// A minecart line whose trickle run (a farm's slow output, then nothing) ended with the cart
/// parked at the loader: with nothing to carry, the cart waits there.
pub fn cart_waits(c: &crate::Circuit) -> bool {
    cart_line(c) && cm(c, "trickle").contains("parked at the loader")
}

/// A metric's first number of game ticks (`32 gt (a goes 0 -> 2 from a=0)` -> 32).
fn first_ticks(v: &str) -> Option<u32> {
    v.split_whitespace().next()?.parse().ok()
}

/// A machine's badge in words (`tileable(9)` -> `tiles every 9 blocks`, `tileable(1)` -> `tiles
/// side by side`, `top-up` -> `tops up part-full boxes`): short, for chips and card headlines
/// (critique: "restart", "top-up", "drop-free" need the variations table's words).
pub fn badge_words(badge: &str) -> String {
    match badge.split_once('(') {
        Some(("tileable", n)) => match n.trim_end_matches(')') {
            "1" => "tiles side by side".to_string(),
            n => format!("tiles every {n} blocks"),
        },
        _ => match badge {
            "16-stack" => "16-stack items too",
            "unstackable" => "unstackable items too",
            "mixed" => "packs mixed items",
            "filter" => "packs one kind, passes the rest",
            "tileable" => "tiles side by side",
            "always-on" => "always on, no lever",
            "restart" => "restarts by itself",
            "box-starved" => "waits out a box shortage",
            "colours" => "keeps box colours and names",
            "flush" => "sends out a part-full box",
            "top-up" => "tops up part-full boxes",
            "drop-free" => "drops nothing on the ground",
            "combo" => "loads and unloads",
            "any-box" => "empties any box",
            // (an item sorter's: a full chest blocks nothing)
            "overflow" | "overflow-proof" => "a full chest never jams the line",
            "stream-direct" => "a water stream can end right on it",
            other => other,
        }
        .to_string(),
    }
}

/// A tunable group's range as a card's headline (`every 17th to 576th pulse`, `on for 12 to 36
/// ticks · restarts on every press`, `reach 2 to 4`), from its span (`L 12-36 gt, 5 values`);
/// None for a design that is no group's card or whose span is no range.
/// A kind card's span of sizes (`c.span`, `1-16 furnaces, 3 values`): its lowest and highest.
fn span_bounds(span: &str) -> Option<(u32, u32)> {
    let (range, _) = span.split_once(", ")?;
    range.split_whitespace().find_map(|w| {
        let (a, b) = w.split_once('-')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    })
}

fn group_range(c: &crate::Circuit) -> Option<String> {
    // the flying machine card (the coordinator 21:55Z: one card, a table of direction and load)
    if c.kind.starts_with("flying-machine") && c.tunable.starts_with("kind:") && !c.span.is_empty() {
        return Some("flies sideways, up or down, with cargo or trailers".into());
    }
    // one machine timed with several kinds of items (critique: three cards for 1-, 16- and
    // 64-stack items): one card that works with any of them
    if c.kind.starts_with("shulker-") && !c.span.is_empty() {
        // a card that leads with what it adds keeps its own title (critique: "works with any
        // stack size" stopped saying anything once a smaller loader did it too)
        if !c.lead.is_empty() {
            return None;
        }
        // (with this card's own rate: critique, "card 1 has no items/s figure")
        let rate = c.metrics.iter().find(|(k, _)| k == "items/s").map(|(_, v)| format!(" · {v} items a second")).unwrap_or_default();
        return Some(format!("works with any stack size{rate}"));
    }
    // a super smelter's card leads with its pictured size's double chest (the coordinator
    // 2026-10-22): its own title; the sizes its page lists close its second line (circuit_props)
    if c.kind.starts_with("smelter") && double_chest_minutes(c).is_some() {
        return None;
    }
    let (lo, hi) = span_bounds(&c.span)?;
    let mut w = c.kind.split_whitespace();
    let (k0, k1) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    // (with seconds: critique, "on for 12 to 36 ticks" had none)
    let ticks = format!("{lo} to {hi} ticks ({} to {})", crate::real_time(lo), crate::real_time(hi));
    Some(match k0 {
        "counter" | "divider" => format!("every {} to {} pulse", nth(&lo.to_string()), nth(&hi.to_string())),
        "piston-extender" => format!("reach {lo} to {hi}"),
        "piston-bridge" => format!("gaps of {lo} to {hi} blocks"),
        "burst" => format!("{lo} to {hi} pulses per press"),
        "shift" | "adder" | "binary-counter" | "comparator" => format!("{lo} to {hi} bits"),
        "decoder" | "mux" => format!("{lo} to {hi} select bits"),
        "randomizer" => format!("1 of {lo} to {hi} outputs"),
        "item-elevator" | "elevator" => format!("{lo} to {hi} blocks up"),
        "rising-edge" | "falling-edge" | "dual-edge" => format!("{lo}- to {hi}-tick pulse"),
        "pulse-limiter" => format!("cuts pulses to {lo}–{hi} ticks, by version"),
        "pulse-extender" => format!("stretches pulses to {lo}–{hi} ticks, by version"),
        "delay" => format!("{lo}- to {hi}-tick delay"),
        "monostable" => format!("on for {ticks}{}", if k1 == "retriggerable" { " · restarts on every press" } else if k1 == "non-retriggerable" { " · ignores presses while on" } else { "" }),
        "off-delay" => format!("stays on {ticks} after"),
        "on-delay" => format!("must be held {ticks}"),
        "pulse-train" => format!("a pulse every {ticks}"),
        _ => return None,
    })
}

/// A tunable group's span in words: `L 12-36 gt, 13 values` -> `13 versions, 12 to 36 ticks`.
pub fn span_words(span: &str) -> String {
    let Some((range, count)) = span.split_once(", ") else {
        // not a range of values (`2 versions`): as it is
        return span.to_string();
    };
    let n = count.split_whitespace().next().unwrap_or("");
    // `L 12-36 gt` (a letter first) or `17-576` (no letter: a counter's count)
    let words: Vec<&str> = range.split_whitespace().collect();
    let start = if words.first().is_some_and(|w| w.starts_with(|c: char| c.is_ascii_digit())) { 0 } else { 1 };
    let r = words.get(start).copied().unwrap_or("");
    // (every word after the range: `gt`, an item elevator's `blocks up`)
    let unit = words.get(start + 1..).map(|w| w.join(" ")).unwrap_or_default();
    let r = match r.split_once('-') {
        Some((a, b)) => format!("{a} to {b}"),
        None => r.to_string(),
    };
    let unit = match unit.as_str() {
        "gt" => " ticks".to_string(),
        "" => String::new(),
        other => format!(" {other}"),
    };
    if n.is_empty() { format!("versions {r}{unit}") } else { format!("{n} versions, {r}{unit}") }
}

/// The searches to offer when a text finds nothing (critique C12: "an empty search suggests the
/// nearest kinds instead of 0 results"): the kinds whose words share a word (or its first 4
/// letters) with the text, else the most asked ones; links into the index.
pub fn nearest_searches(text: &str) -> String {
    const SEARCHES: [(&str, &str); 14] = [
        ("toggle switch", "kind=circuit&q=toggle"),
        ("one-shot timer", "kind=circuit&q=one+shot"),
        ("stay-on timer", "kind=circuit&q=stay+on"),
        ("piston extender", "kind=circuit&q=piston+extender"),
        ("logic gate", "kind=circuit&q=logic+gate"),
        ("delay", "kind=circuit&q=delay"),
        ("counter", "kind=circuit&q=counter"),
        ("block change detector", "kind=circuit&q=bud"),
        ("flying machine", "kind=circuit&q=flying+machine"),
        ("TNT duper", "kind=circuit&q=tnt"),
        ("shulker box loader", "kind=circuit&q=shulker"),
        ("pulse shortener or stretcher", "kind=circuit&q=pulse"),
        ("3×3 piston door", "kind=door"),
        ("clock", "kind=clock"),
    ];
    let words: Vec<String> = text.to_lowercase().split_whitespace().map(String::from).collect();
    let close = |label: &str| {
        let l = label.to_lowercase();
        words.iter().any(|w| l.split_whitespace().any(|x| x == w || (w.len() >= 4 && x.len() >= 4 && x[..4] == w[..4])))
    };
    let mut picked: Vec<&(&str, &str)> = SEARCHES.iter().filter(|(l, _)| close(l)).collect();
    if picked.is_empty() {
        picked = SEARCHES.iter().filter(|(l, _)| ["toggle switch", "one-shot timer", "piston extender", "3×3 piston door", "clock"].contains(l)).collect();
    }
    picked.iter().map(|(l, f)| format!("<a href=\"#{}\">{}</a>", esc(f), esc(l))).collect::<Vec<_>>().join(" · ")
}

/// The words a player may search a build by, beyond its name and caption: the community's names
/// for its kind (critique C12: "toggle", "one shot", "hidden door" found nothing useful).
pub fn search_aliases(b: &Build) -> &'static str {
    // (a sand elevator is no general extender: "double piston extender" is not its name)
    if b.group == "Sand elevator" {
        return "sand elevator lift sand up";
    }
    match b.kind.as_str() {
        "door" if crate::door_side(b) == 2 => "2x2 piston door hidden secret entrance",
        "door" => "3x3 piston door hidden secret entrance",
        "clock" => "clock pulser oscillator repeating",
        _ => match b.circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("")).unwrap_or("") {
            "rising-edge" => "rising edge detector pulse on press",
            // (the 0-9 display: what players search for)
            "seven-segment" => "7 segment display seven segment digit number display chest level storage counter item counter fill level comparator readout",
            "falling-edge" => "falling edge detector pulse on release",
            "dual-edge" => "dual edge detector both edges",
            "pulse-limiter" => "pulse limiter shortener",
            "pulse-extender" => "pulse extender stretcher",
            "tff" => "toggle t flip-flop tff on off switch",
            "tff-reset" => "toggle t flip-flop tff reset",
            "delay" => "delay line repeater",
            "gate" => "logic gate and or not xor nand nor xnor imply nimply",
            "rs-latch" => "rs latch sr latch set reset memory",
            "d-latch" => "d latch data memory",
            "dff" => "d flip-flop dff clocked memory",
            "dff-enable" => "d flip-flop dff enable",
            "shift" => "shift register",
            "counter" => "counter divider frequency divider",
            "piston-extender" => "piston extender double triple quadruple dpe tpe qpe",
            "piston-bridge" => "piston bridge hidden bridge retractable bridge drawbridge secret bridge slime bridge moat",
            // (the coordinator 00:31Z: "payload" and "two-way" stay search words)
            "flying-machine" => "flying machine flier engine payload cargo two-way back and forth docking station return flat tall",
            "shulker-loader" => "shulker box loader filler packer",
            "shulker-unloader" => "shulker box unloader emptier",
            "bud" => "bud switch block update detector",
            "tnt-duper" => "tnt duper duplicator world eater precise air burst",
            "sorter" => "item sorter filter hopper sorting system storage",
            "smelter" => "auto smelter super smelter furnace array automatic smelting ore fuel blast furnace raw iron double chest",
            "brewer" => "auto brewer automatic brewing stand potion brewer swiftness nether wart blaze powder",
            "composter" => "auto composter automatic composter bone meal farm compost crops seeds kelp sugar cane wheat",
            "lock" => "combination lock item frame password code vault door secret",
            "sequence-lock" => "password door combination lock button lock code lock sequence lock keypad buttons order",
            "tnt-duper-triggered" => "tnt duper duplicator cannon",
            "burst" => "burst pulse multiplier",
            "divider" => "clock divider frequency divider",
            "monostable" => "monostable one shot one-shot timer",
            "off-delay" => "off delay off-delay timer stay on",
            "on-delay" => "on delay on-delay hold to activate pulse filter",
            "pulse-train" => "pulse train rapid fire gated clock",
            // (the adder study, w86; the randomizers, clk-item)
            "adder" => "adder half adder full adder ripple carry binary addition calculator alu",
            "binary-counter" => "binary counter ripple counter count presses bits",
            "decoder" => "decoder binary decoder demultiplexer select one output",
            "mux" => "multiplexer mux selector data selector",
            "comparator" => "magnitude comparator number comparator compare less greater equal",
            "randomizer" => "randomizer random dice coin flip rng random number generator lottery",
            "harvester" => "crop farm auto farm automatic farm harvester sugar cane farm paper bamboo farm kelp farm pumpkin farm melon farm cactus farm",
            "item-elevator" => "item elevator dropper elevator item lift vertical item transport storage",
            "elevator" => "player elevator lift flying machine elevator floors up down",
            "station" => "minecart station rail station train station stop platform button railway",
            "cart-unloader" => "minecart unloader hopper minecart unloader cart unloader unload storage rail",
            "cart-loader" => "minecart loader hopper minecart loader cart loader load storage rail",
            "cart-line" => "minecart line item transport rail line hopper minecart carry items far away chest to chest storage system farm to base",
            "auto-door" => "automatic door auto door pressure plate door walk through door piston door 2x2 opens by itself",
            "crafter" => "auto crafter autocrafter crafter automatic crafting craft iron block sticks crafting table",
            _ => "",
        },
    }
}

/// The community's seamless tiers in words (properties.rs `seamless`: QUART = closed, nothing of
/// the machine shows on one side; SEMI = that and open; FULL = both sides, closed and open; SUPER
/// = FULL and every tick of the motion), short enough for a chip.
pub fn seamless_words(tier: &str) -> String {
    // the community regulation's tiers (piston-door properties.rs): QUART = closed, nothing of
    // the machine on one side; SEMI = QUART + the open doorway clean; FULL = QUART on both sides
    // + open clean; SUPER = FULL + clean while moving
    match tier {
        "SUPER" => "seamless: machine hidden from both sides, closed, open and while moving".to_string(),
        "FULL" => "seamless: machine hidden from both sides, closed and open".to_string(),
        "SEMI" => "machine hidden when open, and from one side when closed".to_string(),
        "QUART" => "machine hidden from one side when closed".to_string(),
        "" => String::new(),
        other => format!("{} seamless", other.to_lowercase()),
    }
}

/// A board a door leads, in words, with the door's own seamless tier for the seamless boards
/// (they rank the tier first, then the size: critique, "most compact hidden door" on a 41-block
/// row of a page about a 38-block door hidden when closed): `seamless` + `FULL` -> `most compact
/// fully seamless door`.
pub fn door_lead_words(board: &str, tier: &str) -> String {
    if let Some(rest) = board.strip_prefix("seamless") {
        let t = match tier {
            "SUPER" => "super-seamless",
            "FULL" => "fully seamless",
            "SEMI" => "half-seamless",
            "QUART" => "partly seamless",
            _ => "seamless",
        };
        let flat = if rest.contains("flat") { "1-block-thick " } else { "" };
        let slime = if rest.contains("no slime") { " with no slime" } else { "" };
        return format!("most compact {t} {flat}door{slime}");
    }
    lead_words(board)
}

/// A board a door leads, in words (`solid` -> `most compact stone door`, `sculk, not
/// rotation-safe` -> `most compact sculk door that only works facing one way`,
/// `fastest open (solid only)` -> `fastest stone door to open`).
pub fn lead_words(board: &str) -> String {
    let (base, one_way) = match board.strip_suffix(", not rotation-safe") {
        Some(b) => (b, true),
        None => (board, false),
    };
    let tail = if one_way { " that only works facing one way" } else { "" };
    if let Some(rest) = base.strip_prefix("fastest ") {
        let (what, scope) = rest.split_once(" (").map(|(w, s)| (w, s.trim_end_matches(')'))).unwrap_or((rest, ""));
        let what = match what {
            "open" => "to open",
            "close" => "to close",
            "open + close" => "to open and close",
            other => other,
        };
        let kind = if scope == "solid only" { "stone door" } else { "door" };
        return format!("fastest {kind} {what}{tail}");
    }
    // the size boards rank by the space a door takes, not its blocks (critique: "smallest hidden
    // door" on a 41-block card next to a 40-block one): "most compact"
    let what = match base {
        "solid" => "stone door",
        "sculk" => "sculk door",
        "gravity" => "gravity door (its doorway blocks are sand or gravel)",
        "entity" => "door that uses entities (a minecart or the like) as moving parts",
        "slime-free" => "door with no slime",
        "flat" => "1-block-thick door",
        "flat, no slime or honey" => "1-block-thick door with no slime",
        "seamless" => "hidden door",
        "seamless, flat, no slime or honey" => "hidden 1-block-thick door",
        "wall" => "door in a 3-thick wall",
        "ground" => "door that stands on flat ground (no digging below it)",
        "entities" => "door that uses entities (a minecart or the like) as moving parts",
        other => return format!("best: {other}{tail}"),
    };
    format!("most compact {what}{tail}")
}

/// What a chunk reload does to a clock, in a few words, where it matters (clockinfra's `reload`
/// column; the coordinator 19:20Z: a property, a chip where it matters): `stops`, `changes
/// period to 4 gt`, `fails: ...`; None for `ok`, `glitch then resumes` and unchecked.
pub fn reload_chip(reload: &str) -> Option<&'static str> {
    let what = reload.split(" in ").next().unwrap_or("").trim();
    if what == "stops" {
        Some("stops if its chunk unloads")
    } else if what.starts_with("changes period") {
        Some("period changes on chunk reload")
    } else if what.starts_with("fails") {
        Some("breaks on chunk reload")
    } else {
        None
    }
}

/// How far under a precise-drop duper its TNT explode, from the judge's summary (`in mid-air
/// 10.0 blocks below the machine`).
pub fn tnt_burst_depth(c: &crate::Circuit) -> Option<f64> {
    [c.verdict.as_str(), c.robustness.as_str()].iter().find_map(|t| {
        let i = t.find(" blocks below the machine")?;
        t[..i].rsplit(' ').next()?.parse::<f64>().ok()
    })
}

/// The air a TNT duper needs under it, in blocks: the lowest it survives at (its `safe drop`),
/// and where it is meant to stand when higher (`stand: N`, the judge's `stands`); a precise drop's
/// blast must be in mid-air, so it needs one block more than its blast's depth under it (its
/// declared stand, the judge's test height, can be higher than it needs); 0 for every other
/// design (the coordinator 15:18Z, vjeux: a duper standing 6 up takes the 6 blocks of air under it
/// too).
pub fn tnt_air(c: &crate::Circuit) -> u32 {
    if !c.kind.starts_with("tnt-duper") {
        return 0;
    }
    let n = |k: &str| cm(c, k).split_whitespace().next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    match tnt_burst_depth(c) {
        Some(d) => n("safe drop").max(d.floor() as u32 + 1),
        None => n("stands").max(n("safe drop")),
    }
}

/// A build's effective volume: its judge box, and for a TNT duper the air column it needs under
/// it (its footprint times [`tnt_air`]: `4x4x4` standing 6 up is 4·4·4 + 4·4·6 = 160), what the
/// TNT dupers rank by (the coordinator 15:18Z).
pub fn effective_volume(b: &Build) -> Option<u32> {
    // (the judge's own number when it gives one: clk-misc's `effective volume N (WxD footprint x
    // (H + S) high: the box and the air under it)`)
    if let Some(n) = b.circuit.as_ref().and_then(|c| cm(c, "effective volume").split_whitespace().next()?.parse::<u32>().ok()) {
        return Some(n);
    }
    let v = b.volume?;
    let air = b.circuit.as_ref().map(tnt_air).unwrap_or(0);
    if air == 0 {
        return Some(v);
    }
    let d: Vec<u32> = b.dims.as_deref().unwrap_or("").split('x').filter_map(|x| x.trim().parse().ok()).collect();
    // (the box is x × y × z, y up: the footprint is x × z)
    match d.as_slice() {
        [x, _, z] => Some(v + x * z * air),
        _ => Some(v),
    }
}

/// A TNT duper's rate in words, one number a builder would say (critique: "Lights 85.7 TNT/min"
/// next to the caption's "86 lit TNT a minute"): `85.7 TNT/min` -> `about 86 TNT a minute`,
/// `200 TNT/min` -> `200 TNT a minute`.
pub fn tnt_rate_words(rate: &str) -> String {
    let Some(n) = rate.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) else { return rate.to_string() };
    let r = n.round();
    if (r - n).abs() < 1e-9 { format!("{r} TNT a minute") } else { format!("about {r} TNT a minute") }
}

/// Block ids for people: `observer+redstone_lamp` -> `observer, redstone lamp`.
pub fn human_blocks(ids: &str) -> String {
    ids.split('+').filter(|s| !s.is_empty()).map(|s| s.trim_start_matches("minecraft:").replace('_', " ")).collect::<Vec<_>>().join(", ")
}

/// The chip of a clock with no spot to take its output into a circuit (the judge's `usable`: `no:
/// ...`; the page's warning says it in full).
pub const HARD_TO_WIRE: &str = "hard to wire into a circuit";

/// Which adder a design is (the adder study, w86 2026-10-21: every design is `circuit: adder 1`,
/// its archive axis says which): `half` (two bits into a sum and a carry), `full` (and a carry
/// in), `slice` (a full adder whose copies stack into an adder of any width); from its axis, else
/// its ports (a carry input: full); empty for no adder.
pub fn adder_variant(c: &crate::Circuit) -> &'static str {
    if !c.kind.starts_with("adder") {
        return "";
    }
    let a = c.axis.to_lowercase();
    if a.contains("slice") {
        "slice"
    } else if a.contains("half") {
        "half"
    } else if a.contains("full") || c.inputs.iter().any(|i| i == "cin") {
        "full"
    } else {
        "half"
    }
}

/// A randomizer's N (`randomizer 4`: one press lights one of 4 outputs; clk-item's study 16).
pub fn randomizer_n(c: &crate::Circuit) -> Option<u32> {
    c.kind.strip_prefix("randomizer ")?.split_whitespace().next()?.parse().ok()
}

/// An auto-crafter's recipe (clk-item's `crafter RESULT`): what one craft makes and what it takes
/// (`an iron block`, `9 iron ingots`); an unknown result: its name and "its recipe".
pub fn crafter_recipe(c: &crate::Circuit) -> (String, String) {
    let result = c.kind.strip_prefix("crafter ").map(str::trim).unwrap_or("");
    match result {
        "iron_block" => ("an iron block".into(), "9 iron ingots".into()),
        "gold_block" => ("a gold block".into(), "9 gold ingots".into()),
        "crafting_table" => ("a crafting table".into(), "4 planks".into()),
        "stick" => ("4 sticks".into(), "2 planks".into()),
        "tnt" => ("a TNT block".into(), "5 gunpowder and 4 sand".into()),
        other => {
            let w = other.trim_start_matches("minecraft:").replace('_', " ");
            let a = if w.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            (format!("{a} {w}"), "its recipe".into())
        }
    }
}

/// A crop farm's crop id (`harvester sugar_cane` -> `sugar_cane`; a namespaced `minecraft:melon`,
/// as w86's farm-gen writes the header, reads `melon`).
pub fn farm_crop(c: &crate::Circuit) -> Option<&str> {
    c.kind.strip_prefix("harvester ").map(|k| k.trim().trim_start_matches("minecraft:"))
}

/// A crop farm's crop in words (`harvester sugar_cane` -> `sugar cane`; w86's study 17).
pub fn crop_words(c: &crate::Circuit) -> String {
    farm_crop(c).map(|k| k.replace('_', " ")).unwrap_or_default()
}

/// What one plant of a crop farm is called (`cane`, a bamboo `stalk`, a pumpkin or melon `stem`):
/// the judge's "per cane".
pub fn plant_word(c: &crate::Circuit) -> &'static str {
    match farm_crop(c) {
        Some("sugar_cane") => "cane",
        Some("bamboo") => "stalk",
        Some("pumpkin" | "melon") => "stem",
        _ => "plant",
    }
}

/// A crop farm's crop as its harvest is named (`pumpkins`, `melon slices`, `sugar cane`): the
/// items it sends into the chest; one of them (a rate of about 1.0) in the singular.
pub fn harvest_words(c: &crate::Circuit) -> String {
    match farm_crop(c) {
        Some("pumpkin") => "pumpkins".into(),
        // (w86: a melon farm's items are melon slices)
        Some("melon") => "melon slices".into(),
        _ => crop_words(c),
    }
}

/// [`harvest_words`] for a rate shown as `{n:.1}` (`1.0 pumpkin`, `4.8 melon slices`).
pub fn harvest_words_at(c: &crate::Circuit, n: f64) -> String {
    let w = harvest_words(c);
    if format!("{n:.1}") == "1.0" && matches!(farm_crop(c), Some("pumpkin" | "melon")) { w.trim_end_matches('s').to_string() } else { w }
}

/// A crop farm's crop as a title names it (`pumpkins`, `melons`, `sugar cane`).
pub fn crop_plural(c: &crate::Circuit) -> String {
    match farm_crop(c) {
        Some("pumpkin") => "pumpkins".into(),
        Some("melon") => "melons".into(),
        _ => crop_words(c),
    }
}

/// A full-size crop farm (the coordinator via w86 2026-10-22: farms rank by items an hour per
/// block among farms of 64 plants or more; the 1-to-4-plant modules stay as variants).
pub const FULL_SIZE_PLANTS: u32 = 64;

/// Whether a crop farm is full-size ([`FULL_SIZE_PLANTS`] plants or more: its `port crop` cells).
pub fn full_size_farm(c: &crate::Circuit) -> bool {
    c.kind.starts_with("harvester") && c.crops >= FULL_SIZE_PLANTS
}

/// A crop that grows only in light 9 or more (`StemBlock.randomTick`, `BambooStalkBlock`:
/// `getRawBrightness >= 9`; fly's QA): melon and pumpkin stems, bamboo. Sugar cane, cactus and kelp
/// grow in the dark.
pub fn farm_needs_light(c: &crate::Circuit) -> bool {
    matches!(farm_crop(c), Some("melon" | "pumpkin" | "bamboo"))
}

/// The iron a design's blocks cost to craft, from the verdict's `bill` (`2112 iron, 64 quartz,
/// 304 redstone`).
pub fn bill_iron(c: &crate::Circuit) -> Option<u32> {
    bill_item(c, "iron")
}

/// The slimeballs a design's blocks cost (`…, 92 slime` in the verdict's `bill`).
pub fn bill_slime(c: &crate::Circuit) -> Option<u32> {
    bill_item(c, "slime")
}

/// One item's count in the verdict's `bill` (`1732 iron, 4 quartz, 12 redstone, 92 slime`).
fn bill_item(c: &crate::Circuit, item: &str) -> Option<u32> {
    cm(c, "bill").split(", ").find_map(|part| {
        let (n, what) = part.trim().split_once(' ')?;
        (what.trim() == item).then(|| n.parse().ok()).flatten()
    })
}

/// A number as a player reads a big rate (the coordinator's cards: `3326` -> `3,300`, `646` ->
/// `646`, `4.237` -> `4.2`, `0.524` -> `0.52`): two significant figures from 1,000 up, thousands
/// grouped; a whole number from 10; one decimal under 10; two under 1.
pub fn readable(x: f64) -> String {
    if !x.is_finite() || x <= 0.0 {
        return "0".into();
    }
    if x < 1.0 {
        let s = format!("{x:.2}");
        return if s == "1.00" { "1.0".into() } else { s };
    }
    if x < 10.0 {
        let s = format!("{x:.1}");
        return if s == "10.0" { "10".into() } else { s };
    }
    if x < 999.5 {
        return format!("{}", x.round() as u64);
    }
    let digits = x.log10().floor() as i32 + 1;
    let unit = 10f64.powi(digits - 2);
    group_thousands(((x / unit).round() * unit) as u64)
}

/// `3300` -> `3,300`.
pub fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A count rounded to two significant figures from 1,000 up, `~` before it when that changed it
/// (`2112` -> `~2,100`, `1836` -> `~1,800`, `64` -> `64`).
pub fn about_count(n: u32) -> String {
    // (a count under 1,000 as it is: `readable`'s one decimal under 10 is for rates, "~9.0 iron"
    // read wrong on the day-1 iron farm)
    if n < 1000 {
        return n.to_string();
    }
    let s = readable(n as f64);
    if s.replace(',', "") == n.to_string() { s } else { format!("~{s}") }
}

/// What a crop farm needs besides its blocks, for its title (the coordinator 2026-10-22: "needs
/// open sky or light 9" for the stems, "needs nothing but water" for sugar cane): melon, pumpkin
/// and bamboo grow only in light 9 ([`farm_needs_light`]); sugar cane and kelp only by water;
/// cactus says nothing.
pub fn farm_needs_words(c: &crate::Circuit) -> Option<&'static str> {
    match farm_crop(c) {
        Some("melon" | "pumpkin" | "bamboo") => Some("needs open sky or light 9"),
        Some("sugar_cane" | "kelp") => Some("needs nothing but water"),
        _ => None,
    }
}

/// How much faster than the game a crop farm's test (and its view, which replays it) grows the
/// crop: its judge's random tick speed over the game's 3 (`circuit::farm::game_rts`: 300 for a
/// farm of up to 8 plants, `100 times faster`; a big farm's [`crate::Circuit::farm_rts`], slower
/// so its collection keeps up: melon-lattice8 at 8, `about 3 times faster`).
pub fn farm_speed_words(c: &crate::Circuit) -> String {
    let rts = c.farm_rts.unwrap_or(300) as f64;
    let x = rts / 3.0;
    if (x - x.round()).abs() < 0.05 {
        format!("{} times faster", x.round())
    } else if x < 2.0 {
        format!("{x:.1} times faster")
    } else {
        format!("about {} times faster", x.round())
    }
}

/// The items a double chest holds (54 stacks of 64), and an 8-furnace super smelter's items an
/// hour at full speed (8 furnaces, one item per 200 game ticks): what a fuel farm's card measures
/// its harvest against (w86's designs/briefs/fuel-farm.md, 2026-10-22).
pub const DOUBLE_CHEST_ITEMS: f64 = 3456.0;
pub const SUPER_SMELTER_PER_HOUR: f64 = 2880.0;

/// A fuel farm's items smelted an hour (w86's `items smelted per hour per block` and `items
/// smelted per hour`, on a harvester whose crop burns in a furnace: bamboo so far, a quarter of an
/// item each): (per block, the whole farm). None for every other design.
pub fn fuel_rates(c: &crate::Circuit) -> Option<(f64, f64)> {
    if !c.kind.starts_with("harvester") {
        return None;
    }
    let num = |k: &str| cm(c, k).split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
    let total = num("items smelted per hour").filter(|t| *t > 0.0)?;
    Some((num("items smelted per hour per block").unwrap_or(0.0), total))
}

/// A fuel farm's items smelted an hour as a player reads them (the coordinator 2026-10-22,
/// "~1,350 items smelted an hour" for 1,357.4): to the nearest 50 from a thousand, 10 from a
/// hundred, `~` when rounded.
pub fn fuel_count(v: f64) -> String {
    let step = if v >= 1000.0 {
        50.0
    } else if v >= 100.0 {
        10.0
    } else {
        1.0
    };
    let r = (v / step).round() * step;
    format!("{}{}", if (r - v).abs() > 1e-9 { "~" } else { "" }, group_thousands(r as u64))
}

/// A share of a whole in words (`half`, `a third`, `a twelfth`, `two thirds`, `three quarters`),
/// the nearest within an eighth of it, else a percentage (`40%`).
pub fn share_words(r: f64) -> String {
    const NAMES: [&str; 19] = ["half", "a third", "a quarter", "a fifth", "a sixth", "a seventh", "an eighth", "a ninth", "a tenth", "an eleventh", "a twelfth", "a thirteenth", "a fourteenth", "a fifteenth", "a sixteenth", "a seventeenth", "an eighteenth", "a nineteenth", "a twentieth"];
    let mut best: Option<(f64, String)> = None;
    let mut consider = |value: f64, name: String| {
        let err = ((r - value) / value).abs();
        if err <= 0.125 && best.as_ref().is_none_or(|(e, _)| err < *e) {
            best = Some((err, name));
        }
    };
    for (i, name) in NAMES.iter().enumerate() {
        consider(1.0 / (i as f64 + 2.0), name.to_string());
    }
    consider(2.0 / 3.0, "two thirds".to_string());
    consider(0.75, "three quarters".to_string());
    best.map(|(_, n)| n).unwrap_or_else(|| format!("{:.0}%", r * 100.0))
}

/// A fuel farm's lead (the coordinator 2026-10-22: "fuels ~1,350 items smelted an hour: a double
/// chest of ore every 2.5 h, half of an 8-furnace super smelter at full speed"): its items
/// smelted an hour, how often that is a double chest of ore, and its share of an 8-furnace super
/// smelter at full speed.
pub fn fuel_lead(c: &crate::Circuit) -> Option<String> {
    let (count, every, smelter) = fuel_parts(c)?;
    Some(format!("fuels {count} items smelted an hour: a double chest of ore every {every}, {smelter}"))
}

/// [`fuel_lead`]'s parts: its items smelted an hour (`~1,350`), how often that is a double chest
/// of ore (`2.5 h`), its share of an 8-furnace super smelter (`half of an 8-furnace super smelter
/// at full speed`).
pub fn fuel_parts(c: &crate::Circuit) -> Option<(String, String, String)> {
    let (_, total) = fuel_rates(c)?;
    let hours = DOUBLE_CHEST_ITEMS / total;
    let every = if hours >= 10.0 {
        format!("{:.0} h", hours)
    } else if hours >= 1.0 {
        format!("{:.1} h", hours)
    } else {
        format!("{:.0} min", hours * 60.0)
    };
    let ratio = total / SUPER_SMELTER_PER_HOUR;
    // (never rounded up to "full speed" or "enough", the coordinator 2026-10-22 on the 10x64 field,
    // 2,750 of the 2,880 an hour: "96% of an 8-furnace super smelter at full speed"; a simple
    // fraction only for a small share, "half", "a twelfth"; a percentage rounded down)
    let smelter = if ratio >= 1.0 {
        "enough for an 8-furnace super smelter at full speed".to_string()
    } else if ratio >= 0.6 {
        format!("{:.0}% of an 8-furnace super smelter at full speed", (ratio * 100.0).floor())
    } else {
        format!("{} of an 8-furnace super smelter at full speed", share_words(ratio))
    };
    Some((fuel_count(total), every, smelter))
}

/// An iron farm's title after its blocks (the crop farms' "· 0.73 per block · ~2,200 iron + 92
/// slime · one flying machine sweeps the field"): its iron an hour per block, its iron bill (the
/// quartz left out, as a crop farm's), how it works in a few words (the design's `how:` line,
/// [`crate::Circuit::feature`]).
pub fn mob_farm_title_tail(c: &crate::Circuit) -> Option<String> {
    if !c.kind.starts_with("mob-farm") {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    // (each number with its unit, the bill as a cost: vjeux 2026-11-08 10:10, "1.6 per block" and
    // "11 iron" beside "295 iron an hour" read as two rates)
    if let Some(per_block) = cm(c, "iron per hour per block").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) {
        parts.push(format!("{} an hour per block", readable(per_block)));
    }
    if let Some(iron) = bill_iron(c) {
        parts.push(format!("{} iron to build", about_count(iron)));
    }
    if !c.feature.is_empty() {
        parts.push(c.feature.clone());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A crop farm's title after its blocks (the coordinator 2026-10-22: "3,300 melon slices an hour
/// · 64 stems · 785 blocks · 4.2 per block · ~2,100 iron · needs open sky or light 9"): its harvest
/// per block, its iron, the light it needs.
pub fn farm_title_tail(c: &crate::Circuit) -> Option<String> {
    let (per_block, total) = farm_rates(c)?;
    if total <= 0.0 {
        return None;
    }
    // (a fuel farm: its iron and slime only, the coordinator's "· bamboo count, blocks, iron +
    // slime"; its fuel per block, flying machine and light need go on the second line)
    let fuel = fuel_rates(c).is_some();
    // (each number with its unit, the bill as a cost: vjeux 2026-11-08 10:10)
    let mut parts = if fuel { Vec::new() } else { vec![format!("{} an hour per block", readable(per_block))] };
    if let Some(iron) = bill_iron(c).filter(|n| *n > 0) {
        // (slime from a slime block's worth up: a flying machine's, the coordinator's "~1,700
        // iron + 92 slime"; a sticky piston's one or two slimeballs are left out like the quartz)
        let slime = bill_slime(c).filter(|n| *n >= 9).map(|n| format!(" + {} slime", about_count(n))).unwrap_or_default();
        parts.push(format!("{} iron{slime} to build", about_count(iron)));
    }
    if fuel {
        return (!parts.is_empty()).then(|| parts.join(" · "));
    }
    // (how it harvests, when it is a flying machine: the coordinator's "one flying machine sweeps
    // the field"; then sugar cane's "needs nothing but water" goes, a light need stays)
    let machines = c.flying_machines;
    if machines > 0 {
        parts.push(if machines == 1 { "one flying machine sweeps the field".to_string() } else { format!("{machines} flying machines sweep the field") });
    }
    if let Some(needs) = farm_needs_words(c).filter(|n| machines == 0 || n.contains("light")) {
        parts.push(needs.into());
    }
    Some(parts.join(" · "))
}

/// What a design's card title says after its blocks: a crop farm's harvest per block, iron and
/// light ([`farm_title_tail`]); a super smelter's rest word ([`smelter_rest_word`]).
pub fn title_tail(c: &crate::Circuit) -> Option<String> {
    // (the 0-9 display: "reads at a glance · one digit", the coordinator 2026-10-22)
    if decimal_display(c) {
        return Some("reads at a glance · one digit".to_string());
    }
    // (an automatic door: "· no waiting, even sprinting", the coordinator and clk-obs 2026-10-22)
    if let Some(w) = door_wait_words(c) {
        return Some(w);
    }
    // (a whole minecart line: its iron and gold, then that its cart waits when there is nothing
    // to carry, from the trickle run; the coordinator 2026-10-22)
    if cart_line(c) {
        let bill = cm(c, "bill");
        let part = |item: &str| bill.split(", ").find(|p| p.ends_with(&format!(" {item}"))).map(str::to_string);
        let cost: Vec<String> = ["iron", "gold"].iter().filter_map(|i| part(i)).collect();
        let mut tail: Vec<String> = Vec::new();
        if !cost.is_empty() {
            tail.push(cost.join(" + "));
        }
        if cart_waits(c) {
            tail.push("the cart waits when there's nothing to carry".to_string());
        }
        return (!tail.is_empty()).then(|| tail.join(" · "));
    }
    farm_title_tail(c).or_else(|| mob_farm_title_tail(c)).or_else(|| smelter_rest_word(c).map(str::to_string))
}

/// A stem crop (pumpkin, melon): the fruit grows beside its stem, which stays.
pub fn stem_crop(c: &crate::Circuit) -> bool {
    plant_word(c) == "stem"
}

/// A crop farm's harvest at the game's normal growth speed (random tick speed 3): (items an hour
/// for each block placed, items an hour for the whole design). The farm's efficiency (vjeux
/// 19:03Z: "items gathered per hour ÷ blocks placed"): the judge's own `items per hour per block`
/// and `items per hour` when it gives them, else from its per-plant numbers (`items per hour per
/// cane` over `per cane` blocks; times its crop cells).
pub fn farm_rates(c: &crate::Circuit) -> Option<(f64, f64)> {
    if !c.kind.starts_with("harvester") {
        return None;
    }
    let num = |k: &str| cm(c, k).split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
    let per_plant = num("items per hour per cane");
    let blocks_per_plant = num("per cane").filter(|b| *b > 0.0);
    let per_block = num("items per hour per block").or_else(|| Some(per_plant? / blocks_per_plant?))?;
    let total = num("items per hour").or_else(|| per_plant.map(|p| p * c.crops.max(1) as f64)).unwrap_or(0.0);
    Some((per_block, total))
}

/// A rate in a builder's precision: two decimals under 10 (`0.53`), one above (`16.8`).
pub fn rate_words(x: f64) -> String {
    if x < 10.0 { format!("{x:.2}") } else { format!("{x:.1}") }
}

/// A piston bridge's walkway in one word (`slime`, `stone`), from its `walkway` metric.
pub fn walkway_word(c: &crate::Circuit) -> String {
    let w = cm(c, "walkway").trim_start_matches("minecraft:").replace('_', " ");
    w.strip_suffix(" block").map(str::to_string).unwrap_or(w)
}

/// An auto-smelter built of blast furnaces (its blocks: `blast_furnace`): twice as fast, ores,
/// raw metals and metal tools only.
pub fn smelter_blast(c: &crate::Circuit) -> bool {
    c.family.split('+').any(|k| k == "blast_furnace")
}

/// An auto-smelter's player number (sorter's `chest` run, designs/briefs/super-smelter.md): the
/// minutes it takes to smelt a double chest of raw iron (3456) with the coal the load needs plus
/// a stack, from a fresh build until the last ingot is in the output chest.
pub fn double_chest_minutes(c: &crate::Circuit) -> Option<f64> {
    cm(c, "minutes for a double chest").split_whitespace().next()?.parse().ok().filter(|m: &f64| *m > 0.0)
}

/// Minutes as a player reads a long job: `72 min`, `9.6 hours` from 100 minutes on.
pub fn minutes_words(m: f64) -> String {
    if m < 100.0 {
        format!("{m:.0}\u{a0}min")
    } else {
        let h = format!("{:.1}", m / 60.0);
        format!("{}\u{a0}hours", h.trim_end_matches(".0"))
    }
}

/// An auto-smelter's furnaces in words: `8 furnaces`, `16 blast furnaces`, `1 furnace`.
pub fn furnaces_words(c: &crate::Circuit) -> String {
    let n: u32 = c.kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1);
    format!("{n} {}furnace{}", if smelter_blast(c) { "blast " } else { "" }, if n == 1 { "" } else { "s" })
}

/// A super smelter's number as players compare it: `a double chest of raw iron in 72 min (100% of
/// 8 furnaces)`, the share of its furnaces' own limit from the verdict's `of the ceiling`.
pub fn smelter_number(c: &crate::Circuit, minutes: f64) -> String {
    let share = cm(c, "of the ceiling").split_whitespace().next().filter(|p| p.ends_with('%')).map(|p| format!("{p} of ")).unwrap_or_default();
    format!("a double chest of raw iron in {} ({share}{})", minutes_words(minutes), furnaces_words(c))
}

/// The coal a super smelter's double chest needs, in stacks: `~8 stacks of coal` (the verdict's
/// `coal needed 496 (the load's 432 and a stack)`: 7.75 stacks).
pub fn coal_stacks_words(c: &crate::Circuit) -> Option<String> {
    let n: u32 = cm(c, "coal needed").split_whitespace().next()?.parse().ok()?;
    let stacks = (n as f64 / 64.0).round().max(1.0) as u32;
    let about = if n % 64 == 0 { "" } else { "~" };
    Some(format!("{about}{stacks} stack{} of coal", if stacks == 1 { "" } else { "s" }))
}

/// A super smelter's title closes with whether it rests (`rests`: no clock runs once the chest
/// is empty) or not (`a clock runs on`), from the verdict's `at rest`.
pub fn smelter_rest_word(c: &crate::Circuit) -> Option<&'static str> {
    if !c.kind.starts_with("smelter") || double_chest_minutes(c).is_none() {
        return None;
    }
    match cm(c, "at rest") {
        r if r.starts_with("yes") => Some("rests"),
        r if r.starts_with("no") => Some("a clock runs on"),
        _ => None,
    }
}

/// A design's copy spacing, from its `tile: dx dy dz` header (the largest step), if it has one.
pub fn tile_step(c: &crate::Circuit) -> Option<i32> {
    let n: Vec<i32> = c.tile.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    n.iter().map(|v| v.abs()).max().filter(|v| *v > 0)
}

/// A randomizer's fairness, from the verdict's flag (`uniform: chi2 0.2 (limit 10.8) over 768
/// presses in 16 runs, counts 378/390`, crate::random::pool): (presses, runs, counts).
pub fn randomizer_fairness(c: &crate::Circuit) -> Option<(u32, u32, Vec<u32>)> {
    let t = [c.verdict.as_str(), c.robustness.as_str()].into_iter().find(|t| t.contains("uniform: "))?;
    let rest = t.split_once("uniform: ")?.1;
    let presses = rest.split_once(" over ")?.1.split_whitespace().next()?.parse().ok()?;
    let runs = rest.split_once(" presses in ")?.1.split_whitespace().next()?.parse().ok()?;
    let counts: Vec<u32> = rest.split_once("counts ")?.1.split(|ch: char| !(ch.is_ascii_digit() || ch == '/')).next()?.split('/').filter_map(|n| n.parse().ok()).collect();
    (!counts.is_empty()).then_some((presses, runs, counts))
}

/// The blocks a clock's card swaps in, in words, every wood or metal of one toggled component as
/// one word (`iron_trapdoor`, `oak_trapdoor` -> `trapdoor`; vjeux 13:09Z: "also works with
/// trapdoors, powered rails").
pub fn swap_words(ids: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim_start_matches("minecraft:");
        let word = if id.ends_with("_trapdoor") {
            "trapdoor".to_string()
        } else if id.ends_with("_door") {
            "door".to_string()
        } else if id.ends_with("_fence_gate") {
            "fence gate".to_string()
        } else if id.ends_with("copper_bulb") {
            "copper bulb".to_string()
        } else if id.ends_with("_skull") || id.ends_with("_head") {
            "mob head".to_string()
        } else {
            id.replace('_', " ")
        };
        if !out.contains(&word) {
            out.push(word);
        }
    }
    out.join(", ")
}

/// A door's category in words (the kind line).
pub fn door_category(group: &str) -> &'static str {
    match group {
        "solid" => "stone door",
        "sculk" => "triggered by a sculk sensor",
        "gravity" => "sand or gravel door blocks",
        "entity" => "uses moving entities",
        _ => "",
    }
}

/// `32 gt` -> `32 ticks (1.6 s)` (critique C50: every tick count with its seconds); a range ->
/// `4 to 8 ticks`.
fn tk(v: &str) -> String {
    let v = v.trim().trim_end_matches(" (from the press)");
    // `over 30 gt` -> `over 30 ticks (1.5 s)` (critique: "handles presses over 30 gt apart")
    if let Some(rest) = v.strip_prefix("over ") {
        return format!("over {}", tk(rest));
    }
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    match n.parse::<u32>() {
        Ok(1) => format!("1 tick ({})", crate::real_time(1)),
        Ok(x) => format!("{x} ticks ({})", crate::real_time(x)),
        // a range with its seconds too (critique: "4 to 6 ticks apart" had none)
        Err(_) => match n.split_once('-').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))) {
            Some((a, b)) => format!("{a} to {b} ticks ({} to {})", crate::real_time(a), crate::real_time(b)),
            None => format!("{} ticks", n.replace('-', " to ")),
        },
    }
}

/// `3` -> `3rd`.
fn nth(n: &str) -> String {
    let v: u32 = n.parse().unwrap_or(0);
    let suffix = match (v % 10, v % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{v}{suffix}")
}

/// A logic function by its rule (`nimply` -> `A AND NOT B`).
/// A circuit's sort value within its kind: a gate's function by how common it is (critique: "A AND
/// B" was 9th of 9), none for a shulker machine (its value is the stack size of its test items, so
/// they sort by blocks: critique, the smallest loader was card 2), else its main value.
pub fn sort_value(c: &crate::Circuit) -> Option<i64> {
    if c.kind.starts_with("gate ") {
        Some(gate_rank(c.kind.split_whitespace().nth(1).unwrap_or("")))
    } else if c.kind.starts_with("shulker-") {
        None
    } else if c.kind.starts_with("adder") {
        // half, full, then the slice (the slice's axis "4-bit adder slice" read as 4 put it first)
        Some(match adder_variant(c) {
            "half" => 0,
            "full" => 1,
            _ => 2,
        })
    } else {
        c.value
    }
}

/// A gate function's place in lists, the most common first (critique: "A AND B" was 9th of 9):
/// OR, AND, NOT, NOR, NAND, XOR, XNOR, then the one-sided ones.
pub fn gate_rank(f: &str) -> i64 {
    match f {
        "or" => 0,
        "and" => 1,
        "not" => 2,
        "nor" => 3,
        "nand" => 4,
        "xor" => 5,
        "xnor" => 6,
        "nimply" | "nimply-ba" => 7,
        "imply" | "imply-ba" => 8,
        "buffer" => 9,
        _ => 10,
    }
}

pub fn gate_label(f: &str) -> String {
    match f {
        "not" => "NOT".into(),
        "buffer" => "copy".into(),
        "and" => "A AND B".into(),
        "or" => "A OR B".into(),
        "xor" => "A XOR B (one, not both)".into(),
        "xnor" => "A same as B".into(),
        "nand" => "NOT (A AND B)".into(),
        "nor" => "NOT (A OR B)".into(),
        "imply" => "(NOT A) OR B".into(),
        "nimply" => "A AND (NOT B)".into(),
        "imply-ba" => "A OR (NOT B)".into(),
        "nimply-ba" => "B AND (NOT A)".into(),
        "not-a" => "NOT A".into(),
        "not-b" => "NOT B".into(),
        other => other.to_uppercase(),
    }
}

/// What a card leads with (vjeux 12:09Z: "put the focus on what are the properties of the build"):
/// the kind in words (small, above), the headline (the study's main property and the block
/// count), a line of secondary properties and chips; the design's name is only small secondary
/// text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    pub kind: String,
    pub title: String,
    pub sub: String,
    pub chips: Vec<String>,
}

/// A circuit metric's value (`""` when absent).
fn cm<'a>(c: &'a crate::Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// A circuit's main property in words, without its kind (`reach 3`, `slows by 3 · even on/off`,
/// `A AND B · instant`; empty when the kind has no number: a latch), and its secondary ones, from
/// the judge's metrics (vjeux 13:06Z: plain player language, no letters before meaning).
fn circuit_props(c: &crate::Circuit) -> (String, Vec<String>) {
    let mut w = c.kind.split_whitespace();
    let (k0, k1) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    let l = tk(cm(c, "L"));
    let num = |s: &str| s.split_whitespace().next().unwrap_or("").to_string();
    let said = |label: &str, k: &str| -> Option<String> {
        let v = cm(c, k);
        if v.is_empty() {
            return None;
        }
        // (no delay at all: "starts at once", not "starts after 0 ticks (0 s)")
        if label.starts_with("starts after") && v.split_whitespace().next() == Some("0") {
            return Some("starts at once".to_string());
        }
        Some(label.replace("{}", &tk(v)))
    };
    let sub = |keys: &[(&str, &str)]| -> Vec<String> { keys.iter().filter_map(|(label, k)| said(label, k)).collect() };
    match k0 {
        // (an extendable family's base shows the family's range: critique, "30-tick pulse" badged
        // "comes in every size"; a card's lead, when it has one, follows)
        "rising-edge" | "falling-edge" | "dual-edge" => {
            let main = format!("{}-tick pulse", num(cm(c, "L")));
            let lead = if c.lead.is_empty() { String::new() } else { format!(" · {}", c.lead) };
            (format!("{main}{lead}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")]))
        }
        "pulse-limiter" => (format!("at most {l}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")])),
        "pulse-extender" => (format!("at least {l}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")])),
        "tff" => {
            // what sets it apart from the other toggles leads (critique: a 7-block card lost every
            // number to the 3-block one with nothing saying what it adds)
            let lead = if c.lead.is_empty() { String::new() } else { format!("{} · ", c.lead) };
            (format!("{lead}{}", said("reacts in {}", "latency").unwrap_or_default()), sub(&[("handles presses {} apart", "min-period")]))
        }
        "delay" => {
            let n = if cm(c, "N").is_empty() { num(c.axis.trim_start_matches("N = ")) } else { num(cm(c, "N")) };
            // (a 0-tick delay is a signal carrier: critique, "0-tick delay" on a page of delays)
            (if n == "0" { "no delay: same-tick signal carrier".to_string() } else { format!("{n}-tick delay") }, sub(&[("needs an input of at least {}", "min-pulse"), ("two pulses {} apart stay separate", "min-gap")]))
        }
        "gate" => {
            let f = if cm(c, "function").is_empty() { k1 } else { cm(c, "function") };
            let instant = cm(c, "class") == "instant" || cm(c, "latency") == "0 gt";
            (format!("{}{}", gate_label(f), if instant { " · instant" } else { "" }), if instant { Vec::new() } else { sub(&[("reacts in {}", "latency")]) })
        }
        "rs-latch" | "d-latch" | "dff" | "dff-enable" | "tff-reset" => {
            let mut v = sub(&[("reacts in {}", "latency"), ("resets in {}", "reset")]);
            match cm(c, "enable") {
                // (the input's polarity in plain words: critique, "while the enable input is off"
                // next to another card's "while E is on")
                "low" => v.push("copies its input while its lock input is unpowered, holds while powered".into()),
                "high" => v.push("copies its input while its enable input is powered, holds while unpowered".into()),
                _ => {}
            }
            match cm(c, "edge") {
                "rising" => v.push("copies when the clock turns on".into()),
                "falling" => v.push("copies when the clock turns off".into()),
                _ => {}
            }
            (String::new(), v)
        }
        "shift" => (format!("{k1} bits"), sub(&[("reacts in {}", "latency")])),
        // a randomizer (clk-item's study 16): how many outputs it picks among, then how the chosen
        // one shows (held until the next press, or a pulse), how soon, and its fairness
        "randomizer" => {
            let n = randomizer_n(c).map(|n| n.to_string()).unwrap_or_else(|| k1.to_string());
            let mut rest: Vec<String> = Vec::new();
            match cm(c, "style") {
                "hold" => rest.push("the chosen output stays on until the next press".to_string()),
                "pulse" => rest.push(match cm(c, "pulse") {
                    "" => "the chosen output gives one pulse".to_string(),
                    p => format!("the chosen output gives one {} pulse", tk(p)),
                }),
                _ => {}
            }
            rest.extend(sub(&[("lights after {}", "latency")]));
            if let Some((presses, ..)) = randomizer_fairness(c) {
                rest.push(format!("fair over {presses} presses"));
            }
            (format!("1 of {n} outputs at random"), rest)
        }
        // a crop farm (w86's study 17): its crop, then its numbers in a builder's words
        "harvester" => {
            // the FARM's harvest leads (vjeux 21:39Z, "50 melon slices an hour is pitiful"; the
            // coordinator 2026-10-22: "3,300 melon slices an hour · 64 stems · 785 blocks · 4.2 per
            // block · ~2,100 iron · needs open sky or light 9": the blocks and what follows them
            // are `props`' and [`farm_title_tail`]'s); at the game's normal speed, which a player
            // assumes (vjeux 2026-11-08 10:10: "Normal speed is expected"), so not said
            let rates = farm_rates(c);
            let plant = plant_word(c);
            let mut rest: Vec<String> = Vec::new();
            // (2 tall: as soon as it grows, unless a block waits for another plant's step: w86's
            // two-cane farm harvests its west cane up to 91 gt later; 3 is a full sugar cane, a
            // bamboo grows on to 16)
            let wait = cm(c, "wait").split_whitespace().next().and_then(|v| v.parse::<u32>().ok());
            match cm(c, "harvest height").split_whitespace().next() {
                Some("2") if wait.is_none_or(|w| w <= 5) => rest.push("takes each block as soon as it grows".to_string()),
                Some("3") if plant == "cane" => rest.push("harvests each cane when it is full".to_string()),
                Some(h) if h.parse::<u32>().is_ok() => rest.push(format!("harvests each {plant} at {h} blocks tall")),
                _ => {}
            }
            let n = c.crops.max(1);
            let plants = format!("{n} {plant}{}", if n == 1 { "" } else { "s" });
            // a fuel farm (w86's bamboo sweeper): what its harvest smelts leads (the coordinator
            // 2026-10-22: "fuels ~1,350 items smelted an hour: a double chest of ore every 2.5 h,
            // half of an 8-furnace super smelter at full speed", then the bamboo count, blocks, iron
            // + slime); its fuel per block, its raw harvest, its flying machine and light need on
            // the second line
            if let (Some(lead), Some((fpb, _))) = (fuel_lead(c), fuel_rates(c)) {
                let mut more = vec![format!("{} items smelted an hour per block", readable(fpb))];
                if let Some((_, total)) = rates.filter(|(_, t)| *t > 0.0) {
                    more.push(format!("{} {} an hour", readable(total), harvest_words_at(c, total)));
                }
                if c.flying_machines > 0 {
                    more.push(if c.flying_machines == 1 { "one flying machine sweeps the field".to_string() } else { format!("{} flying machines sweep the field", c.flying_machines) });
                }
                if let Some(needs) = farm_needs_words(c) {
                    more.push(needs.to_string());
                }
                more.extend(rest);
                let crop = farm_crop(c).unwrap_or("").replace('_', " ");
                return (format!("{lead} · {n} {crop} {plant}{}", if n == 1 { "" } else { "s" }), more);
            }
            let main = match rates {
                Some((_, total)) if total > 0.0 => format!("{} {} an hour · {plants}", readable(total), harvest_words_at(c, total)),
                Some((per_block, _)) => format!("{} {} an hour per block · {plants}", readable(per_block), harvest_words_at(c, per_block)),
                None => format!("harvests {} by itself", crop_plural(c)),
            };
            (main, rest)
        }
        // an iron farm (ironfarm's study): its iron an hour leads (the crop farms' "800 sugar cane
        // an hour · 256 canes"); its blocks, iron per block, bill and how it works follow the
        // blocks ([`mob_farm_title_tail`]); then, in a player's words (vjeux 2026-11-08 10:10: for
        // the player deciding to build it), its share of the game's limit, how often a golem comes
        // and what it needs; the first night's hide is a build step, not a subtitle
        "mob-farm" => {
            let num = |k: &str| cm(c, k).split_whitespace().next().and_then(|v| v.trim_end_matches('%').parse::<f64>().ok());
            let main = num("iron per hour").map(|r| format!("{} iron an hour", readable(r))).unwrap_or_default();
            let mut rest: Vec<String> = Vec::new();
            if let Some(share) = cm(c, "share of ceiling").split_whitespace().next().filter(|s| !s.is_empty()) {
                rest.push(format!("{share} of the game's maximum"));
            }
            if let Some(s) = num("seconds per golem") {
                rest.push(format!("a golem every {} s", s.round()));
            }
            let item = |one: &str, many: &str| bill_item(c, many).or_else(|| bill_item(c, one)).unwrap_or(0);
            let (villagers, beds, zombies) = (item("villager", "villagers"), item("bed", "beds"), item("zombie", "zombies"));
            if villagers > 0 {
                let zombie = if zombies == 1 { "a zombie".to_string() } else { format!("{zombies} zombies") };
                rest.push(if beds > 0 { format!("needs {villagers} villagers, {beds} beds and {zombie}") } else { format!("needs {villagers} villagers and {zombie}") });
            }
            if cm(c, "restarts after a missed night").starts_with("yes") {
                rest.push("starts again by itself after a missed night".to_string());
            }
            (main, rest)
        }
        // w86's computational circuits (the compute study): what it does with its bits leads
        "binary-counter" => {
            let n: u32 = k1.parse().unwrap_or(4);
            let top = if n < 31 { (1u64 << n) - 1 } else { 0 };
            let mut rest: Vec<String> = Vec::new();
            if let Some(per) = cm(c, "per bit").split_whitespace().next().filter(|p| !p.is_empty()) {
                rest.push(format!("{per} blocks per bit"));
            }
            rest.extend(sub(&[("reacts in {}", "latency")]));
            (format!("counts presses from 0 to {top} in binary"), rest)
        }
        "decoder" => {
            let n: u32 = k1.parse().unwrap_or(2);
            (format!("lights 1 of {} outputs from a {n}-bit number", 1u32 << n.min(16)), sub(&[("reacts in {}", "latency")]))
        }
        "mux" => {
            let n: u32 = k1.parse().unwrap_or(2);
            (format!("passes 1 of {} inputs, picked by {n} select lines", 1u32 << n.min(16)), sub(&[("reacts in {}", "latency")]))
        }
        "comparator" => (format!("compares two {k1}-bit numbers: less, equal or greater"), sub(&[("reacts in {}", "latency")])),
        // the adder study (w86): which adder it is leads (every design is `adder 1`: "1 bits"
        // said nothing)
        "adder" => {
            let main = match adder_variant(c) {
                "half" => "half adder: two bits into a sum and a carry".to_string(),
                "full" => "full adder: two bits and a carry in".to_string(),
                "slice" => "adder slice: copies stack into any width".to_string(),
                _ => format!("{k1} bits"),
            };
            // a slice: its blocks per bit and how its time grows with each bit (the judge's
            // `slice` metric: `54 blocks per bit, latency 18 + 12 (N-1) gt`)
            let law = cm(c, "slice").split_once("latency ").map(|(_, l)| l.trim()).and_then(|l| {
                let (first, rest) = l.split_once(" + ")?;
                let step = rest.split_whitespace().next()?;
                Some(format!("reacts in {}, {} more for each bit after the first", tk(&format!("{first} gt")), tk(&format!("{step} gt"))))
            });
            let rest = match (adder_variant(c), law) {
                ("slice", Some(law)) => {
                    let per = cm(c, "slice").split(',').next().unwrap_or("").trim().to_string();
                    [per, law].into_iter().filter(|s| !s.is_empty()).collect()
                }
                ("slice", None) => std::iter::once("one copy per bit".to_string()).chain(sub(&[("reacts in {}", "latency")])).collect(),
                _ => sub(&[("reacts in {}", "latency")]),
            };
            (main, rest)
        }
        // (a counter whose output toggles says so: it reads differently from a level counter of
        // twice its count, critique)
        "counter" => (if cm(c, "output").starts_with("toggles") { format!("toggles every {} pulse", nth(k1)) } else { format!("every {} pulse", nth(k1)) }, sub(&[("reacts in {}", "latency")])),
        "piston-extender" => {
            let (e, r) = (tk(cm(c, "extend")), tk(cm(c, "retract")));
            let timing = if e.is_empty() { None } else { Some(format!("out in {e}, back in {r}")) };
            // what sets it apart from the others of its reach leads (critique: three reach-2
            // cards read the same)
            let lead = if c.lead.is_empty() { String::new() } else { format!(" · {}", c.lead) };
            (format!("reach {k1}{lead}"), timing.into_iter().collect())
        }
        // a piston bridge (pext's `piston-bridge N`): the gap it spans, then how fast, its walkway
        // and its width when wider than 1
        "piston-bridge" => {
            let (e, r) = (tk(cm(c, "extend")), tk(cm(c, "retract")));
            let mut rest: Vec<String> = Vec::new();
            if !e.is_empty() {
                rest.push(format!("out in {e}, back in {r}"));
            }
            let walk = walkway_word(c);
            if !walk.is_empty() {
                rest.push(format!("a {walk} walkway"));
            }
            if let Some(w) = cm(c, "width").split_whitespace().next().and_then(|w| w.parse::<u32>().ok()).filter(|w| *w > 1) {
                rest.push(format!("{w} blocks wide"));
            }
            (format!("spans a {k1}-block gap"), rest)
        }
        "flying-machine" => {
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            // (direction words: "flies sideways", not "flies horizontal": critique)
            let flies = match c.direction.as_str() {
                "horizontal" => "flies sideways".to_string(),
                "" => "flies".to_string(),
                d => format!("flies {d}"),
            };
            let speed = cm(c, "speed").split_whitespace().next().and_then(|g| g.parse::<f64>().ok()).filter(|g| *g > 0.0).map(|g| {
                let s = format!("{:.2}", 20.0 / g);
                format!("{} blocks a second", s.trim_end_matches('0').trim_end_matches('.'))
            });
            // the title says what sets it apart, the rest goes under it (critique: five
            // "horizontal · 2 blocks a second" cards differed only by block count; a payload
            // title ran 4 lines)
            let (lead, mut rest) = if two_way {
                // (vjeux 00:31Z: "For back and forth, can you call this docking station"; the
                // coordinator 00:45Z: its lever is a real input, said plainly on the card)
                let lever = c.devices.iter().any(|d| d == "lever");
                let trip = cm(c, "two-way").split("round trip ").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|g| g.parse::<u32>().ok()).map(|g| format!("a round trip every {}", tk(&format!("{g} gt"))));
                let mut rest: Vec<String> = trip.into_iter().collect();
                if lever {
                    rest.insert(0, "its lever: off, it docks and stays; on, it leaves at once".to_string());
                }
                (if lever { "flies to its docking stations and back · an on/off lever".to_string() } else { "flies to its docking stations and back".to_string() }, rest)
            } else if c.feature.starts_with("carries") && c.extend_n.is_some() {
                // a tall machine's cargo capacity is its headline (vjeux 00:31Z: a property)
                (format!("carries up to {} extra blocks", c.extend_n.map(|n| n.1).unwrap_or(0)), vec![flies])
            } else if !c.feature.is_empty() {
                (c.feature.clone(), vec![flies])
            } else {
                (flies, vec![])
            };
            rest.extend(speed);
            rest.extend(sub(&[("starts in {}", "latency")]));
            (lead, rest)
        }
        "shulker-loader" | "shulker-unloader" => {
            // what the card is best at leads (critique: cards with the same numbers differed only
            // by their badges); the items it is timed with go in the second line (critique: "first
            // box after 11 min" with nothing saying 64-stack items take 1728 to fill a box)
            let rate = cm(c, "items/s");
            // (the time to fill one box, to the half minute: critique, "first box after 11 min
            // 33.75 s" said nothing of what it measured)
            let unloader = c.kind.starts_with("shulker-unloader");
            let first = cm(c, "first box").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| {
                // (an unloader empties its box: critique, "fills a box" under "empties full boxes")
                let verb = if unloader { "empties" } else { "fills" };
                let secs = g as f64 / 20.0;
                if secs < 90.0 {
                    format!("{verb} a box in about {} s", secs.round())
                } else {
                    let halves = (secs / 30.0).round() as u64;
                    format!("{verb} a box in about {}{} min", halves / 2, if halves % 2 == 1 { "½" } else { "" })
                }
            });
            let lead = if c.lead.is_empty() { String::new() } else { format!("{} · ", badge_words(&c.lead)) };
            let stack = c.axis.split_whitespace().find_map(|w| w.strip_suffix("-stack")).unwrap_or(k1);
            let items = match stack {
                "" => None,
                "1" => Some("timed with unstackable items".to_string()),
                n => Some(format!("timed with {n}-stack items")),
            };
            (if rate.is_empty() { lead.trim_end_matches(" · ").to_string() } else { format!("{lead}{rate} items a second") }, items.into_iter().chain(first).collect())
        }
        "bud" => {
            let d: Vec<&str> = cm(c, "detects").split_whitespace().map(|k| match k {
                "place" => "placed",
                "remove" => "removed",
                "state" => "changed",
                "shape" => "grown",
                "update" => "updated",
                other => other,
            }).collect();
            (if d.is_empty() { String::new() } else { format!("notices blocks {}", d.join(", ")) }, sub(&[("reacts in {}", "latency"), ("{} output pulses", "pulse"), ("ready again after {}", "re-arm")]))
        }
        "tnt-duper" => {
            // the variation first, the rate second (critique: three cards of 30, 43 and 75 blocks
            // read the same): the axis names it after the rate (`13.2 TNT/min precise air burst`);
            // an on/off switch is a property (`..., switchable`: its lever's chip says it)
            let variation = c.axis.split_once("TNT/min").map(|(_, v)| v.trim()).unwrap_or("");
            let variation = variation.replace("switchable", "");
            let variation = variation.trim().trim_end_matches(',').trim();
            let safe = cm(c, "safe drop").starts_with("0 ");
            // where it is meant to stand (`stand: N`: the judge's `stands`), said plainly
            let stand = cm(c, "stands").split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).filter(|n| *n > 0);
            let stand_words = stand.map(|n| format!("stand it {n} blocks up"));
            let lead = match variation {
                "precise air burst" => "explodes each TNT at one exact point in the air",
                "precise drop" => "drops each TNT straight down to explode at one point",
                "precise" => "lands each TNT on the same spot",
                _ if safe => "safe standing on the ground",
                _ if stand_words.is_some() => stand_words.as_deref().unwrap_or(""),
                "launched" => "throws its TNT far away",
                _ => "",
            };
            let rate = tnt_rate_words(cm(c, "rate"));
            (if lead.is_empty() { rate } else { format!("{lead} · {rate}") }, sub(&[("a push every {}", "period"), ("first TNT after {}", "latency")]))
        }
        // (its own button: one per press, tnt-button-17b; else the judge's short pulse: a press of
        // a button is longer and lights more, the needs line says)
        "tnt-duper-triggered" => {
            let n = if cm(c, "per pulse").is_empty() { "1" } else { cm(c, "per pulse") };
            if c.devices.iter().any(|d| d == "button") {
                (format!("{n} TNT per press"), sub(&[("lights after {}", "latency"), ("presses at least {} apart", "gap")]))
            } else {
                (format!("{n} TNT per short pulse"), sub(&[("lights after {}", "latency"), ("pulses at least {} apart", "gap")]))
            }
        }
        // an item sorter (study 13): what it sorts and how fast, the first item's time second
        "sorter" => {
            let rate = cm(c, "items/s");
            let first = cm(c, "first item").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("its first item sorted within {}", crate::real_time(g)));
            // (the rate goes in the second line: critique b0fb9ae4e, a 4-line title pushed the 3D
            // view off a phone's first screen)
            let rate = rate.trim_end_matches('0').trim_end_matches('.').to_string();
            (
                "takes one item kind out of a mixed line".to_string(),
                (!rate.is_empty()).then(|| format!("{rate} items a second")).into_iter().chain(first).collect(),
            )
        }
        "burst" => (format!("{k1} pulses per press"), sub(&[("{} apart", "period"), ("starts after {}", "latency")])),
        // an auto-smelter (study 17): its furnaces and how fast each smelts, the first result
        "smelter" => {
            let n: u32 = k1.parse().unwrap_or(1);
            // a super smelter's player number leads (the coordinator 2026-10-22: "smelts a double
            // chest of raw iron in 72 min · 8 furnaces · 108 blocks · needs only the coal the load
            // burns plus a stack · rests when idle"), from sorter's `chest` run
            if let Some(m) = double_chest_minutes(c) {
                // (the reviewer via the coordinator 2026-10-22: "a double chest of raw iron in 72
                // min (100% of 8 furnaces) · ~8 stacks of coal · 108 blocks · rests"; the blocks
                // and the rest word are added by `props`)
                let mut sub = Vec::new();
                if smelter_blast(c) && !c.claim.contains("ores and raw metals only") {
                    sub.push("ores and raw metals only".into());
                }
                let main = match coal_stacks_words(c) {
                    Some(coal) => format!("{} · {coal}", smelter_number(c, m)),
                    None => smelter_number(c, m),
                };
                return (main, sub);
            }
            let tile = !cm(c, "tileable").is_empty();
            let head = if tile { format!("{n} furnace{} a slice · slices side by side", if n == 1 { "" } else { "s" }) } else { format!("{n} furnace{}", if n == 1 { "" } else { "s" }) };
            let rate = cm(c, "items/min per furnace");
            let first = cm(c, "first item").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("its first result within {}", crate::real_time(g)));
            (head, (!rate.is_empty()).then(|| format!("{rate} items a minute per furnace")).into_iter().chain(first).collect())
        }
        // an auto-brewer (study 21): its stands and how fast each brews, the first potion
        "brewer" => {
            let n: u32 = k1.parse().unwrap_or(1);
            let tile = !cm(c, "tileable").is_empty();
            let head = if tile { format!("{n} brewing stand{} a slice · slices side by side", if n == 1 { "" } else { "s" }) } else { format!("{n} brewing stand{}", if n == 1 { "" } else { "s" }) };
            let rate = cm(c, "potions/min per stand");
            let first = cm(c, "first potion").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("its first potion within {}", crate::real_time(g)));
            (head, (!rate.is_empty()).then(|| format!("{rate} potions a minute per stand")).into_iter().chain(first).collect())
        }
        // an auto-composter (study 22): its composters, how much it composts, the first bone meal
        "composter" => {
            let n: u32 = k1.parse().unwrap_or(1);
            let tile = !cm(c, "tileable").is_empty();
            let head = if tile { format!("{n} composter{} a slice, one crop each · slices side by side", if n == 1 { "" } else { "s" }) } else { format!("{n} composter{}", if n == 1 { "" } else { "s" }) };
            let rate = cm(c, "items composted/min");
            let first = cm(c, "first bone meal").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("its first bone meal within {}", crate::real_time(g)));
            (head, (!rate.is_empty()).then(|| format!("{rate} items composted a minute")).into_iter().chain(first).collect())
        }
        // an auto-crafter (clk-item's study): what it crafts from what, then how fast and how soon
        "crafter" => {
            let (makes, from) = crafter_recipe(c);
            let rate = cm(c, "crafts/min");
            let first = cm(c, "first craft").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("the first craft after {}", crate::real_time(g)));
            (format!("crafts {makes} from every {from}"), (!rate.is_empty()).then(|| format!("{rate} crafts a minute")).into_iter().chain(first).collect())
        }
        // a button sequence lock (clk-item's `sequence-lock N L`): its code's size leads, then how
        // soon it opens and closes
        "sequence-lock" => {
            let mut w = c.kind.split_whitespace().skip(1);
            let (n, l) = (w.next().unwrap_or("2"), w.next().unwrap_or("3"));
            let gt = |k: &str| cm(c, k).split_whitespace().next().and_then(|g| g.parse::<u32>().ok());
            let mut rest: Vec<String> = Vec::new();
            if let Some(o) = gt("opens within") {
                rest.push(format!("opens within {}", crate::real_time(o)));
            }
            if let Some(o) = gt("closes within") {
                rest.push(format!("closes within {}", crate::real_time(o)));
            }
            // (how many press sequences we tried is our business, not the builder's: vjeux
            // 2026-11-08 10:10)
            (format!("a code of {l} presses on {n} buttons"), rest)
        }
        // an automatic door (clk-obs's autodoor study): what it does, then how soon it closes
        // behind you (its wait is the title's tail: "no waiting, even sprinting")
        "auto-door" => {
            let gt = |k: &str| cm(c, k).split_whitespace().next().and_then(|g| g.parse::<u32>().ok());
            let mut rest: Vec<String> = Vec::new();
            if let Some(d) = gt("close delay") {
                rest.push(format!("it closes {} after you pass", crate::real_time(d)));
            }
            ("opens as you walk up, closes behind you".to_string(), rest)
        }
        // a hopper-minecart unloader or loader (clk-obs's rail study, circuit::cartio): what it does
        // to the cart, then how fast and how soon the cart leaves
        "cart-unloader" | "cart-loader" => {
            let unload = k0 == "cart-unloader";
            let through = c.axis.contains("through");
            let rate = cm(c, if unload { "unload rate" } else { "load rate" }).split_whitespace().next().unwrap_or("").to_string();
            let delay = cm(c, "departure delay").split_whitespace().next().and_then(|g| g.parse::<u32>().ok());
            let main = match (unload, through) {
                (true, false) => "empties a hopper minecart into a chest, then sends it back",
                (true, true) => "empties a hopper minecart into a chest, then sends it on",
                (false, false) => "fills a hopper minecart from a chest, then sends it back",
                (false, true) => "fills a hopper minecart from a chest, then sends it on",
            };
            let mut rest: Vec<String> = Vec::new();
            if !rate.is_empty() {
                rest.push(format!("{rate} items a second"));
            }
            if let Some(d) = delay {
                rest.push(format!("the cart leaves {} after it is {}", crate::real_time(d), if unload { "empty" } else { "full" }));
            }
            (main.to_string(), rest)
        }
        // a whole minecart line (sorter's crate::cartsys, the site's `cart-line`): a double chest
        // carried over its track, in minutes (the coordinator's card, 2026-10-22: "moves a double
        // chest 128 blocks in 21 min · ~65 iron + 24 gold · the cart waits when there's nothing to
        // carry"; its bill and the waiting are the title's tail)
        "cart-line" => {
            let min = cm(c, "minutes for a double chest").split_whitespace().next().and_then(|m| m.parse::<f64>().ok());
            let main = match (c.value, min) {
                (Some(n), Some(m)) => format!("moves a double chest {n} blocks in {} min", m.round()),
                (_, Some(m)) => format!("moves a double chest in {} min", m.round()),
                _ => "carries a chest's items to a chest far away".to_string(),
            };
            let mut rest: Vec<String> = Vec::new();
            if let Some(r) = cm(c, "items/s").split_whitespace().next().filter(|r| !r.is_empty()) {
                rest.push(format!("{r} items a second"));
            }
            (main, rest)
        }
        // a player elevator (the fly study's `elevator N`: a two-way flying machine carries a
        // platform between two floors): how high, the trips, the buttons
        "elevator" => {
            let t = |k: &str| cm(c, k).split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(crate::real_time);
            let trips = match (t("trip up"), t("down")) {
                (Some(u), Some(d)) => Some(format!("up in {u}, down in {d}")),
                _ => None,
            };
            let buttons = (!cm(c, "calls and sends").is_empty()).then(|| "a button on each floor calls it or sends it".to_string());
            (format!("carries a player {k1} blocks up and down"), trips.into_iter().chain(buttons).collect())
        }
        // an item elevator (clk-misc's study): how high, then how fast and how soon
        "item-elevator" => {
            let rate = cm(c, "items/s");
            let first = cm(c, "first item").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("the first item at the top after {}", crate::real_time(g)));
            // (a clock that runs only while items come: clk-misc's bubble-rest)
            let rests = cm(c, "at rest").starts_with("yes").then(|| "rests when it has nothing to lift".to_string());
            (format!("lifts items {k1} blocks up"), (!rate.is_empty()).then(|| format!("{rate} items a second")).into_iter().chain(first).chain(rests).collect())
        }
        // a minecart station (clk-obs's study 19): which line, then how it stops and sends a cart
        "station" => {
            // (blocks a tick -> blocks a second: 20 ticks a second)
            let speed = |k: &str| cm(c, k).split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).map(|v| format!("{:.1} blocks a second", v * 20.0));
            let two = k1 == "2";
            let mut rest: Vec<String> = Vec::new();
            if let Some(d) = cm(c, "stop distance").split_whitespace().next().filter(|d| !d.is_empty()) {
                rest.push(format!("stops every cart within {d} blocks"));
            }
            if let Some(s) = speed("leaves at") {
                rest.push(format!("sends it off at {s}"));
            }
            if let Some(s) = speed("other way passes at").filter(|_| two) {
                rest.push(format!("the other way passes at {s}"));
            }
            (if two { "a stop on a two-way line".to_string() } else { "a stop on a one-way line".to_string() }, rest)
        }
        // (what sets a divider apart from a counter leads: how fast a clock it follows; critique:
        // "slows by 3 · 55 blocks" next to a 17-block counter needed its reason)
        "divider" => {
            let follows = cm(c, "min-period");
            let even = if cm(c, "output") == "square" { " · even on/off" } else { "" };
            (if follows.is_empty() { format!("slows by {k1}{even}") } else { format!("slows by {k1} · input clocks of a pulse every {} or slower{even}", tk(follows)) }, Vec::new())
        }
        "monostable" => (
            format!("on for {l}{}", if k1 == "retriggerable" { " · restarts on every press" } else if k1 == "non-retriggerable" { " · ignores presses while on" } else { "" }),
            // (a retriggerable timer's re-arm reads like a contradiction next to "restarts on
            // every press": critique C49; it stays in the measurements)
            if k1 == "retriggerable" { sub(&[("starts after {}", "delay")]) } else { sub(&[("starts after {}", "delay"), ("ready again after {}", "re-arm")]) },
        ),
        "off-delay" => (format!("stays on {l} after"), sub(&[("needs an input of at least {}", "min-pulse")])),
        "on-delay" => (format!("must be held {}", tk(cm(c, "T"))), sub(&[("its output is {} shorter than the input", "shortening")])),
        "pulse-train" => (format!("a pulse every {}", tk(cm(c, "P"))), sub(&[("{} pulses", "pulse"), ("first after {}", "latency")])),
        // (a display decoder's number is its input lines, in words: "4 · 566 blocks" said nothing)
        // the 0-9 display (the coordinator 2026-10-22: "shows 0–9 from a container's fill level ·
        // 496 blocks · reads at a glance · one digit"; the blocks and the tail: `props`)
        "seven-segment" if decimal_display(c) => {
            let mut rest = Vec::new();
            if let Some(g) = first_ticks(cm(c, "latency")) {
                rest.push(format!("the digit changes within {}", crate::real_time(g)));
            }
            if cm(c, "on-blips").split_whitespace().next() == Some("0") {
                rest.push("no wrong lamp flashes between digits".to_string());
            }
            ("shows 0–9 from a container's fill level".to_string(), rest)
        }
        "seven-segment" => (value_words(&c.kind, &c.kind), sub(&[("reacts in {}", "latency")])),
        _ => {
            let axis = if c.axis == c.kind { String::new() } else { axis_shown(&c.kind, &c.axis).unwrap_or_default() };
            let n = if k1.is_empty() || axis.contains(k1) { String::new() } else { k1.to_string() };
            ([n, axis].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), sub(&[("reacts in {}", "latency")]))
        }
    }
}

/// The card of a build, by its properties ([`Props`]): its curated card's lines when it has one
/// (`Build::card`, crates/site/src/cards.rs), else the built-in ones.
pub fn props(b: &Build) -> Props {
    let mut p = built_in_props(b);
    if !b.curated.sub.is_empty() {
        p.sub = b.curated.sub.join(" · ");
    }
    p
}

/// The built-in card of a build ([`props`] without its curated lines).
fn built_in_props(b: &Build) -> Props {
    let flags = |list: &[&str]| -> Vec<String> { list.iter().filter(|f| b.flags.iter().any(|x| x == *f)).map(|f| f.to_string()).collect() };
    let dims = b.dims.as_deref().map(|d| d.replace('x', "×")).unwrap_or_default();
    match b.kind.as_str() {
        "door" => {
            let mut chips: Vec<String> = Vec::new();
            if let Some(d) = &b.door {
                if let Some(l) = d.leads.first() {
                    chips.push(door_lead_words(l, &d.seamless));
                }
                if !d.seamless.is_empty() {
                    chips.push(seamless_words(&d.seamless));
                }
            }
            for (f, words) in [("slime-free", "no slime"), ("flat", "1 block thick"), ("variant", "a bigger version")] {
                if b.flags.iter().any(|x| x == f) {
                    chips.push(words.into());
                }
            }
            // (working facing any way is what a builder assumes: said only when a door does not,
            // vjeux 2026-11-08 10:10)
            if b.door.as_ref().is_some_and(|d| !d.rotation.is_empty() && d.rotation != "safe") && !b.flags.iter().any(|x| x == "rotation-safe") {
                chips.push("works only facing one way".into());
            }
            let timing = b.door.as_ref().and_then(|d| Some(format!("opens in {} · closes in {}", crate::real_time(d.open?), crate::real_time(d.close?))));
            let mut sub = timing.unwrap_or_default();
            // the one door card says what sits behind it (critique: the lone card never said 11
            // records are on its page)
            if let Some(n) = b.door.as_ref().map(|d| d.records).filter(|n| *n > 1) {
                sub = format!("the best of {n} records (smallest, fastest, hidden, sculk…): pick one on its page · {sub}");
            }
            let cat = door_category(&b.group);
            // one block count (critique: "7×8×1 · 38 blocks" next to "56 blocks of space" read as
            // two counts): the blocks, in their space
            let title = if dims.is_empty() { blocks_word(b.blocks) } else { format!("{} in a {dims} space", blocks_word(b.blocks)) };
            // one short kind line (critique: "3×3 PISTON DOOR · STONE DOOR" on every card); a
            // fully seamless stone door says so (vjeux 23:44Z: what people call a piston door);
            // the opening's size first (a 2x2 door, designs/piston-door-2x2)
            let full = b.door.as_ref().is_some_and(|d| d.seamless == "FULL" || d.seamless == "SUPER");
            let n = crate::door_side(b);
            let kind = match b.group.as_str() {
                "solid" if full => format!("{n}×{n} seamless stone door"),
                "solid" => format!("{n}×{n} stone door"),
                "sculk" => format!("{n}×{n} sculk door"),
                "gravity" => format!("{n}×{n} sand or gravel door"),
                "entity" => format!("{n}×{n} entity door"),
                _ => if cat.is_empty() { format!("{n}×{n} piston door") } else { cat.to_string() },
            };
            Props { kind, title, sub, chips }
        }
        "clock" => {
            let c = b.clock.as_ref();
            let period = c.map(|c| format!("every {} ({})", if c.every() == 1 { "tick".to_string() } else { format!("{} ticks", c.every()) }, crate::real_time(c.every()))).unwrap_or_default();
            // an output that pulses several times evenly in a cycle: its page says the cycle; the
            // card's "2 pulses in each 8-tick cycle" under "every 4 ticks" added nothing (critique)
            let pulse = c.and_then(|c| match (c.pulse_every, c.edges) {
                (Some(_), _) => None,
                (None, 0) => None,
                (None, 1) if c.on > 0 => Some(format!("on for {} of {} ticks", c.on, c.period)),
                (None, 1) => None,
                (None, e) => Some(format!("{e} pulses a cycle")),
            });
            let made = format!("made of {}", human_blocks(&b.group));
            // the clocks of its period and mechanism with other blocks (folded under it: the
            // coordinator 11:33Z, vjeux: "The first 10 clocks look all the same to me?")
            let also = c.and_then(|c| match (c.swaps.is_empty(), c.variants.len()) {
                (_, 0) => None,
                (false, _) => Some(format!("also works with {}", swap_words(&c.swaps))),
                (true, 1) => Some("1 other block choice on its page".to_string()),
                (true, n) => Some(format!("{n} other block choices on its page")),
            });
            let sub = [Some(made), also, pulse].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let mut chips: Vec<String> = Vec::new();
            for (f, words) in [("flat", "1 block thick"), ("slime-free", "no slime"), ("self-starting", "starts by itself"), ("extendable", "comes in every size"), ("composite", "several clocks in one"), ("variant", "a bigger version")] {
                if b.flags.iter().any(|x| x == f) {
                    chips.push(words.into());
                }
            }
            // what a chunk reload does, where it matters (clockinfra's reload; the coordinator)
            if let Some(chip) = c.and_then(|c| reload_chip(&c.reload)) {
                chips.push(chip.into());
            }
            // no spot to take its output into a circuit (the judge's `usable`: a warning on its
            // own card, never summed up over a list)
            if c.is_some_and(|c| c.usable.starts_with("no")) {
                chips.push(HARD_TO_WIRE.into());
            }
            // a lamp on its output stays lit (critique a417977c3; said by its cause, not as "too
            // fast": critique 41f1ec62a): on the card too
            if let Some(c) = c.filter(|c| !c.lamp.is_empty()) {
                chips.push(if c.lamp == "misses pulses" { "a lamp on it misses pulses".into() } else { "a lamp on it stays lit".into() });
            }
            // a longer version of another clock says so first (vjeux 01:24Z: "extended n = k"; in
            // words, 2026-11-08: no "n")
            if b.flags.iter().any(|f| f == "extended") && c.and_then(|c| c.n).is_some() {
                chips.insert(0, "a longer version".to_string());
            }
            // the one clock card: the periods its page's table covers (vjeux 21:47Z)
            if let Some(range) = c.map(|c| c.kind_range.clone()).filter(|r| !r.is_empty()) {
                let mut parts = range.split(':');
                let span = parts.next().unwrap_or("");
                let tested = parts.next().unwrap_or("");
                let archived = parts.next().unwrap_or("");
                let (lo, hi) = span.split_once('-').unwrap_or((span, span));
                let hi_words = hi.parse::<u32>().map(crate::real_time).unwrap_or_else(|_| format!("{hi} ticks"));
                // (every clock of each period, the longer versions included: vjeux 00:43Z; the
                // periods only longer versions reach said apart, and what pictures the card:
                // critique a9f92b9c7; nothing of how they were checked, vjeux 2026-11-08 10:10)
                let only_ext = tested.parse::<u32>().ok().zip(archived.parse::<u32>().ok()).map(|(t, a)| t.saturating_sub(a)).filter(|n| *n > 0);
                return Props {
                    kind: "Clock".into(),
                    title: format!("every clock that works, for each of {tested} periods from {lo} ticks to {hi_words}"),
                    sub: format!(
                        "{}pictured: the smallest that keeps going after a chunk reload, {period}, {}",
                        only_ext.map(|n| format!("{n} of the periods reached only by longer versions of another clock · ")).unwrap_or_default(),
                        blocks_word(b.blocks)
                    ),
                    chips: Vec::new(),
                };
            }
            Props { kind: "Clock".into(), title: format!("{period} · {}", blocks_word(b.blocks)), sub, chips }
        }
        _ => match &b.circuit {
            Some(c) => {
                let (main, sub) = circuit_props(c);
                let k0 = c.kind.split_whitespace().next().unwrap_or("");
                // a sand elevator lifts its block up (its own section: critique)
                let main = if b.group == "Sand elevator" { format!("{} up", main.replace("reach ", "lifts ")) } else { main };
                let mut chips: Vec<String> = Vec::new();
                if !c.span.is_empty() {
                    chips.push(format!("{} versions", c.span.split(", ").last().unwrap_or("").trim_end_matches(" values").trim_end_matches(" versions")));
                }
                // what "faster" means for its kind (critique: "faster than smaller versions" on a
                // delay, whose speed metrics are the shortest pulse and gap it handles)
                let faster = match k0 {
                    "delay" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "off-delay" => "handles shorter pulses than smaller versions",
                    "tff" => "handles faster presses than smaller versions",
                    "shulker-loader" | "shulker-unloader" => "moves more items a second than smaller versions",
                    _ => "faster than smaller versions",
                };
                // (a TNT duper's lever is its on/off switch: the coordinator 14:08Z; a one-way
                // flying machine's starts it, 16:10Z; a docking machine's title says its own)
                let fly_two_way = k0 == "flying-machine" && (c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way"));
                let lever = if k0 == "tnt-duper" {
                    "on/off lever"
                } else if fly_two_way {
                    ""
                } else if k0 == "flying-machine" {
                    "start lever"
                } else {
                    "has its own lever"
                };
                for (f, words) in [("faster", faster), ("button", "has its own button"), ("lever", lever), ("mirrored", "mirror version too"), ("slime-free", "no slime"), ("flat", "1 block thick"), ("extendable", "comes in every size"), ("variant", "a bigger version"), ("layout", "another layout"), ("flip-safe", "safe to flip back any time")] {
                    if !words.is_empty() && b.flags.iter().any(|x| x == f) {
                        chips.push(words.into());
                    }
                }
                // the same build archived as another kind too
                for a in &c.also {
                    // (the same build is also another kind's best: critique, "also a one pulse
                    // when switched on" was garbled)
                    chips.push(format!("also the best “{}”", a.to_lowercase()));
                }
                // a design made to be copied side by side (its `tile:` header: a crop farm's)
                if let Some(step) = tile_step(c) {
                    chips.push(badge_words(&format!("tileable({step})")));
                }
                // the box's shape: 1 high (a flat 2-D design) or 1 wide (tiles side by side); a
                // BUD's box leaves out the block it watches, which its picture shows (critique: a
                // "lies flat" card with a fence standing on it)
                let watches = c.kind.starts_with("bud");
                if let Some(d) = &b.dims {
                    let n: Vec<u32> = d.split('x').filter_map(|v| v.trim().parse().ok()).collect();
                    if n.len() == 3 && !chips.iter().any(|c| c == "1 block thick") {
                        if n[1] == 1 && !c.ports_off_layers {
                            chips.push(if watches { "1 block high, plus the block it watches".into() } else { "lies flat (1 block high)".into() });
                        } else if n[0] == 1 || n[2] == 1 {
                            chips.push("1 block wide".into());
                        }
                    }
                }
                let badges: Vec<&str> = cm(c, "badges").split('+').filter(|s| !s.is_empty() && !c.lead.split(" + ").any(|l| l == badge_words(s))).collect();
                // (every badge: a card caps its chips after leaving out the shared ones, so it
                // never shows only "+4 more": critique)
                chips.extend(badges.iter().map(|s| badge_words(s)));
                // a chip the card's lead already says is not repeated ("no slime" in "reach 2 ·
                // no slime, 1 block wide"; "faster than smaller versions" under "fastest")
                if !c.lead.is_empty() {
                    chips.retain(|ch| !c.lead.contains(ch.as_str()) && !(ch.ends_with("than smaller versions") && c.lead.contains("fast")));
                }
                let mut sub = sub;
                if !c.direction.is_empty() && c.kind != "flying-machine" {
                    sub.insert(0, c.direction.clone());
                }
                // what it is made of (the blocks that change state) and its box tell siblings apart
                // (no design names in lists: vjeux 13:11Z; critique: two delay cards read the
                // same, "2 comparators" vs "a repeater"; fly cards: slime or honey); only the
                // design's own blocks, not the test's (a BUD's target block: critique, "made of oak
                // log, observer")
                let own: Vec<&str> = c.family.split('+').filter(|k| !k.is_empty() && (b.kinds.is_empty() || b.kinds.iter().any(|x| x == k))).collect();
                if !own.is_empty() && own.len() <= 3 {
                    sub.push(format!("made of {}", human_blocks(&own.join("+"))));
                }
                if !dims.is_empty() {
                    // (a TNT duper standing up: the air under it is part of its space, said with
                    // its box: the coordinator 15:18Z)
                    let air = tnt_air(c);
                    sub.push(if air > 0 { format!("fits in {dims} + {air} block{} of air under it", if air == 1 { "" } else { "s" }) } else { format!("fits in {dims}") });
                }
                let title = if main.is_empty() {
                    blocks_word(b.blocks)
                } else if cart_line(c) {
                    // (a whole minecart line: no block count beside "over 128 blocks" of track,
                    // the coordinator's card; its blocks go on the second line)
                    sub.push(format!("{} with its track", blocks_word(b.blocks)));
                    main.clone()
                } else {
                    format!("{main} · {}", blocks_word(b.blocks))
                };
                // (a crop farm's title goes on with its harvest per block, iron and light; a super
                // smelter's with whether it rests: the reviewer's "… · 108 blocks · rests")
                let title = match title_tail(c) {
                    Some(w) => format!("{title} · {w}"),
                    None => title,
                };
                // a design with no number (a latch): its first property leads (critique: a card read
                // just "4 blocks")
                let (title, sub) = if main.is_empty() && !sub.is_empty() {
                    let mut sub = sub;
                    let first = sub.remove(0);
                    (format!("{first} · {}", blocks_word(b.blocks)), sub)
                } else {
                    (title, sub)
                };
                // why it is the one on the site leads its second line (critique: "cards don't say
                // why they're the best"; not a chip: the shared chips fold into a section's line)
                let sub = if c.claim.is_empty() { sub } else { std::iter::once(c.claim.clone()).chain(sub).collect() };
                // (the kind line: the catalog's group; a two-way flying machine is one "with docking
                // stations": vjeux 00:31Z)
                let two_way = c.kind.starts_with("flying-machine") && (c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way"));
                let kind = if two_way {
                    "Flying machine with docking stations".to_string()
                } else if c.kind.starts_with("flying-machine") {
                    // by engine shape (vjeux 00:31Z): stacked = tall, side by side = flat
                    if c.feature.starts_with("carries") { "Tall flying machine".to_string() } else { "Flat flying machine".to_string() }
                } else if c.kind.starts_with("adder") {
                    // (which adder: every one is `adder 1`)
                    match adder_variant(c) {
                        "full" => "Full adder".to_string(),
                        "slice" => "Adder slice".to_string(),
                        _ => "Half adder".to_string(),
                    }
                } else if decimal_display(c) {
                    // (it shows its own digit on its lamp face: a display, not only a decoder)
                    "7-segment display".to_string()
                } else if b.group.is_empty() {
                    kind_words(&c.kind)
                } else {
                    b.group.clone()
                };
                Props { kind, title, sub: sub.join(" · "), chips }
            }
            None => Props { kind: b.group.clone(), title: blocks_word(b.blocks), sub: String::new(), chips: flags(&["slime-free", "flat"]) },
        },
    }
}

/// A result card for a build: its kind (small, above), its properties as the headline, its name
/// small at the bottom (and in the tooltip). `prefix` is the path from the page to the site root
/// (`""`, `"../"`).
pub fn build_card(b: &Build, prefix: &str) -> String {
    build_card_opts(b, prefix, true, &[])
}

/// A card without what every card around it says (critique C43): no kind line under a heading
/// that names the kind, and none of the chips shared by every card in view.
pub fn build_card_opts(b: &Build, prefix: &str, show_kind: bool, shared: &[String]) -> String {
    let kind = if show_kind { Some(props(b).kind) } else { None };
    card_inner(b, prefix, kind, shared)
}

/// A card on the index's tab of its own kind: the kind line without the kind's name every card
/// there carries (a door keeps its category, a clock drops it; critique: six cards in a row read
/// "3×3 PISTON DOOR · STONE DOOR"), and without the chips most cards share (said once above).
pub fn build_card_tab(b: &Build, prefix: &str, tab: &str, shared: &[String]) -> String {
    card_inner(b, prefix, tab_kind(b, tab), shared)
}

/// The kind line of a card on the index's tab `tab` ([`build_card_tab`]).
pub fn tab_kind(b: &Build, tab: &str) -> Option<String> {
    let full = props(b).kind;
    if tab != b.kind {
        Some(full)
    } else {
        match b.kind.as_str() {
            "door" => Some(full.trim_start_matches("3×3 ").to_string()),
            "clock" => None,
            _ => Some(full),
        }
    }
}

fn card_inner(b: &Build, prefix: &str, kind: Option<String>, shared: &[String]) -> String {
    card_inner_with(b, prefix, kind, shared, false)
}

/// A clock's card under its period's heading on the Clocks tab (vjeux 01:24Z: "a card grouped by
/// timing"): the heading says the period, so the card leads with its blocks and parts (critique:
/// every card under "Every 4 ticks" repeated "every 4 ticks (0.2 s)").
pub fn clock_card_in_period(b: &Build, prefix: &str, shared: &[String]) -> String {
    card_inner_with(b, prefix, None, shared, b.clock.is_some())
}

/// The index's opening line, per tab (critique: the Clocks tab's intro spoke of doors and
/// circuits, and said in one breath that every design was tested in the game and that some were
/// simulated).
pub fn lede(site: &Site, kind: &str) -> String {
    let [doors, clocks, kinds] = site.holds();
    let periods = site.clock_periods();
    let s = |n: usize| if n == 1 { "" } else { "s" };
    // the 3x3 doors' records, and the 2x2 doors' (their own boards: designs/piston-door-2x2)
    let small = site.builds.iter().filter(|b| b.door.as_ref().is_some_and(|d| d.side() == 2)).count();
    let big = doors - small;
    let mut door_part = if big == 1 { "the best 3×3 piston door".to_string() } else { format!("the best 3×3 piston door for each of {big} records") };
    if small > 0 {
        door_part.push_str(&if small == 1 { " and the best 2×2 piston door".to_string() } else { format!(" and the best 2×2 piston door for each of {small} records") });
    }
    let clock_part = format!("{clocks} clock{} for {periods} period{}", s(clocks), s(periods));
    match kind {
        // (no test claims in the opening lines: vjeux 2026-11-08 10:10, "The user doesn't care
        // that you tested")
        "door" if small > 0 => format!("{}{}: each card's page lists every record of its size.", door_part[..1].to_uppercase(), &door_part[1..]),
        "door" => format!("{}{}: the card is the seamless one, its page lists every record.", door_part[..1].to_uppercase(), &door_part[1..]),
        // (short: on a phone the tab's first screen showed one card under it, webrender's profile)
        "clock" => format!("{clock_part}, smallest first."),
        "circuit" => format!("The best design of each of {kinds} circuit kind{}.", s(kinds)),
        _ => {
            let mut parts = vec![door_part];
            if clocks > 0 {
                parts.push(format!("every clock that works ({clock_part})"));
            }
            if kinds > 0 {
                parts.push(format!("the best design of {kinds} circuit kind{}", s(kinds)));
            }
            let list = match parts.len() {
                1 => parts[0].clone(),
                n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
            };
            format!("Redstone builds you can download: {list}.")
        }
    }
}

fn card_inner_with(b: &Build, prefix: &str, kind: Option<String>, shared: &[String], in_period: bool) -> String {
    let mut p = props(b);
    let label = format!("{}: {}", p.kind, p.title);
    if in_period {
        p.title = blocks_word(b.blocks);
    }
    // a super smelter's kind card keeps its pictured size's title (group_range: none) and says the
    // sizes its page lists at the end of its second line (only on the card: its page lists them)
    if let Some((lo, hi)) = b.circuit.as_ref().filter(|c| c.kind.starts_with("smelter") && double_chest_minutes(c).is_some() && c.tunable.starts_with("kind:")).and_then(|c| span_bounds(&c.span)) {
        p.sub = if p.sub.is_empty() { format!("{lo} to {hi} furnaces on its page") } else { format!("{} · {lo} to {hi} furnaces on its page", p.sub) };
    }
    // a group's card says the whole range, not its smallest member's value (critique: "counter
    // 17 · 9 versions" does 17 to 576); the design's own page keeps its own value
    if let Some(r) = b.circuit.as_ref().and_then(group_range) {
        let size = b.circuit.as_ref().map(|c| c.span_blocks.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| blocks_word(b.blocks));
        // (what the group adds over the smaller ones of its kind, when it says: "adds 18, 34 ticks")
        let lead = b.circuit.as_ref().filter(|c| !c.lead.is_empty() && !c.kind.starts_with("shulker-") && !c.kind.starts_with("piston-extender")).map(|c| format!(" · {}", c.lead)).unwrap_or_default();
        p.title = format!("{r}{lead} · {size}");
        // a kind's card: its facts are the pictured size's, so they say which (critique: "reach 2
        // to 12" beside the reach-2 design's "out in 9 ticks, fits in 5×3×1")
        if let Some(c) = b.circuit.as_ref().filter(|c| c.tunable.starts_with("kind:") && !p.sub.is_empty()) {
            let which = if c.kind.starts_with("flying-machine") { c.variant_label.clone() } else { value_words(&c.kind, &c.axis) };
            if !which.is_empty() {
                // (the kind's claim first, then the pictured size's facts)
                let rest = if c.claim.is_empty() { p.sub.clone() } else { p.sub.strip_prefix(&format!("{} · ", c.claim)).map(String::from).unwrap_or_else(|| p.sub.clone()) };
                p.sub = if c.claim.is_empty() || rest == p.sub { format!("pictured, {which}: {rest}") } else { format!("{} · pictured, {which}: {rest}", c.claim) };
            }
        }
    }
    let mut chips: Vec<String> = p
        .chips
        .iter()
        .filter(|x| !shared.contains(x))
        .cloned()
        // a chip most cards around share, missing here, is said (critique: two cards read the same),
        // unless the card's lead says it
        .chain(shared.iter().filter(|s| !p.chips.contains(s) && !b.circuit.as_ref().is_some_and(|c| c.lead.contains(s.as_str()))).filter_map(|s| unlike_most(s)))
        .collect();
    // at most 5 chips on a card, then how many more its page lists
    if chips.len() > 6 {
        let more = chips.len() - 5;
        chips.truncate(5);
        chips.push(format!("+{more} more"));
    }
    let chips: String = chips.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect();
    format!(
        "<a class=\"card\" href=\"{}\" aria-label=\"{}\">{}<div class=\"meta\">{}<div class=\"title\">{}</div>{}<div class=\"badges\">{chips}</div></div></a>",
        esc(&format!("{prefix}{}", b.page)),
        esc(&label),
        // (a card's own still, a crop farm's field grown; else the view's, a display's lit still)
        thumb(prefix, card_src(b), b.trace.as_deref(), b.still.as_deref(), b.picture.as_deref(), &b.kind, &headline(b), &fallback_line(b)),
        if let Some(k) = kind.filter(|k| !k.is_empty()) { format!("<div class=\"kind\">{}</div>", esc(&k)) } else { String::new() },
        esc(&keep_units(&p.title)),
        if p.sub.is_empty() { String::new() } else { format!("<div class=\"sub\">{}</div>", esc(&keep_units(&p.sub))) }
    )
}

/// The chips every card of a list shares (to leave out: they tell nothing there).
pub fn shared_chips(list: &[&Build]) -> Vec<String> {
    if list.len() < 2 {
        return Vec::new();
    }
    let first = props(list[0]).chips;
    first.into_iter().filter(|c| !warning_chip(c) && list.iter().skip(1).all(|b| props(b).chips.contains(c))).collect()
}

/// A chip that warns (a lamp kept lit, a reload, no spot to wire it): it stays on each card it
/// concerns, never summed up over a list.
fn warning_chip(c: &str) -> bool {
    c.starts_with("a lamp on it") || c.starts_with("stops") || c.contains("chunk") || c == HARD_TO_WIRE
}

/// The chips most cards of a list share (two thirds or more, at least 3 cards): said once above
/// the list instead of on every card (critique C43: "has its own lever" on 7 of 8 cards); the
/// cards without one say so ([`unlike_most`]), so two cards never read the same because one chip
/// moved up (critique: rider7-flat vs the slime-free sf7-1).
pub fn common_chips(list: &[&Build]) -> Vec<String> {
    if list.len() < 3 {
        return shared_chips(list);
    }
    // (a warning stays on each card it concerns: summed up over the first cards it read as a
    // property of all, "4177 results. Most keep a lamp on them lit" when 668 of them do)
    let warning = warning_chip;
    let all: Vec<Vec<String>> = list.iter().map(|b| props(b).chips).collect();
    let mut seen: Vec<String> = Vec::new();
    for chips in &all {
        for c in chips {
            if !warning(c) && !seen.contains(c) && all.iter().filter(|x| x.contains(c)).count() * 3 >= list.len() * 2 {
                seen.push(c.clone());
            }
        }
    }
    seen
}

/// A common chip a card lacks, in words (`no slime` -> `uses slime`); a chip without its own
/// negation says nothing (critique: "not works facing any way").
pub fn unlike_most(chip: &str) -> Option<String> {
    Some(
        match chip {
            "no slime" => "uses slime",
            "has its own lever" => "no lever of its own",
            "on/off lever" => "no on/off switch",
            "start lever" => "no start lever",
            "has its own button" => "no button of its own",
            "1 block wide" => "wider than 1 block",
            "1 block thick" => "thicker than 1 block",
            "lies flat (1 block high)" => "taller than 1 block",
            "comes in every size" => "one size only",
            "works facing any way" => "works only facing one way",
            "starts by itself" => "needs a kick to start",
            "a lamp on it stays lit" => "a lamp on it blinks",
            _ => return None,
        }
        .to_string(),
    )
}

/// A chip as what most of a list do (`has its own lever` -> `have their own lever`), for the line
/// above the list.
pub fn chip_plural(chip: &str) -> String {
    if let Some(rest) = chip.strip_prefix("has its own ") {
        return format!("have their own {rest}");
    }
    if chip.starts_with("no ") {
        return format!("use {chip}");
    }
    // (critique: "Most are machine hidden from both sides")
    if let Some(rest) = chip.strip_prefix("machine hidden ") {
        return format!("hide their machine {rest}");
    }
    if let Some(rest) = chip.strip_prefix("also a ") {
        return format!("are also {rest}s");
    }
    match chip {
        "comes in every size" => "come in every size".to_string(),
        "lies flat (1 block high)" => "lie flat (1 block high)".to_string(),
        "works facing any way" => "work facing any way".to_string(),
        "starts by itself" => "start by themselves".to_string(),
        "carries extra blocks" => "carry extra blocks".to_string(),
        "mirror version too" => "come in a mirror version too".to_string(),
        "1 block high, plus the block it watches" => "are 1 block high, plus the block they watch".to_string(),
        "16-stack items too" => "handle 16-stack items too".to_string(),
        "always on, no lever" => "run on their own (no lever)".to_string(),
        "on/off lever" => "have an on/off lever".to_string(),
        "start lever" => "start with their own lever".to_string(),
        "safe to flip back any time" => "are safe to flip back any time".to_string(),
        "unstackable items too" => "handle unstackable items too".to_string(),
        "packs mixed items" => "pack mixed items".to_string(),
        "packs one kind, passes the rest" => "pack one kind and pass the rest".to_string(),
        "tiles side by side" => "tile side by side".to_string(),
        "restarts by itself" => "restart by themselves".to_string(),
        "waits out a box shortage" => "wait out a box shortage".to_string(),
        "keeps box colours and names" => "keep box colours and names".to_string(),
        "sends out a part-full box" => "send out a part-full box".to_string(),
        "tops up part-full boxes" => "top up part-full boxes".to_string(),
        "drops nothing on the ground" => "drop nothing on the ground".to_string(),
        "loads and unloads" => "load and unload".to_string(),
        "empties any box" => "empty any box".to_string(),
        "a lamp on it stays lit" => "keep a lamp on them lit".to_string(),
        "a lamp on it misses pulses" => "make a lamp on them miss pulses".to_string(),
        other => format!("are {other}"),
    }
}

/// A number keeps its unit on the same line (critique C42: "2 / blocks" wrapped): the space after
/// a digit becomes a no-break space.
pub fn keep_units(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_digit = false;
    for ch in s.chars() {
        if ch == ' ' && prev_digit {
            out.push('\u{a0}');
        } else if ch == '–' && prev_digit {
            // a range stays on one line: "2–511 ticks" (critique: "cuts pulses to 2– / 511")
            out.push_str("\u{2060}–\u{2060}");
        } else {
            out.push(ch);
        }
        prev_digit = ch.is_ascii_digit();
    }
    out
}

/// The anchor of a member's row on its family card's page: `n<k>` for the card's first family,
/// `n<k>-<family>` for the others.
pub fn member_anchor(m: &Member, site: &Site) -> String {
    let first = site.cards.iter().find(|c| c.id == m.card).and_then(|c| c.families.first()).map(|f| f.id.as_str());
    if first.is_none_or(|f| f == m.family) { format!("n{}", m.n) } else { format!("n{}-{}", m.n, m.family) }
}

/// The page a member links to: its archived build, else its family's card at its row.
pub fn member_href(m: &Member, site: &Site) -> String {
    // (no family pages: vjeux 00:33Z; a clock member is listed under its period on the clock
    // card's page, 00:43Z)
    let listed = || {
        site.builds
            .iter()
            .find(|b| b.clock.as_ref().is_some_and(|c| !c.kind_range.is_empty()))
            .map(|b| format!("{}#p{}", b.page, m.period))
            .unwrap_or_else(|| format!("families/{}.html#{}", m.card, member_anchor(m, site)))
    };
    match &m.build {
        Some(id) => site.builds.iter().find(|b| &b.id == id).map(|b| b.page.clone()).unwrap_or_else(listed),
        None => listed(),
    }
}

/// A result card for an extendable family's member: its value and size lead (`every 48 ticks
/// (2.4 s) · 24 blocks`; a circuit family's member: its axis value); no family id or n (vjeux:
/// no ids in lists).
pub fn member_card(m: &Member, site: &Site, prefix: &str) -> String {
    let base = site.builds.iter().find(|b| !m.base.is_empty() && b.id == m.base).or_else(|| {
        site.cards
            .iter()
            .find(|c| c.id == m.card)
            .and_then(|c| c.families.iter().find(|f| f.id == m.family))
            .and_then(|f| site.builds.iter().find(|b| b.id == f.build))
    });
    let group = base.map(|b| if b.kind == "clock" { format!("made of {}", human_blocks(&b.group)) } else { b.group.clone() }).unwrap_or_default();
    let status = format!("<span class=\"badge st-{}\">{}</span>", esc(&m.status), esc(status_label(&m.status)));
    let circuit = !m.axis.is_empty();
    let (kind, big, value) = if circuit {
        ("circuit", m.axis.clone(), axis_shown("", &m.axis).unwrap_or_else(|| m.axis.clone()))
    } else {
        ("clock", format!("every {} ticks", m.period), format!("every {} ticks ({})", m.period, crate::real_time(m.period)))
    };
    let kind_line = base.and_then(|b| b.circuit.as_ref().map(|c| kind_words(&c.kind))).unwrap_or_else(|| "Clock".into());
    // (each number kept with its unit: critique, "2 / blocks" wrapped)
    format!(
        "<a class=\"card member\" href=\"{}\" aria-label=\"{}: {} · {}\">{}<div class=\"meta\"><div class=\"kind\">{}</div><div class=\"title\">{} · {}</div><div class=\"sub\">{}</div><div class=\"badges\"><span class=\"badge\">a longer version</span>{status}</div></div></a>",
        esc(&format!("{prefix}{}", member_href(m, site))),
        esc(&kind_line),
        esc(&value),
        esc(&blocks_word(m.blocks)),
        thumb(prefix, &m.design, None, None, m.picture.as_deref(), kind, &big, ""),
        esc(&kind_line),
        esc(&keep_units(&value)),
        esc(&keep_units(&blocks_word(m.blocks))),
        esc(&group)
    )
}

/// The card of a query hit.
pub fn hit_card(site: &Site, h: Hit, prefix: &str) -> String {
    match h {
        Hit::Build(i) => site.builds.get(i).map(|b| build_card(b, prefix)).unwrap_or_default(),
        Hit::Member(i) => site.members.get(i).map(|m| member_card(m, site, prefix)).unwrap_or_default(),
    }
}

/// [`hit_card`] on the index's tab `tab` (`door`, `clock`, `circuit`, or empty for every kind),
/// without the chips in `shared` ([`build_card_tab`]).
pub fn hit_card_tab(site: &Site, h: Hit, prefix: &str, tab: &str, shared: &[String]) -> String {
    let mut prev = None;
    hit_card_run(site, h, prefix, tab, shared, &mut prev)
}

/// [`hit_card_tab`] in a run of cards: on the every-kind list (the home page) a kind line the
/// previous card already showed is left out, so it reads like a heading over its run (critique:
/// the home repeated "3×3 STONE DOOR" on every card).
pub fn hit_card_run(site: &Site, h: Hit, prefix: &str, tab: &str, shared: &[String], prev: &mut Option<String>) -> String {
    match h {
        Hit::Build(i) => site
            .builds
            .get(i)
            .map(|b| {
                let kind = tab_kind(b, tab);
                let shown = if tab.is_empty() && kind.is_some() && kind == *prev { None } else { kind.clone() };
                *prev = kind;
                card_inner(b, prefix, shown, shared)
            })
            .unwrap_or_default(),
        Hit::Member(i) => {
            *prev = None;
            site.members.get(i).map(|m| member_card(m, site, prefix)).unwrap_or_default()
        }
    }
}

/// The chips most of the first hits share (said once above the index's cards), from the first
/// 48 build hits (at least 3).
pub fn hits_common_chips(site: &Site, hits: &[Hit]) -> Vec<String> {
    let builds: Vec<&Build> = hits.iter().filter_map(|h| match h {
        Hit::Build(i) => site.builds.get(*i),
        Hit::Member(_) => None,
    }).take(48).collect();
    if builds.len() < 3 {
        return Vec::new();
    }
    common_chips(&builds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(esc("a<b>&\"c'"), "a&lt;b&gt;&amp;&quot;c&#39;");
    }

    #[test]
    fn card_markup() {
        let b = Build {
            id: "clocks/x".into(),
            kind: "clock".into(),
            group: "observer+redstone_lamp".into(),
            name: "x<1>".into(),
            page: "clocks/x.html".into(),
            design: "designs/clocks/x.txt".into(),
            blocks: 6,
            clock: Some(crate::ClockInfo { period: 26, ..Default::default() }),
            flags: vec!["self-starting".into()],
            ..Default::default()
        };
        let h = build_card(&b, "../");
        assert!(h.starts_with("<a class=\"card\" href=\"../clocks/x.html\" aria-label=\"Clock: every 26 ticks (1.3 s) · 6 blocks\">"), "{h}");
        assert!(h.contains("data-mcview=\"thumb\" data-src=\"../designs/clocks/x.txt\" data-still=\"0\" data-kind=\"clock\""), "{h}");
        // stills only on the grid (vjeux 21:29Z): the build as placed, tick 0, framed by the box
        // the build's still search found; no still found: tick 0 too (no search in the browser)
        let still = build_card(&Build { still: Some("5428 box=0,0,-16,9,1,0".into()), ..b.clone() }, "../");
        assert!(still.contains("data-src=\"../designs/clocks/x.txt\" data-still=\"0 box=0,0,-16,9,1,0\" data-kind=\"clock\""), "{still}");
        // the properties lead; no design name anywhere on the card (vjeux 13:11Z); numbers keep
        // their units (no-break spaces)
        assert!(h.contains("<div class=\"title\">every 26\u{a0}ticks (1.3\u{a0}s) · 6\u{a0}blocks</div>"), "{h}");
        assert!(h.contains("<div class=\"sub\">made of observer, redstone lamp</div>"), "{h}");
        assert!(!h.contains("x&lt;1&gt;") && !h.contains("class=\"name\""), "{h}");
        assert!(h.contains("<span class=\"badge\">starts by itself</span>"), "{h}");
    }

    #[test]
    fn circuit_cards_lead_with_properties() {
        let ext = Build {
            kind: "circuit".into(),
            name: "pext-n3-flat-cascade19".into(),
            blocks: 19,
            dims: Some("9x6x1".into()),
            kinds: vec!["sticky_piston".into()],
            flags: vec!["button".into(), "flat".into()],
            circuit: Some(crate::Circuit { kind: "piston-extender 3".into(), axis: "N = 3".into(), metrics: vec![("N".into(), "3".into()), ("extend".into(), "27 gt".into()), ("retract".into(), "56 gt (from the press)".into())], ..Default::default() }),
            ..Default::default()
        };
        let p = props(&ext);
        assert_eq!(p.kind, "Piston extender");
        assert_eq!(p.title, "reach 3 · 19 blocks");
        assert_eq!(p.sub, "out in 27 ticks (1.35 s), back in 56 ticks (2.8 s) · fits in 9×6×1");
        assert_eq!(p.chips, vec!["has its own button".to_string(), "1 block thick".to_string()]);
        let div = Build { kind: "circuit".into(), blocks: 13, circuit: Some(crate::Circuit { kind: "divider 3".into(), metrics: vec![("output".into(), "square".into()), ("latency".into(), "37 gt".into())], ..Default::default() }), ..Default::default() };
        assert_eq!((props(&div).kind.as_str(), props(&div).title.as_str()), ("Clock slower", "slows by 3 · even on/off · 13 blocks"));
        let mono = Build { kind: "circuit".into(), blocks: 21, circuit: Some(crate::Circuit { kind: "monostable retriggerable".into(), metrics: vec![("L".into(), "32 gt".into())], ..Default::default() }), ..Default::default() };
        assert_eq!((props(&mono).kind.as_str(), props(&mono).title.as_str()), ("One-shot timer", "on for 32 ticks (1.6 s) · restarts on every press · 21 blocks"));
        assert_eq!(kind_words("piston-extender 5"), "Piston extender");
        assert_eq!(kind_words("gate and"), "Logic gate");
        assert_eq!(kind_words("full-adder 4"), "Full adder");
        assert_eq!(gate_label("nimply"), "A AND (NOT B)");
        assert_eq!(human_blocks("observer+redstone_lamp"), "observer, redstone lamp");
        // an empty search offers the nearest kinds
        assert!(nearest_searches("timers").contains("one-shot timer") && nearest_searches("timers").contains("stay-on timer"));
        assert!(nearest_searches("xyzzy").contains("toggle switch"));
    }
}
