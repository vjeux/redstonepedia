//! Plain player language for every build page (vjeux 13:06Z: "I have no clue what a monostable
//! retriggerable system means. Can you make sure you explain all the builds so someone can
//! understand them"): what the build does in one sentence with its own numbers, why you would
//! build it (a concrete use), and how to try it in the 3D view. No jargon, no judge metrics, no
//! raw ids: those live in the page's collapsed details.

use site_model::html::esc;
use site_model::{Build, Circuit, real_time};

fn m<'a>(c: &'a Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// `32 gt` -> `32 ticks (1.6 s)`, `1 gt` -> `1 tick (0.05 s)`, `4-8 gt` -> `4 to 8 ticks`; a value
/// with words after the unit keeps them (`28 gt (from the press)` -> `28 ticks (1.4 s)`).
pub fn ticks(v: &str) -> String {
    let v = v.trim();
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    if let Ok(x) = n.parse::<u32>() {
        return format!("{x} tick{} ({})", if x == 1 { "" } else { "s" }, real_time(x));
    }
    match n.split_once('-') {
        Some((a, b)) => format!("{a} to {b} ticks"),
        None => v.to_string(),
    }
}

/// `32 gt` -> `32 ticks` (no seconds: a card's headline).
pub fn ticks_short(v: &str) -> String {
    let v = v.trim();
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    match n.parse::<u32>() {
        Ok(1) => "1 tick".to_string(),
        Ok(x) => format!("{x} ticks"),
        Err(_) => format!("{} ticks", n.replace('-', " to ")),
    }
}

fn number(kind: &str) -> u32 {
    kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or("")
}

pub fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A logic function in plain words: when the output is on.
fn gate_words(f: &str) -> &'static str {
    match f {
        "not" => "The output is on when the input is off, and off when the input is on",
        "buffer" => "The output copies the input",
        "and" => "The output is on only when both inputs, A and B, are on",
        "or" => "The output is on when A or B (or both) is on",
        "xor" => "The output is on when exactly one of A and B is on, not both",
        "xnor" => "The output is on when A and B are the same: both on or both off",
        "nand" => "The output is on unless both A and B are on",
        "nor" => "The output is on only when both A and B are off",
        "imply" => "The output is off only when A is on and B is off",
        "nimply" => "The output is on only when A is on and B is off",
        "imply-ba" => "The output is off only when B is on and A is off",
        "nimply-ba" => "The output is on only when B is on and A is off",
        "not-a" => "The output is on when A is off (B does not matter)",
        "not-b" => "The output is on when B is off (A does not matter)",
        _ => "The output is a fixed combination of the inputs (see the truth table in the details)",
    }
}

/// What it does, in one sentence with its own numbers.
pub fn one_liner(c: &Circuit) -> String {
    let n = number(&c.kind);
    let l = ticks(m(c, "L"));
    match base(&c.kind) {
        // clk-obs's 0-9 display (the coordinator 2026-10-22: one comparator reads a chest, any
        // container; the digit updates within 1.6 s; no flicker)
        "seven-segment" if site_model::html::decimal_display(c) => {
            let within = m(c, "latency").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!(" A new digit shows within {}", site_model::real_time(g))).unwrap_or_default();
            let flicker = if m(c, "on-blips").split_whitespace().next() == Some("0") { ", with no wrong lamp flickering on between digits." } else if within.is_empty() { "" } else { "." };
            format!("One comparator reads a chest (or any container) beside it, and its 3×5 lamp face shows the signal, 0 to 9, as a digit: the fuller the container, the higher the digit.{within}{flicker}")
        }
        "rising-edge" => format!("When the input turns on, the output gives one short pulse of {l}; turning the input off does nothing."),
        "falling-edge" => format!("When the input turns off, the output gives one short pulse of {l}; turning the input on does nothing."),
        "dual-edge" => format!("Every time the input changes, on or off, the output gives one short pulse of {l}."),
        "pulse-limiter" => format!("However long the input stays on, the output pulse lasts at most {l}."),
        "pulse-extender" => format!("Even the shortest input pulse comes out at least {l} long."),
        "tff" => "Press once and the output turns on; press again and it turns off.".to_string(),
        "tff-reset" => "Each press flips the output on or off; a press on R turns it off.".to_string(),
        "delay" => {
            let n = if m(c, "N").is_empty() { c.axis.trim_start_matches("N = ").trim_end_matches(" gt").to_string() } else { m(c, "N").trim_end_matches(" gt").to_string() };
            // a 0-tick delay passes the signal on in the same tick: a signal carrier, not a delay
            // (critique: "0-tick delay" on a page promising "a moment later")
            if n == "0" {
                "Carries a signal to its output in the same game tick, with no delay at all: a wire that does not slow the signal down.".to_string()
            } else {
                format!("Whatever goes into the input comes out {} later, every pulse the same length.", ticks(&format!("{n} gt")))
            }
        }
        "gate" => {
            let f = if m(c, "function").is_empty() { c.kind.split_whitespace().nth(1).unwrap_or("") } else { m(c, "function") };
            let instant = m(c, "class") == "instant" || m(c, "latency") == "0 gt";
            // two-input gates introduce their inputs (critique: "A and B are never introduced")
            let two = !matches!(f, "not" | "buffer");
            format!("{}{}{}.", if two { "It has two inputs, A and B. " } else { "" }, gate_words(f), if instant { ", in the very same tick (an instant gate)" } else { "" })
        }
        "rs-latch" => "Press the set button (S) and the output turns on and stays on; press the reset button (R) and it turns off and stays off.".to_string(),
        "d-latch" => "Two inputs: while the enable input (E) is on, the output copies the data input (D); when E turns off, the output keeps the last value.".to_string(),
        "dff" => "At each clock pulse the output takes the value of D and keeps it until the next clock pulse.".to_string(),
        "dff-enable" => "At each clock pulse, if E is on, the output takes the value of D; otherwise it keeps its value.".to_string(),
        "shift" => format!("Each clock pulse moves every stored bit one step along: what goes in comes out {n} clock pulses later."),
        "counter" => {
            let style = m(c, "output");
            if style.starts_with("toggles") {
                format!("Every {} input pulse the output flips: on for {n} pulses, off for the next {n}.", ordinal(n))
            } else if style.starts_with("pulses") {
                format!("The output gives one pulse for every {n} input pulses.")
            } else {
                match c.counter_on {
                    Some((k, mm)) if mm == n && k > 0 && k < n => format!("The output's on/off pattern repeats every {n} input pulses: on for {k} of them, off for the other {}.", n - k),
                    _ => format!("The output's on/off pattern repeats every {n} input pulses."),
                }
            }
        }
        "piston-extender" => {
            let (e, r) = (ticks(m(c, "extend")), ticks(m(c, "retract")));
            format!(
                "Turn the input on and a block is pushed {n} blocks out; turn it off and the block comes back exactly where it was.{}",
                if e.is_empty() { String::new() } else { format!(" It goes out in {e} and comes back in {r}.") }
            )
        }
        // a piston bridge (pext's study): the gap, the walkway, how fast
        "piston-bridge" => {
            let (e, r) = (ticks(m(c, "extend")), ticks(m(c, "retract")));
            let walk = m(c, "walkway").trim_start_matches("minecraft:").replace('_', " ");
            let row = if walk.is_empty() { "a row of blocks".to_string() } else { format!("a row of {}{}", walk, if walk.ends_with(" block") { "s" } else { "" }) };
            format!(
                "Flip the lever on and {row} slides out of the wall across a {n}-block gap; flip it off and the row slides back flush into the wall, leaving the gap open.{}",
                if e.is_empty() { String::new() } else { format!(" It goes out in {e} and comes back in {r}.") }
            )
        }
        "flying-machine" => {
            let speed = m(c, "speed");
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            let per = speed.split_whitespace().next().and_then(|g| g.parse::<f64>().ok());
            let pace = match per {
                Some(g) if g > 0.0 => format!(", one block every {} ({} blocks a second)", ticks_short(&format!("{} gt", g as u32)), trim(20.0 / g)),
                _ => String::new(),
            };
            if two_way {
                // (its lever is a real input: the coordinator 00:45Z, said plainly)
                let lever = c.devices.iter().any(|d| d == "lever");
                let trip = speed.split("round trip ").nth(1).and_then(|s| s.split_whitespace().next()).map(|g| format!(", a round trip every {}", ticks_short(&format!("{g} gt")))).unwrap_or_default();
                let trip = if trip.is_empty() { m(c, "two-way").split("round trip ").nth(1).and_then(|s| s.split_whitespace().next()).map(|g| format!(", a round trip every {}", ticks_short(&format!("{g} gt")))).unwrap_or(pace) } else { trip };
                format!(
                    "Once started it shuttles between its two docking stations by itself{trip}.{}",
                    if lever { " Its lever is an on/off switch: off, the machine docks at the lever's station and stays there; on, it leaves at once." } else { "" }
                )
            } else {
                // (direction words: "flies sideways", not "flies horizontal": critique)
                let dir = match c.direction.as_str() {
                    "" => String::new(),
                    "horizontal" => " sideways".to_string(),
                    d => format!(" {d}"),
                };
                // (a tall machine's cargo: what it carries as built, and up to how many its
                // extend headers allow: vjeux 00:31Z, cargo is a property, not a kind)
                let carries = match (c.feature.is_empty(), c.extend_n) {
                    (false, Some((_, hi))) if c.feature.starts_with("carries") => format!(" It carries extra blocks stuck to it: {} as built, up to {hi}.", c.feature.trim_start_matches("carries ").split(" extra").next().unwrap_or("1")),
                    (false, _) => format!(" It {}.", c.feature),
                    _ => String::new(),
                };
                // (started by its own lever: the coordinator 16:10Z)
                if c.devices.iter().any(|d| d == "lever") {
                    format!("Flip its lever and it flies{dir} by itself{pace}, until something stops it.{carries}")
                } else {
                    format!("Once started it flies{dir} by itself{pace}, until something stops it.{carries}")
                }
            }
        }
        "shulker-loader" => format!("Takes items from a chest, puts them into empty shulker boxes and sends each full box out{}.", rate(m(c, "items/s"))),
        "shulker-unloader" => format!("Empties full shulker boxes into a chest and sends the empty boxes out{}.", rate(m(c, "items/s"))),
        "bud" => {
            let d: Vec<&str> = m(c, "detects").split_whitespace().collect();
            let words: Vec<&str> = d.iter().map(|k| match *k {
                "place" => "placed",
                "remove" => "removed",
                "state" => "changed",
                "shape" => "grows (like a crop)",
                "update" => "updated with nothing changing",
                other => other,
            }).collect();
            format!(
                "Fires its output when the block next to it (the cell marked TARGET in the 3D view) is {}, without any redstone signal reaching it: a block update detector (BUD).",
                if words.is_empty() { "changed".to_string() } else { list_words(&words, "or") }
            )
        }
        "tnt-duper" => format!(
            "Lights {}{}{} without ever using up its own TNT block.",
            if m(c, "rate").is_empty() { "TNT over and over".to_string() } else { site_model::html::tnt_rate_words(m(c, "rate")) },
            // (an on/off switch: the coordinator 14:08Z, every duper's pick has one)
            if c.devices.iter().any(|d| d == "lever") { " while its lever is on" } else { "" },
            // (the precise dupers: the coordinator 00:14Z and 01:33Z)
            if c.axis.contains("precise air burst") {
                ", and every TNT explodes at the same point in mid-air,"
            } else if c.axis.contains("precise drop") {
                ", and every TNT falls straight down and explodes at the same point under the machine,"
            } else {
                ""
            }
        ),
        // a duper lit once per trigger: a press of its button (tnt-button-17b), else the judge's
        // short pulse (a button holds 20 ticks or more and lights more, the needs line says)
        "tnt-duper-triggered" if c.devices.iter().any(|d| d == "button") => format!("Each press of its button lights {} TNT without using up the TNT block.", if m(c, "per pulse").is_empty() { "one" } else { m(c, "per pulse") }),
        "tnt-duper-triggered" => format!("Each short input pulse ({}) lights {} TNT without using up the TNT block.", ticks_short(&format!("{} gt", circuit::tnt::PULSE)), if m(c, "per pulse").is_empty() { "one" } else { m(c, "per pulse") }),
        // an item sorter: the kind it is set up for is the one in its filter hopper (`sorter
        // iron_ingot`)
        "sorter" => {
            let item = c.kind.split_whitespace().nth(1).map(|i| i.replace('_', " ")).unwrap_or_default();
            let set = if item.is_empty() { String::new() } else { format!(" (set up for {item}s; the items in its filter hopper choose the kind)") };
            // (the rate as the card writes it, "2.5": the opening lines then say it once)
            let rate = m(c, "items/s").trim_end_matches('0').trim_end_matches('.').to_string();
            format!("Takes every item of one kind out of a line of mixed items into its chest{set}; everything else goes on along the line{}.", if rate.is_empty() { String::new() } else { format!(", {rate} items a second") })
        }
        // an auto-smelter (study 17): its furnaces, how the items get there, how fast
        "smelter" => {
            let n: u32 = c.kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            // a super smelter timed on a double chest (sorter's `chest` run): the player's number
            // first, what to load, and whether it rests (the coordinator 2026-10-22)
            if let Some(mins) = site_model::html::double_chest_minutes(c) {
                let furnaces = site_model::html::furnaces_words(c);
                let rest = if m(c, "at rest").starts_with("yes") { " With the chest empty nothing moves: no clock runs on." } else { "" };
                let only = if site_model::html::smelter_blast(c) { " Blast furnaces smelt only ores, raw metals and metal tools and armour." } else { "" };
                return format!(
                    "Smelts a double chest of raw iron (3,456) in {} with its {furnaces}: dump the load in its input chest and the coal it needs plus one stack in its fuel chest (432 + 64 for a double chest), and every ingot ends up in its output chest.{rest}{only}",
                    site_model::html::minutes_words(mins).replace('\u{a0}', " ").replace(" min", " minutes")
                );
            }
            let rate = m(c, "items/min per furnace").trim_end_matches('0').trim_end_matches('.').to_string();
            let furnaces = if !m(c, "tileable").is_empty() { "the furnaces of its slices (build as many slices side by side as you need)".to_string() } else if n == 1 { "its furnace".to_string() } else { format!("its {n} furnaces") };
            format!("Shares the items of its input chest out to {furnaces}, feeds them coal from its fuel chest and collects every result in its output chest{}.", if rate.is_empty() { String::new() } else { format!(", {rate} items a minute per furnace") })
        }
        // an auto-brewer (study 21): its stands, the order of the ingredients, how fast
        "composter" => {
            let rate = m(c, "items composted/min").trim_end_matches('0').trim_end_matches('.').to_string();
            let how = if !m(c, "tileable").is_empty() { "one crop a slice: build a slice for every crop your farm sends (each slice's filter hopper holds its crop)".to_string() } else { "the crops its filters hold".to_string() };
            format!("Takes the crops out of a hopper line of mixed farm output and composts them, {how}; the bone meal drops into a chest under each composter and everything else runs on to the overflow chest at the end of the line{}.", if rate.is_empty() { String::new() } else { format!(", {rate} items composted a minute") })
        }
        "brewer" => {
            let n: u32 = c.kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            let rate = m(c, "potions/min per stand").trim_end_matches('0').trim_end_matches('.').to_string();
            let stands = if !m(c, "tileable").is_empty() { "the brewing stands of its slices (build as many slices side by side as you need)".to_string() } else if n == 1 { "its brewing stand".to_string() } else { format!("its {n} brewing stands") };
            format!("Brews the water bottles of its bottles chest into potions of swiftness in {stands}: it puts in one nether wart (awkward potions), then one sugar, burns blaze powder from its fuel chest and collects every finished potion in its output chest{}.", if rate.is_empty() { String::new() } else { format!(", {rate} potions a minute per stand") })
        }
        "burst" => format!("One press gives exactly {n} pulses{}.", if m(c, "period").is_empty() { String::new() } else { format!(", {} apart", ticks_short(m(c, "period"))) }),
        "divider" => format!(
            "Feed it a clock and the output goes on and off once for every {n} input pulses{}.",
            if m(c, "output") == "square" { ", on for exactly half of the time" } else { "" }
        ),
        "monostable" => {
            if c.kind.contains("non-retriggerable") {
                format!("One press keeps the output on for {l}; presses while it is on are ignored.")
            } else if c.kind.contains("retriggerable") {
                format!("One press keeps the output on for {l}; pressing again while it is on restarts the countdown.")
            } else {
                format!("One press, however short, keeps the output on for {l}.")
            }
        }
        "off-delay" => {
            // (an input that must charge it first, said up front: the 990-tick item timer needs
            // the input on for a minute to give its 49.5 seconds)
            let need = m(c, "min-pulse").split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).filter(|n| *n > 20);
            match need {
                Some(n) => format!("The output turns on with the input and stays on {l} after the input turns off, as long as the input stayed on for at least {} first (our test tried no shorter input).", ticks(&format!("{n} gt"))),
                None => format!("The output turns on with the input and stays on {l} after the input turns off."),
            }
        }
        "on-delay" => format!("The output only turns on once the input has been on for {}; shorter presses do nothing.", ticks(m(c, "T"))),
        "pulse-train" => format!("While the input is on, the output pulses every {}.", ticks(m(c, "P"))),
        // a combination lock (critique 41f1ec62a: no "hidden" code frame on a page whose caption
        // puts the code frames on the right; how many positions a dial has)
        "lock" => {
            let n = c.metrics.iter().find(|(k, _)| k == "dials").map(|(_, v)| v.clone()).unwrap_or_else(|| number(&c.kind).to_string());
            // (one dial: one frame, said so: "each of its 1 dials" on the 1-dial lock's page)
            let dials = if n == "1" { "its dial (an item frame, 8 positions)".to_string() } else { format!("each of its {n} dials (item frames, 8 positions each)") };
            let codes = if n == "1" { "the code frame" } else { "the code frames" };
            format!("Its output turns on only while {dials} is turned to the same position as its code frame, on the other side of the machine; set the code by turning {codes}, and keep that side out of sight.")
        }
        // a button sequence lock (clk-item's `sequence-lock N L`): clk-item's caption, in plain
        // words; how a wrong press counts from its `semantics` (the coordinator's main rule is a
        // keypad's sliding window; a strict-reset design passes too and says so; the first records,
        // judged before the metric, are strict-reset)
        "sequence-lock" => {
            let mut w = c.kind.split_whitespace().skip(1);
            let (n, l) = (w.next().unwrap_or("2"), w.next().unwrap_or("3"));
            if seq_sliding(c) {
                format!("Press its {n} buttons in the order set by the item frames on its back: it opens as soon as its last {l} presses match the code, like a keypad (a wrong press just becomes part of the next try), and the next press closes it.")
            } else {
                format!("Press its {n} buttons in the order set by the item frames on its back, {l} presses: each right press moves one step on, and a wrong press sends it back to the start and doesn't count as a first press. The output turns on after the last right press and stays on until the next press.")
            }
        }
        // a crop farm (w86's study 17): what it does, and its rate at the game's own speed
        "harvester" => {
            let crop = site_model::html::crop_words(c);
            let harvest = site_model::html::harvest_words(c);
            // its harvest at the game's normal speed, said plainly (vjeux 19:03Z: items an hour for
            // each block placed; random tick speed 3 is the game's own), the whole farm's first
            // (vjeux 21:39Z: "50 melon slices an hour is pitiful": the full-size farms)
            let sig2 = site_model::html::readable;
            let rate = match site_model::html::farm_rates(c) {
                Some((per_block, total)) if total > 0.0 => format!(
                    " At the game's normal growth speed (random tick speed 3) it gathers about {} {} an hour: {} for each block placed.",
                    sig2(total),
                    site_model::html::harvest_words_at(c, total),
                    sig2(per_block)
                ),
                Some((per_block, _)) => format!(" At the game's normal growth speed (random tick speed 3) it gathers about {} {} an hour for each block placed.", sig2(per_block), site_model::html::harvest_words_at(c, per_block)),
                None => String::new(),
            };
            let plant = site_model::html::plant_word(c);
            let from = if c.crops > 1 { format!(", from its {} {plant}s", c.crops) } else { String::new() };
            // (a fuel farm: what its harvest smelts as furnace fuel, the coordinator 2026-10-22)
            let rate = match site_model::html::fuel_parts(c) {
                Some((count, every, smelter)) => {
                    let about = match count.strip_prefix('~') {
                        Some(n) => format!("about {n}"),
                        None => count.clone(),
                    };
                    format!("{rate} As furnace fuel, its {} smelts {about} items an hour: a double chest of ore every {every}, {smelter}.", site_model::html::harvest_words(c))
                }
                None => rate,
            };
            // (a stem crop: the fruit grows beside its stem, which stays)
            if site_model::html::stem_crop(c) {
                let goes = if harvest.ends_with("slices") { "its slices go" } else { "it goes" };
                let beside = if c.crops > 1 { "a stem" } else { "its stem" };
                return format!("Harvests {} by itself, forever{from}: every {crop} that grows beside {beside} is broken and {goes} into a chest.{rate}", site_model::html::crop_plural(c));
            }
            format!("Harvests {crop} by itself, forever{from}: every block that grows is broken and its item goes into a chest.{rate}")
        }
        // an automatic door (clk-obs's autodoor study): what it does for a walker, then, plainly,
        // what it does to someone who stops in the doorway (the judge's reported `stop in
        // doorway`; the reviewer's and the coordinator's must-have, 2026-10-22); how it works is
        // each design's own caption
        "auto-door" => {
            let size = c.axis.split_whitespace().next().filter(|s| s.contains('x')).map(|s| format!("{} ", s.replace('x', "×"))).unwrap_or_default();
            let what = format!("A {size}piston door that opens by itself when you walk up from either side and closes behind you, even when two people walk through close together.");
            match site_model::html::door_stays_open(c) {
                Some(false) => format!("{what} {DOORWAY_HURTS}"),
                // (memory reads the order its plates were pressed in: one person standing is
                // safe, one standing while another walks through is not; designs/briefs/auto-door.md)
                Some(true) => format!("{what} It also stays open for someone standing in the doorway, but if someone else walks through while you stand there, it can still close on you and hurt you (half a heart per hurt) until you step out."),
                None => what,
            }
        }
        // a hopper-minecart unloader / loader (clk-obs's circuit::cartio): what happens to the cart
        "cart-unloader" | "cart-loader" => {
            let back = if c.axis.contains("through") { "it goes on along the line" } else { "back the way it came" };
            if c.kind.starts_with("cart-unloader") {
                format!("A hopper minecart full of items arrives, stops, and empties every item into the chest{}; once it is empty it leaves on its own, {back}.", rate(m(c, "unload rate").split_whitespace().next().unwrap_or("")))
            } else {
                // (the rail judge's loader rule, coordinator and reviewer 2026-10-22: an empty cart
                // at an empty supply must wait for items)
                format!("An empty hopper minecart arrives, stops, and fills from the supply chest{}; it leaves on its own once it is full, or once the chest runs dry with items in the cart, {back}. An empty cart at an empty chest waits there until items come.", rate(m(c, "load rate").split_whitespace().next().unwrap_or("")))
            }
        }
        // a whole minecart line (sorter's crate::cartsys): what happens to a chest's items, end to
        // end, and what the cart does with nothing to carry
        "cart-line" => {
            let over = c.value.map(|n| format!(" {n} blocks away")).unwrap_or_else(|| " far away".to_string());
            let waits = if site_model::html::cart_waits(c) { " With nothing to carry, the cart waits at the loader." } else { "" };
            format!("Carries the items you put in its supply chest to a storage chest{over}, by itself: one hopper minecart fills at the loader, rolls down the track, empties at the unloader and comes back for more.{waits}")
        }
        // w86's computational circuits: what the outputs show, in binary
        "binary-counter" => {
            let n: u32 = c.kind.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(4);
            let tags: Vec<String> = c.outputs.iter().map(|o| o.to_uppercase()).collect();
            let weights: Vec<String> = tags.iter().enumerate().map(|(i, t)| format!("{t} counts {}", 1u64 << i.min(30))).collect();
            format!("Each press of its button adds one to the count, shown in binary on its {n} outputs ({}): it counts from 0 to {}, then starts again at 0.", list_words(&weights.iter().map(String::as_str).collect::<Vec<_>>(), "and"), (1u64 << n.min(30)) - 1)
        }
        "decoder" => {
            let n: u32 = c.kind.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(2);
            format!("Its {n} inputs make a number from 0 to {} in binary (A0 counts 1, A1 counts 2{}), and exactly one of its {} outputs is lit: Q0 for 0, Q1 for 1, and so on.", (1u32 << n.min(16)) - 1, if n > 2 { ", A2 counts 4" } else { "" }, 1u32 << n.min(16))
        }
        // a player elevator (the fly study): the ride, and what each floor's button does
        "elevator" => {
            let n = c.kind.split_whitespace().nth(1).unwrap_or("");
            let t = |k: &str| m(c, k).split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(site_model::real_time);
            let trips = match (t("trip up"), t("down")) {
                (Some(u), Some(d)) => format!(" (up in {u}, down in {d})"),
                _ => String::new(),
            };
            format!("Carries a player {n} blocks up or down between two floors, on a platform a flying machine pushes along: press the button on your floor to call the platform, then step on and press it again to ride to the other floor{trips}.")
        }
        // an auto-crafter (clk-item's study): what it makes from what, and what a partial set does
        "crafter" => {
            let (makes, from) = site_model::html::crafter_recipe(c);
            // (several ingredients come from one supply chest each: clk-item's TNT crafter)
            let chests = if from.contains(" and ") { "fill its supply chests" } else { "fill its supply chest" };
            format!("Crafts {makes} from every {from} by itself: {chests} and every full set is crafted into its output chest{}. A partial set waits, without jamming, until the rest arrives.", match m(c, "crafts/min") {
                "" => String::new(),
                r => format!(" ({r} crafts a minute)"),
            })
        }
        // a minecart station (clk-obs's study 19): what happens to a cart, each way
        "station" => {
            let two = c.kind.ends_with(" 2");
            let other = if two { " Carts coming the other way pass straight through without stopping." } else { "" };
            format!("A cart arriving on its line stops at the platform, ridden or empty, and waits; press the button beside the rail and it rolls on.{other}")
        }
        // an item elevator (clk-misc's study): how high, how fast
        "item-elevator" => {
            let n = c.kind.split_whitespace().nth(1).unwrap_or("");
            format!("Lifts the items of its input chest {n} blocks straight up into its output chest{}, as fast as a hopper feeds them.", rate(m(c, "items/s")))
        }
        // a randomizer (clk-item's study 16): its share in words, and how the chosen output shows
        "randomizer" => {
            let n = site_model::html::randomizer_n(c).unwrap_or(2);
            let share = match n {
                2 => "half the time".to_string(),
                3 => "a third of the time".to_string(),
                4 => "a quarter of the time".to_string(),
                8 => "an eighth of the time".to_string(),
                n => format!("1 time in {n}"),
            };
            let shows = match m(c, "style") {
                "hold" => ", and it stays on until the next press picks again".to_string(),
                "pulse" if !m(c, "pulse").is_empty() => format!(", for a pulse of {}", ticks(m(c, "pulse"))),
                "pulse" => ", for one pulse".to_string(),
                _ => String::new(),
            };
            format!("Each press lights one of its {n} outputs, picked at random, each {share}{shows}.")
        }
        // the adder study (w86): which adder it is (every design is `adder 1`)
        "adder" => format!("{}.", first_cap(&crate::explain::kind_summary(&format!("adder {}", site_model::html::adder_variant(c))))),
        _ => {
            let s = crate::explain::kind_summary(&c.kind);
            if s.is_empty() { String::new() } else { format!("{}.", first_cap(&s)) }
        }
    }
}

/// How to try it in the 3D view, in one or two short sentences.
/// A design whose view shows it as built ([`Circuit::unmodeled`], from the viewer's own session:
/// it follows the viewer both ways): the build only, why in a visitor's words, then the game (the
/// coordinator 2026-10-22: "the view shows the build only, so download it to watch it run in the
/// game").
fn as_built_try(c: &Circuit) -> String {
    format!("The view shows the build only: {}. Download it to watch it run in the game.", crate::explain::as_built_why(&c.unmodeled))
}

pub fn try_it(c: &Circuit, has_viewer: bool) -> String {
    if !has_viewer {
        return "Download the design below to build it in the game (the 3D view needs a browser with WebGL).".to_string();
    }
    let b = base(&c.kind);
    // a view that cannot run the design, or stops early, promises no try-it of its kind (the
    // per-kind lines below assume a live view; the shulker machines and the locks say it their
    // own way)
    if !matches!(b, "shulker-loader" | "shulker-unloader" | "lock" | "harvester") {
        if !c.unmodeled.is_empty() {
            return as_built_try(c);
        }
        if !c.view_halt.is_empty() {
            let (tick, why) = crate::explain::halt_words(&c.view_halt);
            let at = tick.map(|t| format!(" at tick {t} of our test")).unwrap_or_default();
            return format!("The view shows it as built but stops{at}: {why}. Download it to try it in the game.");
        }
    }
    match b {
        // (and how to start one in the game: the judge places its start block for a tick; a
        // one-way machine with its own start lever: the lever, the coordinator 16:10Z)
        "flying-machine" if c.devices.iter().any(|d| d == "lever") && (c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way")) => return "Press play under the view to start it; then flip its lever (the IN button under the view): off, it docks and stays; on, it leaves again. In the game, start it by placing a block where the view puts its start block, then breaking it.".to_string(),
        "flying-machine" if c.devices.iter().any(|d| d == "lever") => return "Flip its lever (the IN button under the view): the machine sets off and flies until something stops it; reset to see it again. In the game, flip the lever the same way.".to_string(),
        "flying-machine" => return "Press play under the view: it puts the start block in place and the machine flies off; reset to see it again. In the game, start it the same way: place a block where the view puts its start block, then break it.".to_string(),
        // a duper lit once per trigger: its button, else a short pulse (the card's "per short
        // pulse")
        "tnt-duper-triggered" if c.devices.iter().any(|d| d == "button") => return "Press its button (IN under the view): each press throws one lit TNT. Press play under the view to watch our test on a loop; touch any other control to take over.".to_string(),
        "tnt-duper-triggered" => return "Tap IN under the view: a tap takes the redstone block away for a moment and one lit TNT flies out; held down, it keeps throwing them. Press play under the view to watch our test on a loop; touch any other control to take over.".to_string(),
        "tnt-duper" => {
            // (the try-it matches the card: a launcher throws its TNT, critique)
            let what = if c.axis.contains("launched") { "watch it throw a lit TNT away each cycle" } else { "watch a lit TNT come out of the machine each cycle" };
            // (its on/off lever: the coordinator 14:08Z)
            return if c.devices.iter().any(|d| d == "lever") {
                format!("Flip its lever (the IN button under the view) to start it: {what} while its TNT block stays in place; flip it back and it stops. Press play under the view to watch our test on a loop; touch any other control to take over.")
            } else {
                format!("Press play under the view: it runs by itself: {what} while its TNT block stays in place.")
            };
        }
        "shulker-loader" | "shulker-unloader" if !c.unmodeled.is_empty() => return "The view shows it as built, with our test's chests; download it to try it in the game.".to_string(),
        // the view's own run (at build time) sent out no box: no promise of boxes moving (the
        // 28-block loader: its judge's redsim scenarios pass, the browser's run sends none)
        "shulker-loader" | "shulker-unloader" if c.view_run.as_ref().is_some_and(|v| v.boxes == 0) => {
            return "The view runs it, but no box comes out there yet: the browser's simulator does not run this machine the way our test did. Download it to try it in the game.".to_string();
        }
        "shulker-loader" | "shulker-unloader" => {
            let lever = c.inputs.iter().zip(c.devices.iter()).any(|(n, d)| d == "lever" && !(n == "mode" || n.starts_with("mode.")));
            return format!(
                "{}Watch the items and boxes move between the chests (the numbers over each container show what it holds); the speed dropdown goes up to 100×.",
                if lever { "Flip its start lever (the lime IN button under the view). " } else { "Press play under the view to start it. " }
            );
        }
        "bud" => return "Press play under the view to watch our test: a block is placed, removed and changed at the cell marked TARGET, and a block next to it is updated with nothing changing (what only a true BUD notices); the box marked OUT turns red when it fires.".to_string(),
        // an item sorter: what to watch (critique: the generic "the box marked OUT turns red")
        "sorter" => {
            let item = c.kind.split_whitespace().nth(1).map(|i| item_plural(&i.replace('_', " "), 2)).unwrap_or_else(|| "items of its kind".to_string());
            return format!("Watch the line of mixed items: the {item} drop into the chest, everything else goes on to the end of the line (the numbers over each container show what it holds).");
        }
        "smelter" => return "Watch the hoppers carry the items into the top of each furnace and the coal into its side; the results drop into the output chest (the numbers over each container show what it holds).".to_string(),
        "brewer" => return "Watch a wart, then a sugar, go into the top of each brewing stand while the bottles go into its side; the finished potions drop into the output chest (the numbers over each container show what it holds).".to_string(),
        "composter" => return "Watch the crops leave the line of hoppers into the filter hopper of their slice and drop one by one into the composter; the bone meal falls into the chest under it, everything else runs on to the overflow chest (the numbers over each container show what it holds).".to_string(),
        // a button sequence lock: the code as built, in the view's own button names
        "sequence-lock" => {
            let label = |name: &str| -> String { c.inputs.iter().zip(&c.input_labels).find(|(n, _)| n.as_str() == name).map(|(_, l)| l.to_uppercase()).unwrap_or_else(|| name.to_uppercase()) };
            let code: Vec<String> = c.seq_code.split_whitespace().map(label).collect();
            let set = "To set another code, turn the item frames on its back: each frame's rotation names its step's button.";
            // (how a wrong press counts: `seq_sliding`)
            let wrong = if seq_sliding(c) { "a wrong press just becomes part of the next try" } else { "a wrong press sends it back to the start" };
            return if code.is_empty() {
                format!("Press its buttons under the view in the order its item frames set: {wrong}, and the output turns on after the last right press. {set}")
            } else {
                format!("With its frames as built the code is {}: press them in that order under the view and the output turns on; {wrong}. {set}", code.join(", "))
            };
        }
        // a combination lock: how to try it in the game, and in the view when its buttons turn
        // the frames (critique 058332315: its Try it named the raw buttons D0..K2, which put a
        // redstone block in the frame's cell instead of turning it); how to hook a door to it
        "lock" => {
            let game = "right-click a dial (an item frame on the dials' side) to turn its item one step, 8 steps around; the output turns on (open) only while every dial shows the same position as its code frame across the machine. To open a door with it, run redstone dust from its output (where the view shows the lamp) to the door";
            // the view's own button names (webrender's input_label: "Dial 1", "Code 1")
            let named = |prefix: char| -> Vec<String> { c.inputs.iter().zip(&c.input_labels).filter(|(n, _)| n.starts_with(prefix)).map(|(_, l)| l.clone()).collect() };
            let (dials, codes) = (named('d'), named('k'));
            return if !c.unmodeled.is_empty() {
                format!("The view shows it as built: the browser's simulator cannot run it yet. In the game, {game}.")
            } else if !crate::explain::viewer_turns_frames() {
                format!("The view runs its redstone, but its buttons cannot turn item frames yet: try the dials in the game. There, {game}.")
            } else if !dials.is_empty() && !codes.is_empty() {
                // (the tags lead: on a phone the bar has no room for the buttons, and the tags
                // press the inputs; critique b0fb9ae4e)
                let tag = |v: &[String]| -> String {
                    match v {
                        [one] => one.to_uppercase(),
                        [first, .., last] => format!("{} to {}", first.to_uppercase(), last.to_uppercase()),
                        [] => String::new(),
                    }
                };
                // (the 1-dial lock: one dial, one code frame)
                let one = dials.len() == 1;
                format!(
                    "Tap {} tag in the view ({}) or the frame itself to turn it one step, 8 steps around; the {} tag{} the code the same way (on a wide screen, the {} and {} buttons under the view do it too). The output turns on (open) only while {}. In the game, right-click a frame to turn it. To open a door with it, run redstone dust from its output (where the view shows the lamp) to the door.",
                    if one { "the dial's" } else { "a dial's" },
                    tag(&dials),
                    tag(&codes),
                    if codes.len() == 1 { " sets" } else { "s set" },
                    if one { dials[0].clone() } else { dials[0].split(' ').next().unwrap_or("Dial").to_string() },
                    if codes.len() == 1 { codes[0].clone() } else { codes[0].split(' ').next().unwrap_or("Code").to_string() },
                    if one { "the dial shows the same position as the code frame" } else { "every dial shows the same position as its code frame" }
                )
            } else {
                format!("Tap a dial (or its button under the view) to turn it one step, 8 steps around; the code frames set the code the same way. In the game, {game}.")
            };
        }
        // a display decoder: which input counts what, which output is which segment (the generic
        // "the box marked OUT" named a box it does not have)
        // (the 0-9 display: its one input is a chest's level, which only our test's walk sets to
        // each digit; the view's A button empties or fills it)
        "seven-segment" if site_model::html::decimal_display(c) => {
            return "Press play under the view to watch our test on a loop: it fills the chest tagged A to every level from 0 to 9, one change at a time, and the face shows each digit. The A button under the view only empties the chest or fills it to the top (15, beyond the digits it shows).".to_string();
        }
        "seven-segment" => {
            return "Set a number with the A0 to A3 buttons under the view (A0 counts 1, A1 2, A2 4, A3 8: all four on is F) and watch the segment outputs light: A top, B upper right, C lower right, D bottom, E lower left, F upper left, G middle. Press play under the view to watch our test on a loop; touch any other control to take over.".to_string();
        }
        // a crop farm: the view runs it like its judge (webrender: the judge's chests, the crop
        // planted, growth by random ticks at the judge's 100 times, FARM_RTS 300 against the
        // game's 3); a crop redsim does not grow yet is shown as built
        "harvester" => {
            let crop = site_model::html::crop_plural(c);
            let tile = site_model::html::tile_step(c).map(|s| format!(" To farm more, build copies side by side, one every {s} blocks.")).unwrap_or_default();
            if !c.unmodeled.is_empty() || !c.view_halt.is_empty() {
                // (shown as built: the build only, its crop drawn grown when the site made it a
                // still with its field grown, the reason in a visitor's words; the coordinator
                // 2026-10-22)
                let why = if c.unmodeled.is_empty() { crate::explain::halt_words(&c.view_halt).1 } else { crate::explain::as_built_why(&c.unmodeled) };
                let grown = if c.view_src.is_empty() { String::new() } else { format!(", with its {crop} grown") };
                return format!("The view shows the build only{grown}: {why}. Download it to watch it harvest in the game.{tile}");
            }
            let chests = if c.outputs.len() > 1 { "chests under its hoppers" } else { "chest under its hopper" };
            // (its judge's speed: 100 times the game's for a small farm, slower for a big one so
            // its collection keeps up, `farm::game_rts`)
            let faster = site_model::html::farm_speed_words(c);
            return format!("The view plants the {crop} and puts our test's {chests}, then runs it like our test, the {crop} growing {faster} than in the game: watch the farm break each block that grows and send its item into the chest (the tag over each chest shows what it holds).{tile}");
        }
        // an automatic door: the browser's simulator has no walkers: shown as built, how to use it
        // (how long you wait at it and what it does if you stop in the doorway, from its verdict)
        "auto-door" => {
            let opens = match site_model::html::door_open_wait(c) {
                Some(0) => "it opens before you reach it, even at a sprint, and closes once you have passed".to_string(),
                Some(w) => format!("it opens as you reach it (you may wait up to {} at the door) and closes once you have passed", site_model::real_time(w)),
                None => "it opens as you reach it and closes once you have passed".to_string(),
            };
            let stop = match site_model::html::door_stays_open(c) {
                Some(false) => " Walk on through: if you stop in the doorway, it closes on you.",
                Some(true) => " Stop in the doorway and it stays open for you.",
                None => "",
            };
            return format!("The view shows the door as built: the browser's simulator has no walkers to open it. In the game, walk up to it from either side: {opens}.{stop}");
        }
        // a hopper-minecart unloader / loader: the view has no hopper minecarts arriving yet: shown
        // as built, how to use it in the game
        "cart-unloader" => {
            return "The view shows the unloader as built, without carts. In the game, send a hopper minecart full of items down its line: it stops, empties into the chest, and leaves on its own once it is empty.".to_string();
        }
        "cart-loader" => {
            return "The view shows the loader as built, without carts. In the game, fill its supply chest and send an empty hopper minecart down its line: it stops, fills, and leaves on its own once it is full or the chest runs dry with items in the cart. An empty cart at an empty chest waits until you add items.".to_string();
        }
        // a whole minecart line: shown as built with our test's chests, no cart running
        "cart-line" => {
            return "The view shows the line as built, with our test's two double chests and no cart: the browser's simulator does not run it yet. In the game, put one hopper minecart on the track and items in the supply chest at the loader: the cart fills, carries them to the storage chest at the other end and comes back for more.".to_string();
        }
        // w86's computational circuits: which buttons, what lights
        "binary-counter" => {
            let clk = if c.input_labels.len() == c.inputs.len() && !c.input_labels.is_empty() { c.input_labels[0].clone() } else { c.inputs.first().map(|i| i.to_uppercase()).unwrap_or_else(|| "CLK".into()) };
            return format!("Press {clk} under the view, again and again: the outputs count up in binary (Q0 lit is 1, Q1 lit is 2, both lit is 3...). Press play under the view to watch our test on a loop; touch any other control to take over.");
        }
        "decoder" => {
            return "Turn the inputs on and off with the buttons under the view: both off lights Q0, A0 alone lights Q1, A1 alone lights Q2, both on lights Q3. Press play under the view to watch our test on a loop; touch any other control to take over.".to_string();
        }
        // a player elevator: the two floors' buttons, as the view labels them
        "elevator" => {
            let names: Vec<String> = if c.input_labels.len() == c.inputs.len() { c.input_labels.clone() } else { c.inputs.iter().map(|n| n.to_uppercase()).collect() };
            let both = list_words(&names.iter().map(String::as_str).collect::<Vec<_>>(), "or");
            return format!("Press {both} under the view, the buttons on the two floors: at the floor where the platform stands, a press sends it to the other floor; at the other floor, it calls the platform. Press play under the view to watch our test on a loop; touch any other control to take over.");
        }
        // an auto-crafter: the view does not feed it the judge's items yet: shown as built, how to
        // run it in the game
        "crafter" => {
            let (makes, from) = site_model::html::crafter_recipe(c);
            // (several ingredients: one supply chest each, clk-item's TNT crafter)
            if from.contains(" and ") {
                return format!("The view shows the crafter as built, without the items our test feeds it. In the game, fill each of its supply chests with its ingredient ({from} make one craft): every full set becomes {makes} in the output chest.");
            }
            return format!("The view shows the crafter as built, without the items our test feeds it. In the game, fill the chest on top of its hopper with {from} (or more): every full set becomes {makes} in the chest below.");
        }
        // a minecart station: the view has no carts yet (redsim models empty carts, not riders):
        // shown as built, how to use it in the game
        "station" => {
            return "The view shows the station as built, without carts. In the game, send a cart down its line: it stops on the platform's rails; press the button beside them (in reach from the cart) and it leaves.".to_string();
        }
        // an item elevator: the view does not load the judge's items yet (crate::lift fills the
        // input chest and empties the output chest every tick): shown as built, how to run it
        "item-elevator" => {
            // (one whose clock runs only while items come, clk-misc's bubble-rest: no start block)
            if m(c, "at rest").starts_with("yes") {
                return "The view shows the elevator as built, without the items our test loads into it. In the game, fill the bottom chest: the elevator starts by itself, every item rises into the top chest, and it rests again once the chest is empty.".to_string();
            }
            return "The view shows the elevator as built, without the items our test loads into it. In the game, start its clock by placing its last block where the view puts its start block, then fill the bottom chest: every item rises into the top chest.".to_string();
        }
        // a randomizer: a press like a button's (the judge holds it 20 to 30 ticks), what lights
        "randomizer" => {
            let tags = list_words(&c.outputs.iter().map(|o| o.to_uppercase()).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>(), "or");
            let after = match m(c, "style") {
                "pulse" => "flashes once",
                _ => "lights and stays lit until the next press picks again",
            };
            return format!("Press IN under the view, holding it a moment like a button press: one of the outputs ({tags}) {after}, at random. Press play under the view to watch our test on a loop; touch any other control to take over.");
        }
        // an adder: what to try (which buttons, what lights), in binary
        "adder" => {
            let tail = " Press play under the view to watch our test on a loop; touch any other control to take over.";
            return match site_model::html::adder_variant(c) {
                "half" => format!("Turn on A0, B0 or both with the buttons under the view: with one on, S0 (the sum) lights; with both, COUT (the carry) lights instead, because 1 + 1 = 10 in binary.{tail}"),
                "slice" => format!("Turn on A0, B0 and CIN (the carry in) with the buttons under the view: S0 lights when one or three are on, COUT when two or more are (1 + 1 + 1 = 11 in binary). To add longer numbers, stack copies side by side: each copy's COUT is the next copy's CIN.{tail}"),
                _ => format!("Turn on A0, B0 and CIN (the carry in) with the buttons under the view: S0 lights when one or three are on, COUT when two or more are (1 + 1 + 1 = 11 in binary).{tail}"),
            };
        }
        _ => {}
    }
    if !c.unmodeled.is_empty() {
        return as_built_try(c);
    }
    // a demo the view stops early (redsim meets what it does not model: the 100- and 576-item
    // counters' droppers holding several stacks, webrender's QA): no demo promised
    if !c.view_halt.is_empty() {
        let (tick, why) = crate::explain::halt_words(&c.view_halt);
        let at = tick.map(|t| format!(" at tick {t} of our test")).unwrap_or_default();
        return format!("The view shows it as built but stops{at}: {why}. Download it to try it in the game.");
    }
    let toggle = crate::explain::input_mode(&c.kind) == "toggle";
    let device = c.devices.first().map(String::as_str).unwrap_or("");
    // (the buttons as the view labels them, when known: webrender's input_label)
    let names: Vec<String> = if c.input_labels.len() == c.inputs.len() { c.input_labels.clone() } else { c.inputs.iter().map(|n| n.to_uppercase()).collect() };
    let press = match (device, names.len()) {
        (_, 0) => String::new(),
        ("lever", 1) => format!("Flip the lever marked {} (or tap {} under the view)", names[0], names[0]),
        ("button", 1) => format!("Press the button marked {} (or tap {} under the view)", names[0], names[0]),
        (_, 1) if toggle => format!("Tap {} under the view to turn the input on, again to turn it off", names[0]),
        (_, 1) => format!("Hold {} under the view for a long press, tap it for a short one", names[0]),
        _ => format!("Use the {} buttons under the view to drive the inputs", list_words(&names.iter().map(String::as_str).collect::<Vec<_>>(), "and")),
    };
    // the viewer draws the output as a box tagged OUT that turns red while powered (webrender
    // 1742c0308); several outputs, each by its own tag
    let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
    let several = if outs.len() > 1 { format!("the boxes marked {} turn red while their outputs are on", list_words(&outs.iter().map(String::as_str).collect::<Vec<_>>(), "and")) } else { String::new() };
    let watch = if c.outputs.is_empty() {
        "watch the machine move"
    } else if outs.len() > 1 {
        several.as_str()
    } else {
        "the box marked OUT turns red while the output is on"
    };
    let hint = match b {
        "piston-extender" => format!("turn it on and watch the block go {} blocks out; turn it off and it comes back", number(&c.kind)),
        "piston-bridge" => format!("turn it on and watch the bridge slide out across the {}-block gap; turn it off and it slides back into the wall", number(&c.kind)),
        "burst" => format!("each tap turns OUT red exactly {} times", number(&c.kind)),
        "divider" | "counter" if number(&c.kind) > 16 => format!("the demo counts the {} pulses for you: pick 100× in the speed dropdown and watch OUT change", number(&c.kind)),
        "divider" | "counter" => format!("tap it {} times and OUT goes through one on-off cycle", number(&c.kind)),
        "monostable" if c.kind.contains("non-retriggerable") => "a tap keeps OUT red for a while; tapping again while it is red changes nothing".to_string(),
        "monostable" => "a tap keeps OUT red for a while; tap again while it is red and it stays red longer".to_string(),
        "off-delay" => "turn it on, then off: OUT stays red a while after".to_string(),
        "on-delay" => "a quick tap does nothing; hold it and OUT turns red".to_string(),
        "pulse-train" => "turn it on and OUT keeps flashing red until you turn it off".to_string(),
        "tff" | "tff-reset" => "each tap flips OUT on or off".to_string(),
        _ => String::new(),
    };
    // (nothing plays at once, vjeux 21:29Z: the view starts on play)
    let demo = if c.stimulus.is_empty() { String::new() } else { " Press play under the view to watch our test on a loop; touch any other control to take over.".to_string() };
    if press.is_empty() {
        return format!("Watch the view: {watch}.{demo}");
    }
    format!(
        "{press}: {}.{demo}",
        match (hint.is_empty(), c.outputs.is_empty()) {
            (true, _) => watch.to_string(),
            (false, true) => hint,
            (false, false) => format!("{hint} ({watch})"),
        }
    )
}

fn rate(items_s: &str) -> String {
    if items_s.is_empty() { String::new() } else { format!(" ({items_s} items a second)") }
}

fn trim(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A button sequence lock that opens whenever its last presses match the code (`semantics:
/// sliding window`, the coordinator's main rule via clk-item); else it resets strictly (`strict
/// reset`, and the first records, judged before the metric existed).
fn seq_sliding(c: &Circuit) -> bool {
    m(c, "semantics").starts_with("sliding")
}

fn list_words(v: &[&str], and: &str) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].to_string(),
        _ => format!("{} {and} {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

fn first_cap(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

/// An automatic door's three page must-haves (the reviewer and the coordinator, via clk-obs
/// 2026-10-22), word for word: what happens to someone who stands in the doorway of a door that
/// closes on them, who else opens it, and the plain door when hiding does not matter. ("step out",
/// not "pushed out": the client code that moves a player out of a block is not verified.)
pub const DOORWAY_HURTS: &str = "Stand still in the doorway and the door closes on you and hurts you (half a heart per hurt) until you step out.";
pub const MOB_PLATES: &str = "A mob on a plate opens it too: nothing in vanilla tells a player from a mob, so put the plates where mobs can't walk.";
pub const IRON_DOORS: &str = "If the entrance doesn't need to be hidden, two iron doors with pressure plates do the job in about 10 blocks.";

/// What a builder must know before building it, near the top (critique, coordinator 13:54Z):
/// a sculk door can be set off by footsteps; a directional build works only facing one way.
pub fn warning(b: &Build) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    if b.kind == "door" && b.group == "sculk" {
        out.push("Uses a sculk sensor: footsteps or other noises nearby can open or close it. Keep walkways away from it, or shield it with wool.".to_string());
    }
    // a sculk clock: sounds nearby can add pulses (critique fd51ca0d2: sculk builds "keep noise
    // away"; the clocks' test runs in silence)
    if b.clock.is_some() && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
        out.push("Uses a sculk sensor: other sounds nearby (footsteps, another machine) can add pulses, so keep noise away from it.".to_string());
    }
    // a vibration-proof sculk circuit (clk-obs's vibration test; the sensitive ones are hidden)
    if let Some(c) = &b.circuit {
        let proof = c.metrics.iter().any(|(k, v)| k == "vibration-proof" && v.starts_with("yes"));
        if proof && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
            out.push(format!(
                "Uses a sculk sensor tuned so footsteps and other noises don't trigger it{}.",
                if b.kinds.iter().any(|k| k == "chest") { ": the items in its chest set the one sound it listens to" } else { "" }
            ));
        }
    }
    // what the world around it does to it (vjeux 19:02Z: these builds are published, with their
    // limitation said plainly instead of hidden)
    // a crop farm with a sculk sensor (w86's cactus farm hears the cactus block break): any noise
    // keeps the sensor busy, and a block grown in that time is lost (w86's correction: an extra
    // push is harmless, a missed sweep is not)
    if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("harvester")) && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
        let c = b.circuit.as_ref().unwrap();
        let crop = site_model::html::crop_words(c);
        let apart = site_model::html::tile_step(c).map(|s| format!(" Copies must stand {s} blocks apart so one does not hear another.")).unwrap_or_default();
        out.push(format!("Uses a sculk sensor that hears its crop break. Footsteps, mobs or other noises within 8 blocks keep it busy for 2 seconds; a {crop} that grows at the end of that time is lost.{apart}"));
    }
    // (an automatic door's tripwire is its walker sensor, there to be crossed: no "enclose it")
    if b.kinds.iter().any(|k| k.starts_with("tripwire")) && !b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("auto-door")) {
        out.push("Uses a tripwire: anything that crosses the string (a mob, a player, a dropped item) adds a pulse, so enclose it.".to_string());
    }
    // an automatic door: what else opens it (the reviewer's and the coordinator's page
    // must-haves, 2026-10-22: no vanilla detector tells a player from a mob; a string feels
    // dropped items too)
    if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("auto-door")) {
        out.push(MOB_PLATES.to_string());
        if b.kinds.iter().any(|k| k.starts_with("tripwire")) {
            out.push("Its outer rows are tripwire strings, which a dropped item presses too: keep items off them.".to_string());
        }
    }
    if b.kinds.iter().any(|k| k.starts_with("daylight_detector")) {
        out.push("Uses a daylight detector: it needs open sky and runs only in daytime; it stops at night and in rain.".to_string());
    }
    // a piston extender that breaks when its lever is flipped back too soon (clk-misc's `flips
    // breaks: off 20 gt after on strands blocks`): said plainly, never hidden
    if let Some(v) = b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, v)| k == "flips" && v.starts_with("breaks")).map(|(_, v)| v.clone())) {
        let within = v.split_once("off ").and_then(|(_, r)| r.split_whitespace().next()).and_then(|g| g.parse::<u32>().ok());
        out.push(match within {
            Some(g) => format!("Do not flip its lever back too soon: turned off within {} of being turned on, it leaves blocks stranded. Wait until the block is all the way out.", ticks(&format!("{g} gt"))),
            None => "Do not flip its lever back before the block is all the way out: it leaves blocks stranded.".to_string(),
        });
    }
    // an auto-smelter: the fuel the test uses (other fuels fill a furnace differently)
    if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("smelter")) {
        out.push("Our test feeds it coal (charcoal works the same); other fuels such as lava buckets or blocks of coal may be shared out differently between the furnaces.".to_string());
    }
    // an auto-composter: composting is random (the bone meal varies run to run)
    if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("composter")) {
        out.push("Composting is random: each item fills the composter with its own chance (wheat seeds and kelp 30%, sugar cane 50%, wheat 65%), so the bone meal per crop varies from run to run; seven layers make one bone meal.".to_string());
    }
    // an auto-brewer: the recipe the test brews (another second ingredient works the same way)
    if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("brewer")) {
        out.push("Our test brews potions of swiftness (nether wart, then sugar); another second ingredient in the sugar chest (a magma cream, a golden carrot...) brews its potion the same way. Keep the chests in order: sugar on water bottles makes useless mundane potions.".to_string());
    }
    // an item sorter: what it cannot sort (the coordinator 00:01Z: "it says plainly"; WR.md: a
    // comparator cannot count non-stacking items, and other names or enchantments do not stack)
    if let Some(c) = b.circuit.as_ref().filter(|c| c.kind.starts_with("sorter")) {
        out.push("It cannot sort items that do not stack (tools, armour, potions) nor items renamed or enchanted unlike the ones in its filter: they pass by unsorted.".to_string());
        // the judge's real-use runs (study 13, circuit::sorter): what a player must not do
        let badges = c.metrics.iter().find(|(k, _)| k == "badges").map(|(_, v)| v.clone()).unwrap_or_default();
        if !badges.split('+').any(|w| w == "stream-direct") {
            out.push("A water stream must end on a hopper of the line before the first slice, not on the first slice itself: that slice's hopper would take whole stacks and pass some of its own kind on unsorted. Nor put a hopper pointing down on top of the first slice: it feeds the line twice as fast.".to_string());
        }
        if !badges.split('+').any(|w| w == "overflow-proof") {
            out.push("When one slice's chest is full the line stops until it is emptied.".to_string());
        }
    }
    // a block change detector that misses pure block updates says so (the coordinator 21:55Z)
    if let Some(c) = b.circuit.as_ref().filter(|c| c.kind.starts_with("bud")) {
        let detects = c.metrics.iter().find(|(k, _)| k == "detects").map(|(_, v)| v.clone()).unwrap_or_default();
        if !detects.split_whitespace().any(|w| w == "update") {
            out.push("It does not notice pure block updates: a neighbour changing while the watched block itself stays the same gives no pulse.".to_string());
        }
    }
    // a clock too fast for a lamp (critique a417977c3: a redstone lamp waits 4 ticks before going
    // dark, so on a clock whose output is off for less than that it stays lit): simulated at build
    // time with a lamp in the output cell (generate's lamp check)
    if let Some(c) = b.clock.as_ref().filter(|c| !c.lamp.is_empty()) {
        let gap = ticks(&format!("{} gt", c.lamp_gap));
        out.push(match c.lamp.as_str() {
            // (by its cause, not its speed: critique 41f1ec62a, a 1-second hopper clock is not
            // "too fast", its output is just off briefly)
            "stays lit" => format!("A lamp on its output stays lit: the output is off for only {gap} at a time, and a redstone lamp goes dark only after more than 4 ticks (0.2 s) without power. Redstone dust there follows every pulse."),
            // (seen in the game: the coordinator 03:20Z)
            "relit" => format!("A redstone lamp on its output stays lit, though the power reaching it is off for {gap} at a time: the machine relights it within the same tick its power goes off. Redstone dust there follows every pulse."),
            _ => format!("A lamp on its output misses some pulses: its shortest gaps last only {gap}, and a redstone lamp goes dark only after more than 4 ticks (0.2 s) without power. Redstone dust there follows every pulse."),
        });
    }
    let robustness = match (&b.circuit, &b.clock) {
        (Some(c), _) => c.robustness.as_str(),
        (None, Some(c)) => c.robustness.as_str(),
        _ => "",
    };
    let directional = robustness.starts_with("rotation") || (b.kind == "door" && b.door.is_some() && !b.flags.iter().any(|f| f == "rotation-safe"));
    if directional {
        out.push("Works only facing the way it is shown here: build it from the download without turning it; turned another way it stops or changes its timing (our test tried all 4 directions).".to_string());
    } else if (robustness.contains("position") || robustness.contains("offset")) && crate::explain::numbers_only_variation(robustness).is_none() {
        // (one that passed the same way everywhere, a rate a little different at one spot, is
        // no warning: its properties say the numbers; sorter 2026-10-22 on ld-chest)
        out.push("Depends on where it is built: at another spot in the world its timing changed in our test, so try it where you build it.".to_string());
    }
    // what a chunk reload does to a clock (clockinfra's `reload`: never a reason to hide)
    if let Some(w) = b.clock.as_ref().and_then(|c| reload_words(&c.reload)) {
        out.push(w);
    }
    // no spot to take the output into a circuit (the judge's `usable`, vjeux 2026-10-21: a plain
    // warning, never a reason to hide)
    if b.clock.as_ref().is_some_and(|c| c.usable.starts_with("no")) {
        out.push(USABLE_WARNING.to_string());
    }
    if out.is_empty() { None } else { Some(out.join(" ")) }
}

/// The plain warning on a clock with no usable output (`clock::usable`: no cell beside it where a
/// repeater, a comparator or an observer takes every pulse).
pub const USABLE_WARNING: &str = "Hard to wire into a circuit: we found no spot beside it where a repeater, a comparator or an observer picks up every pulse.";

/// What a build needs to work where you build it, for its opening lines (critique, real-world
/// lens: a TNT duper that must stand 8 blocks up said so only deep in its notes): a TNT duper's
/// height above the ground (`safe drop`), the items its containers start with (the design's
/// `nbt:` lines), water, open sky.
/// An item name counted (plural where English counts it: "105 iron ingots"; block and mass names
/// as players say them: "1,111 cobblestone", "8 paper", "3 redstone").
fn item_plural(item: &str, n: u64) -> String {
    // (tools and other countable things too: rand4-plus's "2 iron shovel")
    const COUNTED: [&str; 20] = ["ingot", "nugget", "egg", "bone", "arrow", "pearl", "ball", "carrot", "potato", "apple", "seed", "rod", "bottle", "bucket", "shovel", "axe", "hoe", "sword", "book", "minecart"];
    if n > 1 && COUNTED.iter().any(|w| item.ends_with(w)) { format!("{item}s") } else { item.to_string() }
}

/// How to load a design's containers, slot by slot (critique on the item sorter: "where they go
/// IS the build"): one clause per loaded container, the top one first, each run of equal slots
/// in words. None without slot data.
fn loading_recipe(design: &str) -> Option<String> {
    let d = mctas::design::Design::parse(design).ok()?;
    // (position, block, slots: (slot, item, count, custom name))
    let mut cons: Vec<([i32; 3], String, Vec<(u8, String, u64, String)>)> = Vec::new();
    for (pos, snbt) in &d.block_entities {
        let block = d.blocks.get(pos).map(|s| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")).unwrap_or_default();
        let mut slots = Vec::new();
        for part in snbt.split("{Slot:").skip(1) {
            let slot = part.split('b').next().and_then(|s| s.trim().parse::<u8>().ok());
            let id = part.split("id:\"").nth(1).and_then(|s| s.split('"').next()).map(|s| s.trim_start_matches("minecraft:").replace('_', " "));
            let (Some(slot), Some(id)) = (slot, id) else { continue };
            let count = part.split("count:").nth(1).and_then(|c| c.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|c| c.parse::<u64>().ok()).unwrap_or(1);
            let name = part.split("custom_name\":\"").nth(1).and_then(|n| n.split('"').next()).unwrap_or("").to_string();
            slots.push((slot, id, count, name));
        }
        if !slots.is_empty() {
            slots.sort();
            cons.push((*pos, block, slots));
        }
    }
    if cons.is_empty() {
        return None;
    }
    cons.sort_by_key(|(p, _, _)| (-p[1], p[0], p[2]));
    let words = ["", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
    let mut clauses: Vec<String> = Vec::new();
    for (k, (_, block, slots)) in cons.iter().enumerate() {
        let same = cons.iter().filter(|(_, b, _)| b == block).count();
        let pos_word = if same == 2 { if cons.iter().take(k).any(|(_, b, _)| b == block) { "lower " } else { "upper " } } else { "" };
        // runs of consecutive slots holding the same thing
        let mut runs: Vec<(Vec<u8>, &str, u64, &str)> = Vec::new();
        for (slot, id, count, name) in slots {
            match runs.last_mut() {
                Some(r) if r.1 == id && r.2 == *count && r.3 == name && r.0.last().is_some_and(|l| l + 1 == *slot) => r.0.push(*slot),
                _ => runs.push((vec![*slot], id, *count, name)),
            }
        }
        let total = slots.len();
        let parts: Vec<String> = runs
            .iter()
            .enumerate()
            .map(|(ri, (ss, id, count, name))| {
                let what = if *count == 1 { format!("one {id}") } else { format!("{} {}", thousands(*count), item_plural(id, *count)) };
                let named = if name.is_empty() { String::new() } else { format!(" named “{name}”") };
                let place = if ss.len() == 1 && ss[0] == 0 {
                    "in its first slot".to_string()
                } else if ri == runs.len() - 1 && ri > 0 && ss.len() + runs[..ri].iter().map(|r| r.0.len()).sum::<usize>() == total && ss.len() > 1 {
                    format!("in each of the other {}", words.get(ss.len()).copied().unwrap_or("slots"))
                } else if ss.len() == 1 {
                    format!("in slot {}", ss[0] + 1)
                } else {
                    format!("in each of slots {} to {}", ss[0] + 1, ss[ss.len() - 1] + 1)
                };
                format!("{what}{named} {place}")
            })
            .collect();
        clauses.push(format!("the {pos_word}{block} holds {}", parts.join(" and ")));
    }
    Some(format!("Load it exactly like this (the download comes loaded): {}.", clauses.join("; ")))
}

pub fn needs(b: &Build, design: &str) -> Vec<String> {
    let mut v = Vec::new();
    // the 0-9 display: its input chest and its face's 13 lamps are our test's (the download has
    // blocks of the decoder only), and its limits, plainly (the reviewer via the coordinator
    // 2026-10-22: one digit only; copies stand 13 blocks apart)
    if let Some(c) = b.circuit.as_ref().filter(|c| site_model::html::decimal_display(c)) {
        let lamps = c.outputs.len();
        v.push(format!("After building it, put a chest (or any container) where the view shows the chest tagged A, and a redstone lamp in each of the {lamps} cells of its face: the download has none of them (our test adds them, and they do not count in its blocks)."));
        let wide = b.dims.as_deref().and_then(|d| d.split('x').next()).and_then(|w| w.trim().parse::<u32>().ok());
        v.push(match wide {
            Some(w) => format!("One digit only: the build is {w} blocks wide (the face is 3 of them), so copies stand {w} blocks apart and cannot make a two-digit number yet. It is tested for signals 0 to 9; a fuller container (signal 10 to 15) shows no digit we checked."),
            None => "One digit only: copies cannot stand close enough to make a two-digit number yet. It is tested for signals 0 to 9; a fuller container (signal 10 to 15) shows no digit we checked.".to_string(),
        });
    }
    if let Some(c) = &b.circuit {
        if let Some((_, drop)) = c.metrics.iter().find(|(k, _)| k == "safe drop") {
            // a precise drop: its blast's depth under the machine, from the judge's summary ("in
            // mid-air 10.0 blocks below the machine"), and the height our test stood it at: one
            // plain statement (critique b0fb9ae4e: "at least 4 up", "10 under the machine" and the
            // view's "23 above the ground" read as three contradicting heights)
            let below = site_model::html::tnt_burst_depth(c);
            let stood = c.metrics.iter().find(|(k, _)| k == "stands").and_then(|(_, v)| v.split_whitespace().next()?.parse::<u32>().ok());
            match (drop.split_whitespace().next().and_then(|n| n.parse::<u32>().ok()), below) {
                (Some(safe), Some(depth)) => {
                    let need = depth.floor() as u32 + 1;
                    let num = |x: f64| if x.fract() == 0.0 { format!("{x:.0}") } else { format!("{x:.1}") };
                    v.push(format!(
                        "Every TNT explodes {} blocks under the machine, so stand it at least {need} blocks up for the blast to be in mid-air{}; the machine itself survives from {safe} block{} up.",
                        num(depth),
                        stood.map(|h| format!(" (our test stood it {h} blocks up)")).unwrap_or_default(),
                        if safe == 1 { "" } else { "s" }
                    ));
                }
                (Some(0), None) => v.push("Works standing on the ground.".to_string()),
                // a design that declares its stand height (`stand: N`): said plainly (vjeux 11:49Z:
                // "stand it 4 blocks up"), and whether that is the lowest it is safe at (vjeux
                // 15:17Z: "is stand 6 the safe minimum?")
                (Some(n), None) if stood.is_some_and(|h| h >= n) => {
                    let h = stood.unwrap_or(n);
                    v.push(if h == n {
                        format!("Stand it {h} blocks up, the lowest our test found safe: lower, its own TNT explodes close enough to destroy it.")
                    } else {
                        format!("Stand it {h} blocks up (our test found it safe from {n} block{} up: lower, its own TNT explodes close enough to destroy it).", if n == 1 { "" } else { "s" })
                    });
                }
                // (true for launchers and droppers alike: critique, "lands under it" contradicted a
                // launcher's caption, its TNT pile up 10 to 20 blocks away and get thrown back)
                (Some(n), None) => v.push(format!("Must stand at least {n} blocks above the ground: lower, its own TNT explodes close enough to destroy it.")),
                (None, _) => {}
            }
        }
        // a crop farm: what the download leaves out (the judge plants the crop and adds the chest;
        // neither is counted in its blocks)
        if c.kind.starts_with("harvester") {
            // (a stem crop is planted as seeds: w86's pumpkin and melon farms, on farmland)
            let crop = if site_model::html::stem_crop(c) { format!("{} seed", site_model::html::crop_words(c)) } else { site_model::html::crop_words(c) };
            let soil = if c.soil.is_empty() { "soil" } else { c.soil.as_str() };
            let (crops, chests) = (c.crops.max(1), c.outputs.len().max(1));
            // (kelp: planted in the design's water source, on top of a hopper: w86)
            let plant = match (crops, c.crop_wet) {
                (1, true) => format!("plant one {crop} in the water on top of its {soil}"),
                (n, true) => format!("plant a {crop} in each of its {n} water sources"),
                (1, false) => format!("plant one {crop} on the {soil}"),
                (n, false) => format!("plant a {crop} on each of its {n} {soil} blocks"),
            };
            let chest = if site_model::html::fuel_rates(c).is_some() {
                // (a fuel farm: its lines run into the super smelter's fuel chest, the coordinator
                // 2026-10-22, not into chests of their own)
                let hoppers = mctas::design::Design::parse(design).map(|d| d.blocks.values().filter(|s| s.starts_with("minecraft:hopper")).count()).unwrap_or(0);
                match (chests, hoppers > 2 * chests as usize) {
                    (1, true) => "run its collector hopper line into your super smelter's fuel chest".to_string(),
                    (1, false) => "run its hopper into your super smelter's fuel chest".to_string(),
                    (2, true) => "run its two output hopper lines into your super smelter's fuel chest".to_string(),
                    (n, true) => format!("run its {n} output hopper lines into your super smelter's fuel chest"),
                    (n, false) => format!("run its {n} hoppers into your super smelter's fuel chest"),
                }
            } else if chests == 1 {
                // (one chest at the end of a collector line: w86's one-chest lattices, "a chest
                // under the end of the collector hopper line (the farm makes about one slice a
                // second; one hopper line carries 2.5)")
                let hoppers = mctas::design::Design::parse(design).map(|d| d.blocks.values().filter(|s| s.starts_with("minecraft:hopper")).count()).unwrap_or(0);
                if hoppers > 2 {
                    let per_s = site_model::html::farm_rates(c).map(|r| r.1 / 3600.0).filter(|r| *r > 0.0);
                    let rate = per_s.map(|r| format!(" (it gathers about {r:.1} {} a second; one hopper line carries 2.5)", site_model::html::harvest_words(c))).unwrap_or_default();
                    format!("put a chest under the end of its collector hopper line{rate}")
                } else {
                    "put a chest under its hopper".to_string()
                }
            } else {
                // (a full-size farm's hoppers run in lines, a chest at the end of each: w86's
                // lattices; a farm with as many hoppers as chests, one under each)
                let hoppers = mctas::design::Design::parse(design).map(|d| d.blocks.values().filter(|s| s.starts_with("minecraft:hopper")).count()).unwrap_or(0);
                if hoppers > 2 * chests as usize { format!("put a chest under the end of each of its {chests} hopper lines") } else { format!("put a chest under each of its {chests} hoppers") }
            };
            // (a fuel farm's lines feed the super smelter: only the bamboo is missing from the
            // download, and our test's chests stand in for the smelter's fuel chest)
            if site_model::html::fuel_rates(c).is_some() {
                let each = if chests == 1 { "a chest at the end of its line" } else { "a chest at the end of each line" };
                v.push(format!("After building it, {plant} and {chest}: the download has no {} (our test adds it, and {each}, which do not count in its blocks).", site_model::html::crop_words(c)));
            } else {
                v.push(format!("After building it, {plant} and {chest}: the download has neither (our test adds them, and they do not count in its blocks)."));
            }
            // (light: a stem or bamboo grows only at light 9 or more, fly's QA; the full-size
            // farms' line, w86)
            if site_model::html::farm_needs_light(c) {
                let p = site_model::html::plant_word(c);
                v.push(format!("It needs open sky, or light 9 at every {p} (a torch or lantern beside it): in the dark, indoors or underground, nothing grows; with light 9 it works day and night."));
            }
            // (a hopper minecart cannot be placed without a rail: fly's QA, w86's line)
            if design.lines().any(|l| l.trim_start().starts_with("entity: minecraft:hopper_minecart")) {
                v.push("Building it by hand, put each hopper minecart in with the rail trick: place a rail where the cart goes, put the cart on it, then break the rail.".to_string());
            }
        }
        // an item elevator: its two chests are our test's (not counted, not in the download)
        if c.kind.starts_with("item-elevator") {
            v.push("After building it, put a chest on top of its hopper (the items to lift) and one at the top, where the view shows its output: the download has neither (our test adds them, and they do not count in its blocks).".to_string());
        }
        // an auto-smelter: its input, fuel and output chests are our test's (fly's QA: a player
        // got furnaces and hoppers only); a double chest where the design names two halves
        // (`port items.b:`, sorter's 16-furnace smelter: one hopper line under each half)
        if c.kind.starts_with("smelter") {
            let double = |p: &str| design.lines().any(|l| l.trim_start().starts_with(&format!("port {p}.b:")));
            let chest = |p: &str, what: &str| if double(p) { format!("a double chest at {what}") } else { format!("a chest at {what}") };
            v.push(format!(
                "After building it, put {}, {} and a chest at OUT (the results): the download has none of them (our test adds them, and they do not count in its blocks).",
                chest("items", "ITEMS (the items to smelt)"),
                chest("fuel", "FUEL (the coal)")
            ));
        }
        // a hopper-minecart unloader / loader: its chest is our test's when the design names it
        // as a port (not counted, not in the download)
        if c.kind.starts_with("cart-unloader") && design.lines().any(|l| l.trim_start().starts_with("port output")) {
            v.push("After building it, put a chest where the view shows its output (the items it unloads): the download has none (our test adds it, and it does not count in its blocks).".to_string());
        }
        if c.kind.starts_with("cart-loader") && design.lines().any(|l| l.trim_start().starts_with("port items")) {
            v.push("After building it, put a chest where the view shows its supply and fill it with the items to load: the download has none (our test adds it, and it does not count in its blocks).".to_string());
        }
        // a whole minecart line: its two double chests and its cart are our test's; both ends must
        // be running (designs/briefs/cart-transport.md: a cart in a chunk that is not ticking
        // stops)
        if site_model::html::cart_line(c) {
            v.push("After building it, put a double chest at each end, the supply chest at the loader and the storage chest at the unloader (where the view shows them), and one hopper minecart on the track: the download has none of them (our test adds them, and they do not count in its blocks). A cart stops in chunks the game is not running: with a server's default simulation distance this line keeps working while you stand at either end.".to_string());
        }
        // an auto-crafter: its supply and output chests are our test's; its crafters' disabled
        // slots are part of the build (the download keeps them, a hand build must set them)
        if c.kind.starts_with("crafter") {
            let disabled: Vec<usize> = design.lines().filter_map(|l| l.trim_start().strip_prefix("nbt:")).filter_map(|l| l.split_once("disabled_slots:[I;").map(|(_, r)| r.split(']').next().unwrap_or("").split(',').filter(|s| !s.trim().is_empty()).count())).collect();
            // (a recipe of several ingredients has one supply chest each: clk-item's TNT crafter,
            // `port items.gunpowder`, `port items.sand`)
            let supplies: Vec<String> = design.lines().filter_map(|l| l.trim_start().strip_prefix("port items")).map(|r| r.split([':', ' ']).next().unwrap_or("").trim_start_matches('.').replace('_', " ")).collect();
            if supplies.len() > 1 {
                let names = list_words(&supplies.iter().map(String::as_str).collect::<Vec<_>>(), "and");
                v.push(format!("After building it, put a chest where the view shows each of its {} supplies ({names}) and one at its output: the download has none of them (our test adds them, and they do not count in its blocks).", supplies.len()));
            } else {
                v.push("After building it, put a chest on top of its hopper (the supply) and one under it (the output): the download has neither (our test adds them, and they do not count in its blocks).".to_string());
            }
            if !disabled.is_empty() {
                // (which crafters: a design's working crafter and the one held as a constant 9,
                // all its slots disabled: clk-item)
                let crafters = mctas::design::Design::parse(design).map(|d| d.blocks.values().filter(|s| s.starts_with("minecraft:crafter")).count()).unwrap_or(0);
                let slots = |n: usize| if n == 9 { "all 9 slots".to_string() } else { format!("{n} slots") };
                let tail = "the download keeps them; building it by hand, disable the same slots by clicking them in the crafter's screen.";
                v.push(match disabled.len() {
                    1 if crafters <= 1 => format!("Its crafter has {} disabled: {tail}", slots(disabled[0])),
                    1 => format!("One of its crafters has {} disabled: {tail}", slots(disabled[0])),
                    _ => format!("Its crafters have slots disabled ({}): {tail}", list_words(&disabled.iter().map(|n| slots(*n)).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>(), "and")),
                });
            }
        }
        // a randomizer's fairness, from our test (the verdict's `uniform:` flag), in plain numbers
        if let Some((presses, runs, counts)) = site_model::html::randomizer_fairness(c) {
            let list = list_words(&counts.iter().map(|n| n.to_string()).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>(), "and");
            v.push(format!("Fair in the real game: over {presses} presses in {runs} builds, its outputs came up {list} times."));
        }
        // a duper lit by one short pulse of a redstone block's removal: what its input is, and why
        // a button lights more (the judge's pulse is 4 ticks; a stone button holds 20); a duper with
        // its own button or lever says nothing here
        if c.kind.starts_with("tnt-duper-triggered") && !c.devices.iter().any(|d| d == "button" || d == "lever") {
            let pulse = circuit::tnt::PULSE;
            let block = c.inputs.first().map(|_| "the redstone block at its input cell").unwrap_or("its input");
            v.push(format!(
                "Its input is {block}: take it away for {} and one lit TNT flies out, put it back and the duper waits. Kept away longer it keeps throwing TNT, so a button (20 ticks or more) lights several: put a pulse limiter of {} between them.",
                ticks_short(&format!("{pulse} gt")),
                ticks_short(&format!("{pulse} gt"))
            ));
        }
    }
    // the items its containers start with, summed per item
    let mut items: Vec<(String, u64)> = Vec::new();
    for line in design.lines().filter_map(|l| l.trim_start().strip_prefix("nbt:")) {
        for part in line.split("id:\"").skip(1) {
            let id = part.split('"').next().unwrap_or("").trim_start_matches("minecraft:").replace('_', " ");
            let count = part.split("count:").nth(1).and_then(|c| c.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|c| c.parse::<u64>().ok()).unwrap_or(1);
            match items.iter_mut().find(|(i, _)| *i == id) {
                Some(e) => e.1 += count,
                None => items.push((id, count)),
            }
        }
    }
    if !items.is_empty() {
        let list: Vec<String> = items.iter().map(|(i, n)| format!("{} {}", thousands(*n), item_plural(i, *n))).collect();
        let one = items.len() == 1 && items[0].1 == 1;
        // a sorter: where its items go IS the build (critique): each container's slots in words
        let recipe = if b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("sorter")) { loading_recipe(design) } else { None };
        match recipe {
            Some(r) => v.push(r),
            None => v.push(format!("Comes loaded with {} in its containers: the download includes {}; building it by hand, put {} in first.", list.join(" and "), if one { "it" } else { "them" }, if one { "it" } else { "them" })),
        }
    }
    if b.kinds.iter().any(|k| k == "water") {
        v.push("Needs water where the view shows it.".to_string());
    }
    // item frames hold an item each, or a comparator reads nothing from them (critique 41f1ec62a:
    // the lock's page never said to put an item in each frame)
    let frames: Vec<&str> = design.lines().filter_map(|l| l.trim_start().strip_prefix("entity: minecraft:item_frame")).collect();
    if !frames.is_empty() {
        let item = frames
            .iter()
            .find_map(|f| f.split("id:\"").nth(1).and_then(|s| s.split('"').next()))
            .map(|i| item_plural(&i.trim_start_matches("minecraft:").replace('_', " "), 2))
            .unwrap_or_else(|| "items".to_string());
        let n = frames.len();
        v.push(format!(
            "Its {} item frame{} must each hold an item ({item} in the download; any item works): building it by hand, put one in each frame. A right-click turns a frame's item one step of 8.",
            n,
            if n == 1 { "" } else { "s" }
        ));
    }
    // (open sky for a daylight detector: said in its warning line)
    v
}

/// A family's base design: the range its one design makes, for the page's opening line
/// (critique 41f1ec62a: "any period from 1 s to 4 min 15 s, set by the number of items in the
/// hopper" sat at the bottom of the hopper clock's page): `To make it slower: <the family's rule>;
/// each step adds 16 ticks (0.8 s), up to every 5108 ticks (4 min 15.4 s).` None for any other
/// clock, or a family that reaches no further on the site.
pub fn slower_line(b: &Build) -> Option<String> {
    let c = b.clock.as_ref()?;
    let e = c.extend.as_ref()?;
    if e.base != b.id || e.hi <= c.every() || e.rule.is_empty() {
        return None;
    }
    let adds = match (e.dt > 0, e.db) {
        (false, _) => String::new(),
        (true, db) if db > 0 => format!("; each step adds {} and {}", ticks(&format!("{} gt", e.dt)), site_model::html::blocks_word(db as u32)),
        (true, _) => format!("; each step adds {}", ticks(&format!("{} gt", e.dt))),
    };
    Some(format!("To make it slower: {}{adds}, up to every {}.", e.rule, ticks(&format!("{} gt", e.hi))))
}

/// `1111` -> `1,111`.
fn thousands(n: u64) -> String {
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

/// What a chunk reload does to a clock, in plain words (clockinfra's `reload`; the coordinator:
/// "stops for good if its chunk unloads mid-cycle (you walk away and come back)"); None when it
/// keeps going or was not checked.
pub fn reload_words(reload: &str) -> Option<String> {
    let r = reload.trim();
    let (what, some) = match r.split_once(" in ") {
        Some((w, k)) if k.ends_with(" builds") => (w.trim(), Some(k.trim_end_matches(" builds").to_string())),
        _ => (r, None),
    };
    let some = some.map(|k| format!(" (in {k} of our test builds)")).unwrap_or_default();
    let walk = "its chunk unloads mid-cycle (you walk away and come back)";
    Some(if what == "stops" {
        format!("Stops for good if {walk}{some}: start it again the way you started it after building it.")
    } else if what == "glitch then resumes" {
        format!("If {walk}, it skips a beat, then keeps going{some}.")
    } else if let Some(p) = what.strip_prefix("changes period to ") {
        let gt = p.trim_end_matches(" gt").trim();
        format!("If {walk}, its period changes to {}{some}.", ticks(&format!("{gt} gt")))
    } else if let Some(why) = what.strip_prefix("fails: ") {
        format!("A chunk reload breaks it{some}: {why}.")
    } else {
        return None;
    })
}

/// A door's plain lines: (what it does, why you would build it, how to try it).
pub fn door(b: &Build) -> (String, String) {
    let d = b.door.as_ref();
    let timing = d.and_then(|d| Some(format!(" It opens in {} and closes in {}.", ticks(&format!("{} gt", d.open?)), ticks(&format!("{} gt", d.close?))))).unwrap_or_default();
    // a fully seamless door says what it looks like (vjeux 23:44Z: what people picture as a
    // piston door): flush when closed, a clean doorway when open, the machine hidden
    let tier = d.map(|d| d.seamless.as_str()).unwrap_or("");
    // a sculk door: the lever's click is what its sensor hears (critique: "say the sensor hears
    // the lever's click"); the opening's size (a 2x2 door: designs/piston-door-2x2)
    let n = site_model::door_side(b);
    let what = if b.group == "sculk" {
        format!("Flip the lever: the sculk sensor hears its click and the {n}×{n} doorway opens; flip it back and it closes.{timing}")
    } else if tier == "FULL" || tier == "SUPER" {
        format!("Flip the lever and the {n}×{n} doorway opens, a clean hole with no part of the machine in sight; flip it back and it closes into a flush wall, the machine hidden from both sides.{timing}")
    } else {
        format!("Flip the lever and the {n}×{n} doorway opens; flip it back and it closes.{timing}")
    };
    // (nothing plays at once, vjeux 21:29Z: the visitor presses play, then the door demo
    // runs, its lever flipped like the record videos; any input of the visitor's ends it)
    (what, "Press play under the view: the door opens, stays open a moment and closes, again and again, like our record video. To take over, tap the lever or the Flip button under the view: once to open the door, again to close it.".to_string())
}

/// A clock's plain lines: (what it does, why you would build it, how to try it).
pub fn clock(b: &Build) -> (String, String) {
    let c = b.clock.as_ref();
    let period = c.map(|c| ticks(&format!("{} gt", c.every()))).unwrap_or_default();
    // an output that pulses several times in one cycle of its parts says both
    let cycle = c.filter(|c| c.pulse_every.is_some()).map(|c| format!(" (its parts repeat every {})", ticks(&format!("{} gt", c.period)))).unwrap_or_default();
    let kicked = c.is_some_and(|c| !(c.start.is_empty() || c.start.starts_with("self")));
    let start = if kicked { " Built in the game it needs one kick to start (the last block placed); then it keeps going." } else { " It starts on its own once built." };
    // how to take its signal into a circuit (the judge's `usable` tap; a clock with none says so
    // in its warning)
    let tap = c.map(|c| tap_words(&c.usable)).unwrap_or_default();
    (
        // (not "forever" when something stops it: critique, the daylight clock stops at night)
        format!("Its output pulses by itself every {period}{cycle}{}.{start}", if b.kinds.iter().any(|k| k.starts_with("daylight_detector")) { " while the sun is up" } else { ", forever" }),
        format!("{}{tap}", if kicked { "Press play under the view to start it; then it runs by itself: pause and step one tick at a time with the buttons under it." } else { "Press play under the view: it runs by itself: pause and step one tick at a time with the buttons under it." }),
    )
}

/// How a builder takes a clock's signal into a circuit, from the judge's `usable` tap (` To use it
/// in a circuit, ...`; empty when it has none or was not checked: the warning says the first).
pub fn tap_words(usable: &str) -> String {
    match usable.trim() {
        "yes: repeater" => " To use it in a circuit, put a repeater where the view puts its lamp: it picks up every pulse.".to_string(),
        "yes: comparator" => " To use it in a circuit, take the signal from its comparator (part of the download, where the view puts its lamp): it reads the clock and passes on every pulse.".to_string(),
        "yes: observer" => " To use it in a circuit, take the signal from its observer (part of the download, where the view puts its lamp): it watches the clock and pulses every cycle.".to_string(),
        _ => String::new(),
    }
}

/// The "Try it" box under the 3D view: how to try it, and how this design does it (its plain
/// mechanism); what it does is in the heading, on the first screen.
pub fn intro_box(b: &Build, has_viewer: bool) -> String {
    let try_line = match (&b.circuit, b.kind.as_str()) {
        (Some(c), _) if b.group == "Sand elevator" && has_viewer => format!(
            "Flip the lever marked IN (or tap IN under the view): the sand rises {} blocks; flip it back and it falls into place.",
            number(&c.kind)
        ),
        // the item counters: what sets the count (critique: "nothing says HOW to load the items")
        (Some(c), _) if !c.layout_key.is_empty() && base(&c.kind) == "counter" => format!(
            "{} Its count is the number of items in its droppers: they pass the items one way per pulse, and the numbers over them show how many each holds. Load another number of items (stone here) to make it react on that count.",
            try_it(c, has_viewer)
        ),
        (Some(c), _) => try_it(c, has_viewer),
        (None, "door") => door(b).1,
        (None, "clock") => clock(b).1,
        _ => String::new(),
    };
    let how = crate::explain::caption_plain(&b.caption);
    // (a caption opening on how to load or use it says the needs box or the opening line again:
    // its next sentence is how it works; the hidden-code locks: "Put any item in each of the 6
    // item frames; ...", the randomizers: "Press the button and one of the two outputs turns on
    // at random, ... Inside, two droppers face each other: ...")
    let first = first_sentence(&how);
    let how = match how.strip_prefix(first.as_str()) {
        // (an instruction, then how it works after its colon: the sequence locks' "Press the
        // buttons in the order set by the item frames on the back: each right press moves a token
        // (an unlit copper bulb) one step on, ...": the part after the colon is the how)
        Some(_) if how.starts_with("Press the button") && first.split_once(": ").is_some_and(|(_, r)| !r.trim().is_empty()) => first_cap(first.split_once(": ").map(|(_, r)| r.trim()).unwrap_or("")),
        Some(rest) if how.starts_with("Put any item") || how.starts_with("Load it") || ((how.starts_with("Press the button") || how.starts_with("Crafts ")) && !rest.trim().is_empty()) => rest.trim_start_matches(['.', ' ']).to_string(),
        _ => how,
    };
    // ("Inside, two droppers ...": the how, without its lead-in)
    let how = how.strip_prefix("Inside, ").map(first_cap).unwrap_or(how);
    let how = first_sentence(&how);
    if try_line.is_empty() && how.is_empty() {
        return String::new();
    }
    let mut s = String::from("<section class=\"what plain\"><h2>Try it</h2>");
    if !try_line.is_empty() {
        s.push_str(&format!("<p>{}</p>", esc(&try_line)));
    }
    if !how.is_empty() {
        s.push_str(&format!("<p class=\"howdesign\"><b>How this design does it:</b> {how}</p>"));
    }
    s.push_str("</section>\n");
    s
}

/// The first sentence of an (escaped) text: up to the first ". " that is not an abbreviation.
pub fn first_sentence(s: &str) -> String {
    let mut from = 0;
    while let Some(i) = s[from..].find(". ") {
        let end = from + i;
        let head = &s[..end];
        if head.len() >= 24 && !head.ends_with("e.g") && !head.ends_with("i.e") && !head.ends_with("vs") {
            return s[..=end].to_string();
        }
        from = end + 2;
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circuit(kind: &str, metrics: &[(&str, &str)]) -> Circuit {
        Circuit { kind: kind.into(), metrics: metrics.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }
    }

    #[test]
    fn warnings_say_every_limitation() {
        // a tripwire clock that only works facing one way says both, in plain words
        let b = Build {
            kind: "clock".into(),
            kinds: vec!["tripwire".into(), "tripwire_hook".into(), "oak_boat".into()],
            clock: Some(site_model::ClockInfo { period: 40, robustness: "rotation-dependent".into(), ..Default::default() }),
            ..Default::default()
        };
        let w = warning(&b).unwrap_or_default();
        assert!(w.contains("Uses a tripwire") && w.contains("Works only facing the way it is shown here"), "{w}");
        let day = Build { kind: "clock".into(), kinds: vec!["daylight_detector".into()], clock: Some(site_model::ClockInfo { period: 20, robustness: "robust".into(), ..Default::default() }), ..Default::default() };
        assert!(warning(&day).unwrap_or_default().contains("stops at night and in rain"));
        assert!(warning(&Build { kind: "clock".into(), clock: Some(site_model::ClockInfo { period: 4, robustness: "robust".into(), ..Default::default() }), ..Default::default() }).is_none());
        // no usable output (the judge's usable column): a plain warning; usable: none
        let hard = |u: &str| warning(&Build { kind: "clock".into(), clock: Some(site_model::ClockInfo { period: 4, robustness: "robust".into(), usable: u.into(), ..Default::default() }), ..Default::default() });
        assert_eq!(hard("no: the output [0, -1, 0] is under the build").as_deref(), Some(USABLE_WARNING));
        assert!(hard("yes").is_none() && hard("").is_none());
        // a chunk reload, in plain words (clockinfra's reload column)
        assert!(reload_words("stops").unwrap().starts_with("Stops for good if its chunk unloads mid-cycle"));
        assert_eq!(reload_words("glitch then resumes in 2 of 4 builds").unwrap(), "If its chunk unloads mid-cycle (you walk away and come back), it skips a beat, then keeps going (in 2 of 4 of our test builds).");
        assert!(reload_words("changes period to 12 gt").unwrap().contains("its period changes to 12 ticks (0.6 s)"));
        assert!(reload_words("ok").is_none() && reload_words("").is_none());
    }

    #[test]
    fn a_sorter_says_how_to_load_its_hoppers() {
        // where the items go IS the build (critique): each hopper's slots in words, top first
        let design = "name: s\ncircuit: sorter iron_ingot\norigin: 0 0 0\npalette:\n  A minecraft:hopper[enabled=true,facing=down]\n  B minecraft:hopper[enabled=false,facing=down]\nlayer 1:\n  B\nlayer 2:\n  A\nnbt: 0 2 0 {Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:41},{Slot:1b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"filter\"}},{Slot:2b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"filter\"}},{Slot:3b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"filter\"}},{Slot:4b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"filter\"}}]}\nnbt: 0 1 0 {Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:64},{Slot:1b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"lock\"}},{Slot:2b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"lock\"}},{Slot:3b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"lock\"}},{Slot:4b,id:\"minecraft:paper\",count:1,components:{\"minecraft:custom_name\":\"lock\"}}]}\n";
        let b = Build { circuit: Some(circuit("sorter iron_ingot", &[])), ..Default::default() };
        assert_eq!(
            needs(&b, design),
            vec!["Load it exactly like this (the download comes loaded): the upper hopper holds 41 iron ingots in its first slot and one paper named “filter” in each of the other four; the lower hopper holds 64 iron ingots in its first slot and one paper named “lock” in each of the other four.".to_string()]
        );
    }

    #[test]
    fn needs_say_what_the_build_needs() {
        // a TNT duper's safe height, and the items its containers start with
        let b = Build { circuit: Some(circuit("tnt-duper", &[("safe drop", "8 blocks")])), ..Default::default() };
        let design = "nbt: 2 0 3 {Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:64},{Slot:1b,id:\"minecraft:cobblestone\",count:1047}]}";
        let v = needs(&b, design);
        assert_eq!(v[0], "Must stand at least 8 blocks above the ground: lower, its own TNT explodes close enough to destroy it.");
        assert!(v[1].starts_with("Comes loaded with 1,111 cobblestone in its containers"), "{}", v[1]);
        // (tools are counted: rand4-plus's two shovels)
        let shovels = needs(&Build { circuit: Some(circuit("randomizer 4", &[])), ..Default::default() }, "nbt: 0 0 0 {Items:[{Slot:0b,id:\"minecraft:iron_shovel\",count:1}]}\nnbt: 2 0 0 {Items:[{Slot:0b,id:\"minecraft:iron_shovel\",count:1}]}");
        assert!(shovels.iter().any(|n| n.starts_with("Comes loaded with 2 iron shovels in its containers")), "{shovels:?}");
        let ground = Build { circuit: Some(circuit("tnt-duper", &[("safe drop", "0 blocks (on the ground)")])), ..Default::default() };
        assert_eq!(needs(&ground, ""), vec!["Works standing on the ground.".to_string()]);
        // a declared stand: whether it is the lowest safe one (vjeux 15:17Z)
        let at = |stands: &str| needs(&Build { circuit: Some(circuit("tnt-duper", &[("safe drop", "6 blocks"), ("stands", stands)])), ..Default::default() }, "")[0].clone();
        assert_eq!(at("6 blocks above the ground"), "Stand it 6 blocks up, the lowest our test found safe: lower, its own TNT explodes close enough to destroy it.");
        assert!(at("9 blocks above the ground").starts_with("Stand it 9 blocks up (our test found it safe from 6 blocks up"));
    }

    #[test]
    fn a_duper_standing_up_counts_the_air_under_it() {
        // (the coordinator 15:18Z: 4x4x4 standing 6 up is 64 + 4·4·6 = 160; standing 4 up, 128)
        let duper = |stand: &str, safe: &str| Build {
            volume: Some(64),
            dims: Some("4x4x4".into()),
            circuit: Some(circuit("tnt-duper", &[("safe drop", safe), ("stands", stand)])),
            ..Default::default()
        };
        assert_eq!(site_model::html::effective_volume(&duper("6 blocks above the ground", "6 blocks")), Some(160));
        assert_eq!(site_model::html::effective_volume(&duper("4 blocks above the ground", "4 blocks")), Some(128));
        assert_eq!(site_model::html::effective_volume(&duper("", "0 blocks (on the ground)")), Some(64));
        // the lowest safe height counts when no stand is declared above it
        assert_eq!(site_model::html::effective_volume(&duper("", "5 blocks")), Some(64 + 16 * 5));
    }

    #[test]
    fn a_crop_farm_says_its_rate_and_what_to_add() {
        // w86's cane-mc-8b and cane2-mc-13b, as the archive carries them
        let c = Circuit {
            kind: "harvester sugar_cane".into(),
            axis: "sugar cane".into(),
            metrics: crate::circuits::parse_metrics("per cane 8.0 blocks, items per hour per cane 2.89, harvest height 2, wait 2 gt, bill 16 iron, 1 quartz, 4 redstone"),
            verdict: "CIRCUIT: cane-mc-8b harvester sugar_cane per cane 8.0 blocks, items per hour per cane 2.89, harvest height 2, wait 2 gt, bill 16 iron, 1 quartz, 4 redstone, 8 blocks, 4x3x1, robust".into(),
            tile: "5 0 0".into(),
            crops: 1,
            soil: "dirt".into(),
            outputs: vec!["out".into()],
            ..Default::default()
        };
        assert_eq!(c.metrics[0], ("per cane".to_string(), "8.0 blocks".to_string()));
        assert_eq!(c.metrics[1], ("items per hour per cane".to_string(), "2.89".to_string()));
        assert_eq!(one_liner(&c), "Harvests sugar cane by itself, forever: every block that grows is broken and its item goes into a chest. At the game's normal growth speed (random tick speed 3) it gathers about 2.9 sugar cane an hour: 0.36 for each block placed.");
        let b = Build { circuit: Some(c.clone()), blocks: 8, ..Default::default() };
        assert!(needs(&b, "").iter().any(|n| n.starts_with("After building it, plant one sugar cane on the dirt and put a chest under its hopper: the download has neither")), "{:?}", needs(&b, ""));
        assert!(try_it(&c, true).ends_with("To farm more, build copies side by side, one every 5 blocks."));
        // (the view runs a farm like its judge: webrender's harvester view)
        assert!(try_it(&c, true).starts_with("The view plants the sugar cane and puts our test's chest under its hopper, then runs it like our test"), "{}", try_it(&c, true));
        // the card leads with the farm's own harvest (vjeux 21:39Z; the coordinator 2026-10-22:
        // "3,300 melon slices an hour · 64 stems · 785 blocks · 4.2 per block · ~2,100 iron · needs
        // open sky or light 9"), the efficiency after its blocks
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "2.9 sugar cane an hour · 1 cane · 8 blocks · 0.36 per block · 16 iron · needs nothing but water");
        assert!(card.sub.starts_with("at the game's normal growth speed (random tick speed 3) · takes each block as soon as it grows"), "{}", card.sub);
        // two canes, two chests; its west cane waits for the east cane's next growth step
        let two = Circuit {
            metrics: crate::circuits::parse_metrics("per cane 6.5 blocks, items per hour per cane 2.82, harvest height 2, wait 91 gt, bill 32 iron, 1 quartz, 5 redstone"),
            verdict: "CIRCUIT: cane2-mc-13b harvester sugar_cane per cane 6.5 blocks, items per hour per cane 2.82, harvest height 2, wait 91 gt, bill 32 iron, 1 quartz, 5 redstone, 13 blocks, 7x3x1, robust".into(),
            tile: "7 0 0".into(),
            crops: 2,
            outputs: vec!["a".into(), "b".into()],
            ..c.clone()
        };
        let b2 = Build { circuit: Some(two.clone()), blocks: 13, ..Default::default() };
        let card = site_model::html::props(&b2);
        assert_eq!(card.title, "5.6 sugar cane an hour · 2 canes · 13 blocks · 0.43 per block · 32 iron · needs nothing but water");
        assert!(card.sub.contains("at the game's normal growth speed (random tick speed 3) · harvests each cane at 2 blocks tall"), "{}", card.sub);
        assert!(needs(&b2, "").iter().any(|n| n.starts_with("After building it, plant a sugar cane on each of its 2 dirt blocks and put a chest under each of its 2 hoppers")), "{:?}", needs(&b2, ""));
        // (the judge's own efficiency numbers win when it gives them: w86, coming)
        let judged = Circuit { metrics: crate::circuits::parse_metrics("per cane 6.5 blocks, items per hour per cane 2.82, items per hour per block 0.40, items per hour 5.20, harvest height 2"), ..two.clone() };
        assert_eq!(site_model::html::farm_rates(&judged), Some((0.40, 5.20)));
        // bamboo (w86's bamboo-hc-12b): per stalk, its mud, cut at 3 blocks tall (a bamboo is
        // not full at 3), no tile
        let bamboo = Circuit {
            kind: "harvester bamboo".into(),
            axis: "bamboo".into(),
            metrics: crate::circuits::parse_metrics("per cane 12.0 blocks, items per hour per cane 16.79, harvest height 3, wait 459 gt, bill 31 iron, 1 quartz, 4 redstone"),
            verdict: "CIRCUIT: bamboo-hc-12b harvester bamboo per cane 12.0 blocks, items per hour per cane 16.79, harvest height 3, wait 459 gt, bill 31 iron, 1 quartz, 4 redstone, 12 blocks, 3x6x4, robust".into(),
            tile: String::new(),
            crops: 1,
            soil: "mud".into(),
            outputs: vec!["a".into(), "b".into()],
            ..c.clone()
        };
        let b3 = Build { circuit: Some(bamboo.clone()), blocks: 12, ..Default::default() };
        let card = site_model::html::props(&b3);
        assert_eq!(card.title, "17 bamboo an hour · 1 stalk · 12 blocks · 1.4 per block · 31 iron · needs open sky or light 9");
        assert!(card.sub.contains("at the game's normal growth speed (random tick speed 3) · harvests each stalk at 3 blocks tall") && !card.sub.contains("cane"), "{}", card.sub);
        assert!(one_liner(&bamboo).contains("it gathers about 17 bamboo an hour: 1.4 for each block placed"), "{}", one_liner(&bamboo));
        assert!(needs(&b3, "").iter().any(|n| n.starts_with("After building it, plant one bamboo on the mud and put a chest under each of its 2 hoppers")), "{:?}", needs(&b3, ""));
        // (bamboo grows only at light 9: fly's QA)
        assert!(needs(&b3, "").iter().any(|n| n.starts_with("It needs open sky, or light 9 at every stalk")), "{:?}", needs(&b3, ""));
        // (a crop the browser's simulator does not grow yet: shown as built, the build only)
        let still = Circuit { unmodeled: "the browser's simulator does not grow bamboo yet".into(), ..bamboo.clone() };
        assert_eq!(try_it(&still, true), "The view shows the build only: the browser's simulator does not grow bamboo yet. Download it to watch it harvest in the game.");
        assert_eq!(crate::explain::metric_definition("harvester bamboo", "per cane"), "blocks placed for each bamboo stalk it harvests");
        // kelp (w86's kelp-17b): planted in the design's own water source, on top of a hopper
        let kelp = Circuit {
            kind: "harvester kelp".into(),
            axis: "kelp".into(),
            metrics: crate::circuits::parse_metrics("per cane 17.0 blocks, items per hour per cane 7.10, harvest height 2, wait 2 gt, bill 11 iron, 1 quartz, 5 redstone"),
            verdict: "CIRCUIT: kelp-17b harvester kelp per cane 17.0 blocks, items per hour per cane 7.10, harvest height 2, wait 2 gt, bill 11 iron, 1 quartz, 5 redstone, 17 blocks, 4x4x4, robust".into(),
            tile: "4 0 0".into(),
            crops: 1,
            soil: "hopper".into(),
            crop_wet: true,
            outputs: vec!["a".into(), "b".into()],
            ..c.clone()
        };
        let b4 = Build { circuit: Some(kelp.clone()), blocks: 17, ..Default::default() };
        let card = site_model::html::props(&b4);
        assert!(card.title == "7.1 kelp an hour · 1 plant · 17 blocks · 0.42 per block · 11 iron · needs nothing but water" && card.sub.contains("at the game's normal growth speed (random tick speed 3) · takes each block as soon as it grows") && !card.sub.contains("cane"), "{} | {}", card.title, card.sub);
        // (kelp grows in the dark)
        assert!(!needs(&b4, "").iter().any(|n| n.contains("light 9")), "{:?}", needs(&b4, ""));
        assert!(needs(&b4, "").iter().any(|n| n.starts_with("After building it, plant one kelp in the water on top of its hopper and put a chest under each of its 2 hoppers")), "{:?}", needs(&b4, ""));
        assert_eq!(crate::explain::metric_definition("harvester kelp", "per cane"), "blocks placed for each plant it harvests");
        assert_eq!(crate::explain::kind_title("harvester kelp"), "Kelp farm: harvests by itself (automatic farm)");
        // a stem crop (w86's pumpkin and melon farms): per stem, planted as seeds on farmland, the
        // melon's harvest in slices, no harvest height
        let melon = Circuit {
            kind: "harvester melon".into(),
            axis: "melon".into(),
            metrics: crate::circuits::parse_metrics("per cane 7.0 blocks, items per hour per cane 4.82, wait 2 gt, bill 16 iron, 1 quartz, 4 redstone"),
            verdict: "CIRCUIT: melon-mc-7b harvester melon per cane 7.0 blocks, items per hour per cane 4.82, wait 2 gt, bill 16 iron, 1 quartz, 4 redstone, 7 blocks, 2x6x1, robust".into(),
            tile: "0 0 2".into(),
            crops: 1,
            soil: "farmland".into(),
            crop_wet: false,
            outputs: vec!["out".into()],
            unmodeled: "the browser's simulator does not grow melons yet".into(),
            ..c.clone()
        };
        let b5 = Build { circuit: Some(melon.clone()), blocks: 7, ..Default::default() };
        let card = site_model::html::props(&b5);
        assert_eq!(card.title, "4.8 melon slices an hour · 1 stem · 7 blocks · 0.69 per block · 16 iron · needs open sky or light 9");
        assert!(card.sub.contains("at the game's normal growth speed (random tick speed 3)") && !card.sub.contains("tall"), "{}", card.sub);
        assert_eq!(one_liner(&melon), "Harvests melons by itself, forever: every melon that grows beside its stem is broken and its slices go into a chest. At the game's normal growth speed (random tick speed 3) it gathers about 4.8 melon slices an hour: 0.69 for each block placed.");
        assert!(needs(&b5, "").iter().any(|n| n.starts_with("After building it, plant one melon seed on the farmland and put a chest under its hopper")), "{:?}", needs(&b5, ""));
        assert!(try_it(&melon, true).starts_with("The view shows the build only: the browser's simulator does not grow melons yet. Download it to watch it harvest in the game."), "{}", try_it(&melon, true));
        // a cactus farm (w86's cactus-wool-9b): per plant, on sand, its sculk sensor said plainly
        let cactus = Circuit {
            kind: "harvester cactus".into(),
            axis: "cactus".into(),
            metrics: crate::circuits::parse_metrics("per cane 9.0 blocks, items per hour per cane 2.9, harvest height 2, wait 1 gt"),
            verdict: "CIRCUIT: cactus-wool-9b harvester cactus per cane 9.0 blocks, items per hour per cane 2.9, harvest height 2, wait 1 gt, 9 blocks, 3x5x2, robust".into(),
            tile: "9 0 0".into(),
            crops: 1,
            soil: "sand".into(),
            crop_wet: false,
            outputs: vec!["out".into()],
            unmodeled: String::new(),
            ..c.clone()
        };
        let b6 = Build { circuit: Some(cactus.clone()), blocks: 9, kinds: vec!["sculk_sensor".into(), "hopper".into()], ..Default::default() };
        let card = site_model::html::props(&b6);
        assert!(card.title == "2.9 cactus an hour · 1 plant · 9 blocks · 0.32 per block" && card.sub.contains("at the game's normal growth speed (random tick speed 3) · takes each block as soon as it grows"), "{} | {}", card.title, card.sub);
        assert!(needs(&b6, "").iter().any(|n| n.starts_with("After building it, plant one cactus on the sand and put a chest under its hopper")));
        assert_eq!(warning(&b6).as_deref(), Some("Uses a sculk sensor that hears its crop break. Footsteps, mobs or other noises within 8 blocks keep it busy for 2 seconds; a cactus that grows at the end of that time is lost. Copies must stand 9 blocks apart so one does not hear another."));
    }

    /// clk-obs's 0-9 display (the coordinator 2026-10-22, with the reviewer's limits): the card,
    /// the opening line, the chest and lamps a builder adds, its limits said plainly, Try it.
    #[test]
    fn the_decimal_display_says_one_digit_from_a_container() {
        let c = Circuit {
            study: "display".into(),
            kind: "seven-segment 4 hex decimal".into(),
            axis: "seven-segment 4 hex decimal".into(),
            metrics: crate::circuits::parse_metrics("latency 32 gt (a goes 0 -> 2 from a=0), glitches 25, on-blips 0"),
            inputs: vec!["a".into()],
            outputs: ["a", "ab", "af", "b", "bcg", "c", "cd", "d", "de", "e", "efg", "f", "g"].iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let b = Build { circuit: Some(c.clone()), blocks: 496, dims: Some("13x10x22".into()), ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "shows 0–9 from a container's fill level · 496 blocks · reads at a glance · one digit");
        assert_eq!(card.kind, "7-segment display");
        assert!(card.sub.contains("the digit changes within 1.6 s · no wrong lamp flashes between digits"), "{}", card.sub);
        assert!(!card.sub.contains("1,668"), "the obvious design's blocks: the body only");
        assert_eq!(one_liner(&c), "One comparator reads a chest (or any container) beside it, and its 3×5 lamp face shows the signal, 0 to 9, as a digit: the fuller the container, the higher the digit. A new digit shows within 1.6 s, with no wrong lamp flickering on between digits.");
        let n = needs(&b, "");
        assert!(n[0].starts_with("After building it, put a chest (or any container) where the view shows the chest tagged A, and a redstone lamp in each of the 13 cells of its face"), "{n:?}");
        assert!(n[1].starts_with("One digit only: the build is 13 blocks wide (the face is 3 of them), so copies stand 13 blocks apart and cannot make a two-digit number yet."), "{n:?}");
        assert!(try_it(&c, true).starts_with("Press play under the view to watch our test on a loop: it fills the chest tagged A to every level from 0 to 9"), "{}", try_it(&c, true));
        assert_eq!(crate::explain::obvious_line(&c), Some("For comparison: a straightforward one-hot decoder built by our generator takes 1,668 blocks for the same digit."));
        assert_eq!(crate::explain::kind_title(&c.kind), "7-segment display: one digit, 0 to 9, from a container's fill level");
        assert_eq!(site_model::html::value_words(&c.kind, &c.axis), "one digit, 0 to 9");
        assert_eq!(crate::explain::metric_value("32 gt (a goes 0 -> 2 from a=0)"), "32 gt (1.6 s; the slowest change: 0 to 2)");
    }

    /// A full-size farm (w86's melon-lattice8, the coordinator 2026-10-22): the farm's own harvest
    /// leads the card, rounded the way a player reads it, its plants, blocks, harvest per block,
    /// iron and the light its stems need; the opening line counts its stems; the needs say where
    /// its 64 seeds and 8 chests go, the light and the hopper minecarts' rail trick.
    #[test]
    fn a_full_size_farm_leads_with_its_harvest() {
        let c = Circuit {
            study: "farm".into(),
            kind: "harvester melon".into(),
            axis: "melon".into(),
            metrics: crate::circuits::parse_metrics("items per hour per block 4.237, items per hour 3326.0, per cane 12.27 blocks, items per hour per cane 51.97, wait 2 gt, bill 2112 iron, 64 quartz, 304 redstone"),
            tile: "17 0 0".into(),
            crops: 64,
            soil: "farmland".into(),
            outputs: (0..8).map(|k| format!("r{}", 2 * k)).collect(),
            ..Default::default()
        };
        assert!(site_model::html::full_size_farm(&c));
        let b = Build { circuit: Some(c.clone()), blocks: 785, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "3,300 melon slices an hour · 64 stems · 785 blocks · 4.2 per block · ~2,100 iron · needs open sky or light 9");
        assert!(one_liner(&c).starts_with("Harvests melons by itself, forever, from its 64 stems: every melon that grows beside a stem is broken and its slices go into a chest. At the game's normal growth speed (random tick speed 3) it gathers about 3,300 melon slices an hour: 4.2 for each block placed."), "{}", one_liner(&c));
        let design = "name: melon-lattice8\ncircuit: harvester melon\nport crop.s0: 0 1 0\nport output.r0: 7 -3 0\nport output.r2: 7 -3 2\norigin: 0 0 0\npalette:\n  H minecraft:hopper[enabled=true,facing=east]\nentity: minecraft:hopper_minecart 0.5 -1.0 1.5 {Rotation:[0f,0f]}\nlayer -2:\n  HHHHHHH\n  HHHHHHH\n  HHHHHHH\n";
        let n = needs(&b, design);
        assert!(n.iter().any(|l| l.starts_with("After building it, plant a melon seed on each of its 64 farmland blocks and put a chest under the end of each of its 8 hopper lines")), "{n:?}");
        assert!(n.iter().any(|l| l.starts_with("It needs open sky, or light 9 at every stem")), "{n:?}");
        assert!(n.iter().any(|l| l.starts_with("Building it by hand, put each hopper minecart in with the rail trick")), "{n:?}");
        assert_eq!(site_model::html::readable(661.7), "662");
        assert_eq!(site_model::html::readable(0.843), "0.84");
        assert_eq!(site_model::html::readable(646.0), "646");
        assert_eq!(site_model::html::about_count(1836), "~1,800");
        assert_eq!(site_model::html::blocks_word(1234), "1,234 blocks");
        // the full-size sugar cane farm (clk-rt's cane-lattice-16x16 on main, the coordinator's card)
        let cane = Circuit {
            kind: "harvester sugar_cane".into(),
            axis: "sugar cane".into(),
            metrics: crate::circuits::parse_metrics("items per hour per block 0.524, items per hour 646.0, per cane 6.05 blocks, items per hour per cane 3.17, harvest height 2, wait 591 gt, bill 1836 iron, 2 quartz, 542 redstone, 2 slime"),
            tile: "21 0 0".into(),
            crops: 204,
            soil: "dirt".into(),
            outputs: vec!["a".into(), "b".into()],
            ..c.clone()
        };
        let bc = Build { circuit: Some(cane), blocks: 1234, ..Default::default() };
        assert_eq!(site_model::html::props(&bc).title, "646 sugar cane an hour · 204 canes · 1,234 blocks · 0.52 per block · ~1,800 iron · needs nothing but water");
        // the sweeper (clk-rt's cane-sweeper-10x24): its slime and its one flying machine
        let sweeper = Circuit {
            metrics: crate::circuits::parse_metrics("items per hour per block 0.678, items per hour 595.4, per cane 4.57 blocks, items per hour per cane 3.10, harvest height 2, wait 494 gt, bill 1732 iron, 4 quartz, 12 redstone, 92 slime"),
            tile: "15 0 0".into(),
            crops: 192,
            flying_machines: 1,
            ..bc.circuit.clone().unwrap()
        };
        let bs = Build { circuit: Some(sweeper), blocks: 878, ..Default::default() };
        assert_eq!(site_model::html::props(&bs).title, "595 sugar cane an hour · 192 canes · 878 blocks · 0.68 per block · ~1,700 iron + 92 slime · one flying machine sweeps the field");
        assert_eq!(crate::circuits::flying_machines(include_str!("../../../designs/circuits/farm/archive/cane-sweeper-10x24.txt")), 1);
        assert_eq!(crate::circuits::flying_machines(include_str!("../../../designs/circuits/farm/archive/cane-lattice-16x16.txt")), 0);
        assert_eq!(site_model::html::about_count(64), "64");
    }

    #[test]
    fn a_randomizer_says_its_odds_and_its_fairness() {
        // clk-item's rand2-token, as the archive carries it
        let c = Circuit {
            kind: "randomizer 2".into(),
            axis: "1 of 2".into(),
            metrics: vec![("style".into(), "hold".into()), ("latency".into(), "8 gt".into()), ("presses".into(), "48".into())],
            verdict: "CIRCUIT: rand2-token randomizer 2 style hold, latency 8 gt, presses 48, 19 blocks, 10x3x1, robust (uniform: chi2 0.2 (limit 10.8) over 768 presses in 16 runs, counts 378/390)".into(),
            outputs: vec!["a".into(), "b".into()],
            inputs: vec!["in".into()],
            ..Default::default()
        };
        assert_eq!(one_liner(&c), "Each press lights one of its 2 outputs, picked at random, each half the time, and it stays on until the next press picks again.");
        let b = Build { circuit: Some(c.clone()), blocks: 19, ..Default::default() };
        assert_eq!(needs(&b, ""), vec!["Fair in the real game: over 768 presses in 16 builds, its outputs came up 378 and 390 times.".to_string()]);
        assert!(try_it(&c, true).contains("one of the outputs (A or B) lights and stays lit"), "{}", try_it(&c, true));
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "1 of 2 outputs at random · 19 blocks");
        assert!(card.sub.contains("the chosen output stays on until the next press · lights after 8 ticks (0.4 s) · tested fair over 768 presses"), "{}", card.sub);
        assert_eq!(site_model::html::value_words("randomizer 2", "1 of 2"), "1 of 2 outputs");
    }

    #[test]
    fn a_2x2_door_has_its_own_card_and_words() {
        // (clk-item's 2x2 doors, the coordinator 2026-10-21: their own card and records, after the
        // 3x3 card)
        let mk = |name: &str, side: u32, leads: &[&str], blocks: u32| Build {
            id: format!("doors/{name}"),
            kind: "door".into(),
            group: "solid".into(),
            name: name.into(),
            page: format!("doors/{name}.html"),
            blocks,
            volume: Some(blocks),
            door: Some(site_model::Door { leads: leads.iter().map(|s| s.to_string()).collect(), size: if side == 3 { 0 } else { side }, open: Some(1), close: Some(3), ..Default::default() }),
            ..Default::default()
        };
        let mut builds = vec![mk("big", 3, &["seamless"], 41), mk("big2", 3, &["solid"], 38), mk("classic6", 2, &["solid", "slime-free"], 20)];
        let notes = crate::kinds::select_doors(&mut builds);
        assert!(notes[0].starts_with("door cards: 2 (3x3: 2, 2x2: 1)"), "{notes:?}");
        let card = |n: &str| builds.iter().find(|b| b.name == n).unwrap().clone();
        assert_eq!(card("big").door.unwrap().records, 2);
        assert_eq!(card("classic6").door.unwrap().records, 1);
        assert!(card("big2").flags.iter().any(|f| f == "size") && !card("classic6").flags.iter().any(|f| f == "size"));
        assert_eq!(site_model::html::props(&card("classic6")).kind, "2×2 stone door");
        assert!(door(&card("classic6")).0.starts_with("Flip the lever and the 2×2 doorway opens"), "{}", door(&card("classic6")).0);
        assert!(door(&card("big")).0.contains("the 3×3 doorway opens"));
        // the Doors tab: the 3x3 card first, then the 2x2's (a smaller door, listed after)
        let site = site_model::Site { builds: builds.clone(), ..Default::default() };
        let (hits, _) = site.run(&site_model::Query::from_fragment("kind=door")).unwrap();
        let names: Vec<&str> = hits.iter().filter_map(|h| if let site_model::Hit::Build(i) = h { Some(site.builds[*i].name.as_str()) } else { None }).collect();
        assert_eq!(names, vec!["big", "classic6"]);
        assert!(site_model::html::lede(&site, "door").starts_with("The best 3×3 piston door for each of 2 records and the best 2×2 piston door, tested"), "{}", site_model::html::lede(&site, "door"));
    }

    #[test]
    fn an_item_elevator_says_how_high_and_how_fast() {
        // clk-misc's lift5-chain-14b, as the archive carries it
        let c = Circuit {
            kind: "item-elevator 5".into(),
            axis: "5 blocks up".into(),
            metrics: crate::circuits::parse_metrics("items/s 2.50, first item 30 gt, at rest no, bill 5 iron, 7 quartz, 20 redstone"),
            verdict: "CIRCUIT: lift5-chain-14b item-elevator 5 items/s 2.50, first item 30 gt, at rest no, bill 5 iron, 7 quartz, 20 redstone, 14 blocks, 4x6x1, robust".into(),
            outputs: vec!["out".into()],
            ..Default::default()
        };
        assert_eq!(c.metrics[2], ("at rest".to_string(), "no".to_string()));
        assert_eq!(one_liner(&c), "Lifts the items of its input chest 5 blocks straight up into its output chest (2.50 items a second), as fast as a hopper feeds them.");
        let b = Build { circuit: Some(c.clone()), blocks: 14, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "lifts items 5 blocks up · 14 blocks");
        assert!(card.sub.contains("2.50 items a second · the first item at the top after 1.5 s"), "{}", card.sub);
        assert!(needs(&b, "").iter().any(|n| n.starts_with("After building it, put a chest on top of its hopper")));
        assert_eq!(site_model::html::value_words("item-elevator 10", "10 blocks up"), "10 blocks up");
        // a tunable group's span keeps its unit words
        assert_eq!(site_model::html::span_words("5-20 blocks up, 3 values"), "3 versions, 5 to 20 blocks up");
        assert_eq!(site_model::html::span_words("L 12-36 gt, 13 values"), "13 versions, 12 to 36 ticks");
        assert_eq!(site_model::html::span_words("17-576, 3 values"), "3 versions, 17 to 576");
    }

    #[test]
    fn a_station_says_what_happens_to_a_cart() {
        // clk-obs's station-slope2-v4, as the archive carries it
        let c = Circuit {
            kind: "station 2".into(),
            axis: "two-way line".into(),
            metrics: crate::circuits::parse_metrics("line two-way, stop distance 1.60 blocks, departure delay 1 gt, leaves at 0.37 b/t, other way passes at 0.40 b/t"),
            verdict: "CIRCUIT: station-slope2-v4 station 2 line two-way, stop distance 1.60 blocks, departure delay 1 gt, leaves at 0.37 b/t, other way passes at 0.40 b/t, 10 blocks, 5x3x2, robust".into(),
            ..Default::default()
        };
        assert_eq!(c.metrics[1], ("stop distance".to_string(), "1.60 blocks".to_string()));
        assert_eq!(c.metrics[4], ("other way passes at".to_string(), "0.40 b/t".to_string()));
        let b = Build { circuit: Some(c.clone()), blocks: 10, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "a stop on a two-way line · 10 blocks");
        assert!(card.sub.contains("stops every cart within 1.60 blocks · sends it off at 7.4 blocks a second · the other way passes at 8.0 blocks a second"), "{}", card.sub);
        assert!(one_liner(&c).ends_with("Carts coming the other way pass straight through without stopping."));
        assert!(try_it(&c, true).starts_with("The view shows the station as built, without carts."));
        assert_eq!(crate::kinds::kind_key(&b), "station 2");
        assert!(crate::explain::kind_title("station 1").starts_with("Minecart station on a one-way line"));
    }

    #[test]
    fn a_crafter_says_what_it_crafts_from_what() {
        // clk-item's ironblock-v24, as the archive carries it
        let c = Circuit {
            kind: "crafter iron_block".into(),
            axis: "3x3 recipe".into(),
            metrics: crate::circuits::parse_metrics("crafts/min 16.6, first craft by 100 gt, crafts 22 (2 waiting sets checked), hoppers awake 1, recipe class 3x3, supply items, bill 5 iron, 2 quartz, 12 redstone"),
            verdict: "CIRCUIT: ironblock-v24 crafter iron_block crafts/min 16.6, first craft by 100 gt, crafts 22 (2 waiting sets checked), hoppers awake 1, recipe class 3x3, supply items, bill 5 iron, 2 quartz, 12 redstone, 16 blocks, 4x2x3, robust".into(),
            ..Default::default()
        };
        assert_eq!(c.metrics[1], ("first craft".to_string(), "by 100 gt".to_string()));
        assert_eq!(c.metrics[4], ("recipe class".to_string(), "3x3".to_string()));
        let b = Build { circuit: Some(c.clone()), blocks: 16, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "crafts an iron block from every 9 iron ingots · 16 blocks");
        assert!(card.sub.contains("16.6 crafts a minute · the first craft after 5 s"), "{}", card.sub);
        assert!(one_liner(&c).starts_with("Crafts an iron block from every 9 iron ingots by itself") && one_liner(&c).ends_with("A partial set waits, without jamming, until the rest arrives."), "{}", one_liner(&c));
        let design = "nbt: -1 0 1 {disabled_slots:[I;0,1,2,3,4,5,6,7,8]}\n";
        let n = needs(&b, design);
        assert!(n.iter().any(|l| l.starts_with("After building it, put a chest on top of its hopper (the supply)")), "{n:?}");
        assert!(n.iter().any(|l| l.starts_with("Its crafter has all 9 slots disabled: the download keeps them")), "{n:?}");
        assert!(try_it(&c, true).contains("fill the chest on top of its hopper with 9 iron ingots"));
        assert_eq!(crate::kinds::kind_key(&b), "crafter iron_block");
        assert_eq!(crate::explain::kind_title("crafter iron_block"), "Iron block crafter: crafts by itself from a supply chest (autocrafter)");
        let stick = Circuit { kind: "crafter stick".into(), ..c.clone() };
        assert_eq!(site_model::html::crafter_recipe(&stick), ("4 sticks".to_string(), "2 planks".to_string()));
        // a recipe of several ingredients, one supply chest each (clk-item's TNT crafter, next)
        let tnt = Circuit { kind: "crafter tnt".into(), axis: "multi recipe".into(), ..c.clone() };
        let bt = Build { circuit: Some(tnt.clone()), blocks: 20, ..Default::default() };
        let n = needs(&bt, "port items.gunpowder: 0 2 0\nport items.sand: 2 2 0\nport output: 1 -1 0\n");
        assert!(n.iter().any(|l| l.starts_with("After building it, put a chest where the view shows each of its 2 supplies (gunpowder and sand) and one at its output")), "{n:?}");
        assert!(try_it(&tnt, true).contains("fill each of its supply chests with its ingredient (5 gunpowder and 4 sand make one craft)"), "{}", try_it(&tnt, true));
        assert!(site_model::html::props(&bt).title.starts_with("crafts a TNT block from every 5 gunpowder and 4 sand"));
    }

    #[test]
    fn a_player_elevator_says_how_to_ride_it() {
        // the fly study's fly-elevator-5, as the archive carries it
        let c = Circuit {
            kind: "elevator 5".into(),
            axis: "height 5".into(),
            metrics: crate::circuits::parse_metrics("height 5 blocks, trip up 89 gt, down 94 gt, rider carries a rider, calls and sends from each floor"),
            verdict: "CIRCUIT: fly-elevator-5 elevator 5 height 5 blocks, trip up 89 gt, down 94 gt, rider carries a rider, calls and sends from each floor, 165 blocks, 8x11x12, robust".into(),
            inputs: vec!["bottom".into(), "top".into()],
            input_labels: vec!["BOT".into(), "TOP".into()],
            ..Default::default()
        };
        assert_eq!(c.metrics[1], ("trip up".to_string(), "89 gt".to_string()));
        assert_eq!(c.metrics[4], ("calls and sends".to_string(), "from each floor".to_string()));
        let b = Build { circuit: Some(c.clone()), blocks: 165, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "carries a player 5 blocks up and down · 165 blocks");
        assert!(card.sub.contains("up in 4.45 s, down in 4.7 s · a button on each floor calls it or sends it"), "{}", card.sub);
        assert!(one_liner(&c).ends_with("ride to the other floor (up in 4.45 s, down in 4.7 s)."), "{}", one_liner(&c));
        assert!(try_it(&c, true).starts_with("Press BOT or TOP under the view, the buttons on the two floors"), "{}", try_it(&c, true));
        assert_eq!(site_model::html::value_words("elevator 10", "height 10"), "10 blocks up");
    }

    #[test]
    fn a_cart_unloader_and_loader_say_what_happens_to_the_cart() {
        // clk-obs's cart-unloader-t2 / cart-loader-t2 (terminus), the metrics as announced
        let un = Circuit {
            kind: "cart-unloader".into(),
            axis: "terminus".into(),
            metrics: crate::circuits::parse_metrics("line terminus, unload rate 2.50 items/s, departure delay 4 gt"),
            ..Default::default()
        };
        assert_eq!(un.metrics[1], ("unload rate".to_string(), "2.50 items/s".to_string()));
        let b = Build { circuit: Some(un.clone()), blocks: 25, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "empties a hopper minecart into a chest, then sends it back · 25 blocks");
        assert!(card.sub.contains("2.50 items a second · the cart leaves 0.2 s after it is empty"), "{}", card.sub);
        assert_eq!(one_liner(&un), "A hopper minecart full of items arrives, stops, and empties every item into the chest (2.50 items a second); once it is empty it leaves on its own, back the way it came.");
        assert_eq!(crate::kinds::kind_key(&b), "cart-unloader terminus");
        assert!(needs(&b, "port output: 0 0 0\n").iter().any(|l| l.starts_with("After building it, put a chest where the view shows its output")));
        let lo = Circuit { kind: "cart-loader".into(), axis: "through line".into(), metrics: crate::circuits::parse_metrics("line through, load rate 2.50 items/s, departure delay 2 gt"), ..Default::default() };
        let bl = Build { circuit: Some(lo.clone()), blocks: 39, ..Default::default() };
        assert_eq!(site_model::html::props(&bl).title, "fills a hopper minecart from a chest, then sends it on · 39 blocks");
        assert!(one_liner(&lo).ends_with("once it is full, or once the chest runs dry with items in the cart, it goes on along the line. An empty cart at an empty chest waits there until items come."), "{}", one_liner(&lo));
        assert_eq!(crate::kinds::kind_key(&bl), "cart-loader through");
        assert!(try_it(&lo, true).starts_with("The view shows the loader as built, without carts."));
    }

    /// Sorter's minecart transport (2026-10-22): a minecart stop ranks by the items it moves a
    /// second, the smallest kept beside the fastest ("9 blocks smaller, half the speed", the
    /// coordinator); a whole line's card and its stations; a rate that differs at one spot is no
    /// "locational" headline.
    #[test]
    fn minecart_stops_rank_by_speed_and_a_line_names_its_stations() {
        let stop = |name: &str, kind: &str, blocks: u32, metrics: &str| Build {
            id: format!("circuits/rail/{name}"),
            name: name.into(),
            blocks,
            volume: Some(blocks * 2),
            circuit: Some(Circuit { study: "rail".into(), kind: kind.into(), axis: "terminus".into(), robustness: "robust".into(), metrics: crate::circuits::parse_metrics(metrics), ..Default::default() }),
            ..Default::default()
        };
        let t3 = stop("cart-unloader-t3", "cart-unloader", 22, "line terminus, unload rate 2.48 items/s, departure delay 15 gt");
        let gate = stop("ul-gate", "cart-unloader", 31, "line terminus, unload rate 4.79 items/s, departure delay 14 gt");
        let mut ld = stop("ld-chest", "cart-loader", 44, "line terminus, load rate 16.87 items/s, departure delay 17 gt");
        if let Some(c) = ld.circuit.as_mut() {
            c.robustness = "position-dependent (rotation 0 at [5, -11, -6]: line terminus, load rate 17.05 items/s, departure delay 17 gt (base build: line terminus, load rate 16.87 items/s, departure delay 17 gt))".into();
        }
        let line = Build {
            id: "circuits/rail/line-b".into(),
            name: "line-b".into(),
            blocks: 363,
            circuit: Some(Circuit {
                study: "rail".into(),
                kind: "cart-line".into(),
                value: Some(128),
                metrics: vec![
                    ("minutes for a double chest".into(), "20.9 min".into()),
                    ("items/s".into(), "2.75".into()),
                    ("bill".into(), "~65 iron, 24 gold, ~80 redstone, 3 quartz (track and cart included)".into()),
                    ("trickle".into(), "300 items fed 5 at a time every 5 s: all delivered by 5.9 min, then the cart parked at the loader".into()),
                ],
                parts: vec!["circuits/rail/ul-gate".into(), "circuits/rail/ld-chest".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(site_model::html::props(&line).title, "moves a double chest 128 blocks in 21 min · ~65 iron + 24 gold · the cart waits when there's nothing to carry");
        let mut builds = vec![t3, gate, ld.clone(), line];
        crate::kinds::select_circuits(&mut builds);
        let get = |n: &str| builds.iter().find(|b| b.name == n).and_then(|b| b.circuit.clone()).unwrap_or_default();
        let (t3, gate, ld) = (get("cart-unloader-t3"), get("ul-gate"), get("ld-chest"));
        assert!(builds.iter().any(|b| b.name == "cart-unloader-t3" && b.flags.iter().any(|f| f == "size")), "t3: a page, no card");
        assert_eq!(t3.claim, "9 blocks smaller, half the speed");
        assert_eq!(t3.station_twin.as_deref(), Some("circuits/rail/ul-gate"));
        assert_eq!(gate.station_twin.as_deref(), Some("circuits/rail/cart-unloader-t3"));
        assert!(gate.claim.starts_with("the fastest of 2 tested"), "{}", gate.claim);
        assert_eq!(gate.part_of.as_deref(), Some("circuits/rail/line-b"));
        assert_eq!(ld.part_of.as_deref(), Some("circuits/rail/line-b"));
        // (the loader passed the same way everywhere: no "locational" warning, its numbers said)
        assert!(warning(&Build { circuit: Some(ld.clone()), ..Default::default() }).is_none());
        assert!(crate::explain::direction_words(&ld.robustness).starts_with("yes: it passed the same way"), "{}", crate::explain::direction_words(&ld.robustness));
        assert!(one_liner(&ld).ends_with("An empty cart at an empty chest waits there until items come."));
    }

    /// w86's fuel farm (designs/briefs/fuel-farm.md, 2026-10-22): a harvester whose verdict carries
    /// `items smelted per hour (per block)` leads with what its bamboo smelts (the reviewer's "a
    /// 64-stalk field fuels a double chest of ore every 14 hours; 12 such fields keep an 8-furnace
    /// super smelter at full speed"), from the brief's bamboo-sweeper8x8 row.
    #[test]
    fn a_fuel_farm_says_what_its_harvest_smelts() {
        let metrics = crate::circuits::parse_metrics("items smelted per hour per block 0.521, items smelted per hour 246.0, items per hour per block 2.085, items per hour 984.1, per cane 7.49 blocks, items per hour per cane 15.62, harvest height 2, bill 672 iron, 2 quartz, 9 redstone, 74 slime");
        assert_eq!(metrics[0], ("items smelted per hour per block".to_string(), "0.521".to_string()));
        assert_eq!(metrics[1], ("items smelted per hour".to_string(), "246.0".to_string()));
        let c = Circuit { study: "farm".into(), kind: "harvester bamboo".into(), crops: 63, flying_machines: 1, metrics, ..Default::default() };
        let b = Build { circuit: Some(c.clone()), blocks: 472, ..Default::default() };
        let p = site_model::html::props(&b);
        assert_eq!(p.title, "fuels ~250 items smelted an hour: a double chest of ore every 14 h, a twelfth of an 8-furnace super smelter at full speed · 63 bamboo stalks · 472 blocks · 672 iron + 74 slime");
        assert!(p.sub.starts_with("0.52 items smelted an hour per block · 984 bamboo an hour · one flying machine sweeps the field · needs open sky or light 9"), "{}", p.sub);
        assert!(one_liner(&c).ends_with("As furnace fuel, its bamboo smelts about 250 items an hour: a double chest of ore every 14 h, a twelfth of an 8-furnace super smelter at full speed."), "{}", one_liner(&c));
        assert!(crate::explain::metric_definition("harvester bamboo", "items smelted per hour per block").contains("a quarter of an item each"));
        // (the coordinator's 10x32 card: "fuels ~1,350 items smelted an hour: a double chest of ore
        // every 2.5 h, half of an 8-furnace super smelter at full speed")
        let big = Circuit { metrics: crate::circuits::parse_metrics("items smelted per hour per block 1.038, items smelted per hour 1357.4"), ..c.clone() };
        assert_eq!(site_model::html::fuel_lead(&big).as_deref(), Some("fuels ~1,350 items smelted an hour: a double chest of ore every 2.5 h, half of an 8-furnace super smelter at full speed"));
        // (w86's 10x64 field, 2,750.2 of the 2,880 an hour: never "full speed" short of it)
        let field = Circuit { metrics: crate::circuits::parse_metrics("items smelted per hour per block 1.104, items smelted per hour 2750.2"), ..c.clone() };
        assert_eq!(site_model::html::fuel_lead(&field).as_deref(), Some("fuels ~2,750 items smelted an hour: a double chest of ore every 1.3 h, 95% of an 8-furnace super smelter at full speed"));
        // (never "enough" short of it: 2,879 of 2,880 is 99%)
        let close = Circuit { metrics: crate::circuits::parse_metrics("items smelted per hour per block 1.2, items smelted per hour 2879"), ..c.clone() };
        assert!(site_model::html::fuel_lead(&close).is_some_and(|l| l.ends_with("99% of an 8-furnace super smelter at full speed")));
        // (its lines run into the super smelter's fuel chest, not chests of their own)
        let b2 = Build { circuit: Some(Circuit { outputs: vec!["a".into(), "b".into()], soil: "mud".into(), ..c.clone() }), blocks: 472, ..Default::default() };
        let design = "port output.a: 0 0 0\nport output.b: 1 0 0\norigin: 0 0 0\npalette:\n  H minecraft:hopper[enabled=true,facing=down]\nlayer 0:\n  HHHHHHHHHH\n";
        let n = needs(&b2, design);
        assert!(n.iter().any(|l| l.starts_with("After building it, plant a bamboo on each of its 63 mud blocks and run its two output hopper lines into your super smelter's fuel chest")), "{n:?}");
    }

    /// Webrender's as-built kinds (2026-10-22: crop farms, super smelters, brewers and composters,
    /// the viewer's `circuit::not_replayed_yet`): Try it and How to try it say the build only, why,
    /// and the game (the coordinator: "the view shows the build only, so download it to watch it
    /// run in the game"); the same design simulated says the live text again.
    #[test]
    fn an_as_built_view_says_the_build_only() {
        let replay = mcview::circuit::NOT_REPLAYED;
        let farm = Circuit { study: "farm".into(), kind: "harvester minecraft:sugar_cane".into(), crops: 256, outputs: vec!["a".into(), "b".into()], unmodeled: format!("{replay} (a crop farm)"), ..Default::default() };
        assert_eq!(try_it(&farm, true), "The view shows the build only: it does not replay our test exactly yet. Download it to watch it harvest in the game.");
        let grown = Circuit { view_src: "designs/stills/farm/x.txt".into(), ..farm.clone() };
        assert!(try_it(&grown, true).starts_with("The view shows the build only, with its sugar cane grown: it does not replay our test exactly yet."), "{}", try_it(&grown, true));
        let how = crate::explain::how_to_try(&farm, true);
        assert!(how.contains("shows the build only (<b>as built</b>): it does not replay our test exactly yet.") && how.contains("download the design (below) to watch it run in the game") && !how.contains("ress play"), "{how}");
        // (made exact again: the live words come back by themselves)
        let live = Circuit { unmodeled: String::new(), ..farm.clone() };
        assert!(try_it(&live, true).starts_with("The view plants the sugar cane"), "{}", try_it(&live, true));
        let smelter = Circuit { study: "smelter".into(), kind: "smelter 8".into(), unmodeled: format!("{replay} (a super smelter)"), ..Default::default() };
        assert_eq!(try_it(&smelter, true), "The view shows the build only: it does not replay our test exactly yet. Download it to watch it run in the game.");
        assert!(!crate::explain::how_to_try(&smelter, true).contains("ress play"));
        // (what the browser's simulator does not run comes first)
        let blast = Circuit { unmodeled: format!("block \"blast_furnace\" is not modeled; {replay} (a super smelter)"), ..smelter.clone() };
        assert_eq!(try_it(&blast, true), "The view shows the build only: it uses blast furnaces, which the browser's simulator does not run yet. Download it to watch it run in the game.");
    }

    /// A big crop farm replayed at its judge's speed (webrender c11cda9a34, the coordinator
    /// 2026-10-22: `data-farm-rts`, `circuit::farm::game_rts`): its pages say how much faster than
    /// the game it grows, from that speed (melon-lattice8 at 8, the 10x64 bamboo field at 4).
    #[test]
    fn a_big_farm_says_its_own_speed() {
        let farm = |rts: Option<u32>| Circuit { study: "farm".into(), kind: "harvester minecraft:melon".into(), crops: 64, outputs: vec!["a".into()], farm_rts: rts, ..Default::default() };
        assert_eq!(site_model::html::farm_speed_words(&farm(None)), "100 times faster");
        assert_eq!(site_model::html::farm_speed_words(&farm(Some(8))), "about 3 times faster");
        assert_eq!(site_model::html::farm_speed_words(&farm(Some(4))), "1.3 times faster");
        assert_eq!(site_model::html::farm_speed_words(&farm(Some(30))), "10 times faster");
        assert_eq!(site_model::html::farm_speed_words(&farm(Some(37))), "about 12 times faster");
        assert!(try_it(&farm(Some(8)), true).contains("the melons growing about 3 times faster than in the game"), "{}", try_it(&farm(Some(8)), true));
        assert!(crate::explain::how_to_try(&farm(Some(8)), true).contains("grows it about 3 times faster than the game"));
    }

    #[test]
    fn an_extender_says_whether_it_can_be_flipped_back_early() {
        // pext's flip check (clk-misc), as main's archive carries it
        let mk = |flips: &str| Build {
            circuit: Some(Circuit { kind: "piston-extender 4".into(), metrics: vec![("extend".into(), "38 gt".into()), ("flips".into(), flips.into())], ..Default::default() }),
            blocks: 23,
            flags: crate::circuits::test_flags(&[("flips", flips)]),
            ..Default::default()
        };
        let breaks = mk("breaks: off 20 gt after on strands blocks");
        assert_eq!(warning(&breaks).as_deref(), Some("Do not flip its lever back too soon: turned off within 20 ticks (1 s) of being turned on, it leaves blocks stranded. Wait until the block is all the way out."));
        let safe = mk("safe (the lever flipped back at any time)");
        assert!(warning(&safe).is_none());
        assert!(site_model::html::props(&safe).chips.iter().any(|c| c == "safe to flip back any time"), "{:?}", site_model::html::props(&safe).chips);
    }

    #[test]
    fn a_binary_counter_and_a_decoder_say_their_bits() {
        // w86's bcount4-spiral and dec2-24b, as the archive carries them
        let bc = Circuit {
            kind: "binary-counter 4".into(),
            axis: "binary-counter 4".into(),
            metrics: crate::circuits::parse_metrics("latency 14 gt, edge rising, per bit 3.8 blocks"),
            inputs: vec!["clk".into()],
            outputs: vec!["q0".into(), "q1".into(), "q2".into(), "q3".into()],
            ..Default::default()
        };
        assert_eq!(bc.metrics[2], ("per bit".to_string(), "3.8 blocks".to_string()));
        let b = Build { circuit: Some(bc.clone()), blocks: 15, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "counts presses from 0 to 15 in binary · 15 blocks");
        assert!(card.sub.contains("3.8 blocks per bit · reacts in 14 ticks (0.7 s)"), "{}", card.sub);
        assert_eq!(one_liner(&bc), "Each press of its button adds one to the count, shown in binary on its 4 outputs (Q0 counts 1, Q1 counts 2, Q2 counts 4 and Q3 counts 8): it counts from 0 to 15, then starts again at 0.");
        assert!(try_it(&bc, true).starts_with("Press CLK under the view, again and again"));
        let dec = Circuit { kind: "decoder 2".into(), axis: "decoder 2".into(), metrics: crate::circuits::parse_metrics("latency 8 gt (a0 rises from a=00), glitches 3"), ..Default::default() };
        let bd = Build { circuit: Some(dec.clone()), blocks: 24, ..Default::default() };
        assert_eq!(site_model::html::props(&bd).title, "lights 1 of 4 outputs from a 2-bit number · 24 blocks");
        assert!(one_liner(&dec).starts_with("Its 2 inputs make a number from 0 to 3 in binary (A0 counts 1, A1 counts 2), and exactly one of its 4 outputs is lit"));
        assert_eq!(site_model::html::value_words("decoder 2", "decoder 2"), "2 inputs to 4 outputs");
        assert_eq!(site_model::html::value_words("binary-counter 4", "binary-counter 4"), "4 bits");
    }

    #[test]
    fn a_flip_safe_extender_is_linked_from_its_size_pick() {
        // (the coordinator 2026-10-21: "safe to flip back any time: N blocks bigger", like the TNT
        // no-switch variants)
        let mk = |name: &str, n: i64, blocks: u32, volume: u32, flips: Option<&str>| {
            let metrics: Vec<(String, String)> = std::iter::once(("extend".to_string(), "9 gt".to_string())).chain(flips.map(|f| ("flips".to_string(), f.to_string()))).collect();
            let flags = crate::circuits::test_flags(&metrics.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>()).into_iter().chain(["robust".to_string()]).collect();
            Build {
                id: format!("circuits/pext/{name}"),
                kind: "circuit".into(),
                name: name.into(),
                page: format!("circuits/pext/{name}.html"),
                blocks,
                volume: Some(volume),
                flags,
                circuit: Some(Circuit { kind: format!("piston-extender {n}"), study: "pext".into(), value: Some(n), axis: format!("N {n}"), robustness: "robust".into(), metrics, ..Default::default() }),
                ..Default::default()
            }
        };
        let mut builds = vec![
            mk("small2", 2, 8, 12, None),
            mk("safe2", 2, 10, 24, Some("safe (the lever flipped back at any time)")),
            mk("small3", 3, 18, 36, Some("breaks: off 20 gt after on strands blocks")),
        ];
        crate::kinds::select_circuits(&mut builds);
        let get = |n: &str| builds.iter().find(|b| b.name == n).cloned();
        let small2 = get("small2").expect("the reach 2 pick stays");
        assert_eq!(small2.circuit.as_ref().unwrap().flip_safe.as_deref(), Some("circuits/pext/safe2"));
        let safe2 = get("safe2").expect("the flip-safe design keeps its page");
        assert!(safe2.flags.iter().any(|f| f == "size"));
        assert_eq!(safe2.circuit.as_ref().unwrap().claim, "safe to flip back any time, 2 blocks bigger than the pick of its size");
        // reach 3 has no flip-safe design: no link
        assert!(get("small3").unwrap().circuit.as_ref().unwrap().flip_safe.is_none());
    }

    #[test]
    fn an_automatic_door_says_how_long_you_wait() {
        // clk-obs's auto-door-lock-v6, as explore/clk-obs b6a20ab520 archives it
        let c = Circuit {
            kind: "auto-door".into(),
            axis: "2x2".into(),
            metrics: crate::circuits::parse_metrics("door 2x2, open wait 12 gt, close delay 34 gt"),
            ..Default::default()
        };
        assert_eq!(c.metrics[1], ("open wait".to_string(), "12 gt".to_string()));
        let b = Build { circuit: Some(c.clone()), blocks: 85, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "opens as you walk up, closes behind you · 85 blocks · you wait up to 0.6 s at the door");
        assert!(card.sub.contains("it closes 1.7 s after you pass") && !card.sub.contains("wait"), "{}", card.sub);
        assert_eq!(crate::kinds::kind_key(&b), "auto-door 2x2");
        assert_eq!(crate::explain::kind_title("auto-door 2x2"), "Automatic door: opens as you walk up, closes behind you (2×2 piston door)");
        assert!(one_liner(&c).starts_with("A 2×2 piston door that opens by itself when you walk up from either side"));
        assert!(try_it(&c, true).starts_with("The view shows the door as built"));
    }

    /// The obvious automatic door (clk-obs's auto-door-plates, the card until the reviewer's G3 on
    /// the 38-block columns door, the coordinator 2026-10-22) and lock-v12, its variant that stays
    /// open for someone standing in the doorway: the card, the reviewer's three page must-haves
    /// word for word, Try it, and the judge's reported stop as a metric of its own.
    #[test]
    fn an_automatic_door_says_what_happens_in_the_doorway() {
        let door = |metrics: &str| Circuit { kind: "auto-door".into(), axis: "2x2".into(), metrics: crate::circuits::parse_metrics(metrics), ..Default::default() };
        let plates = door("door 2x2, open wait 0 gt, close delay 27 gt, stop in doorway the door closes on them");
        assert_eq!(plates.metrics[3], ("stop in doorway".to_string(), "the door closes on them".to_string()));
        assert_eq!(site_model::html::door_stays_open(&plates), Some(false));
        let b = Build { circuit: Some(plates.clone()), blocks: 44, kinds: vec!["stone_pressure_plate".into()], ..Default::default() };
        assert_eq!(site_model::html::props(&b).title, "opens as you walk up, closes behind you · 44 blocks · no waiting, even sprinting");
        assert_eq!(
            one_liner(&plates),
            "A 2×2 piston door that opens by itself when you walk up from either side and closes behind you, even when two people walk through close together. Stand still in the doorway and the door closes on you and hurts you (half a heart per hurt) until you step out."
        );
        assert_eq!(warning(&b).as_deref(), Some("A mob on a plate opens it too: nothing in vanilla tells a player from a mob, so put the plates where mobs can't walk."));
        assert!(try_it(&plates, true).contains("it opens before you reach it, even at a sprint, and closes once you have passed. Walk on through: if you stop in the doorway, it closes on you."));
        let v12 = door("door 2x2, open wait 13 gt, close delay 40 gt, stop in doorway the door stays open");
        assert_eq!(site_model::html::door_stays_open(&v12), Some(true));
        assert!(one_liner(&v12).ends_with("It also stays open for someone standing in the doorway, but if someone else walks through while you stand there, it can still close on you and hurt you (half a heart per hurt) until you step out."));
        let bv = Build { circuit: Some(v12.clone()), blocks: 91, kinds: vec!["stone_pressure_plate".into(), "tripwire".into(), "tripwire_hook".into()], ..Default::default() };
        let w = warning(&bv).unwrap_or_default();
        assert!(w.starts_with(MOB_PLATES) && w.contains("Its outer rows are tripwire strings, which a dropped item presses too: keep items off them.") && !w.contains("enclose"), "{w}");
        assert!(try_it(&v12, true).contains("(you may wait up to 0.65 s at the door)") && try_it(&v12, true).ends_with("Stop in the doorway and it stays open for you."));
        assert_eq!(IRON_DOORS, "If the entrance doesn't need to be hidden, two iron doors with pressure plates do the job in about 10 blocks.");
        // (no door says it never closes on someone in the doorway any more)
        assert!(!crate::explain::kind_summary("auto-door 2x2").contains("doorway"));
        assert!(crate::explain::metric_definition("auto-door", "stop in doorway").contains("half a heart per hurt"));
    }

    #[test]
    fn a_sequence_lock_says_its_code() {
        // clk-item's seqlock-2x3 (explore/tff 2f7127b10): its code frames read a b a
        let text = "circuit: sequence-lock 2 3\nport input.a: 34 1 0 button\nport input.b: 36 1 0 button\nport input.k0: 7 1 24 frame\nport input.k1: 17 1 24 frame\nport input.k2: 27 1 24 frame\nport output: 32 2 17\nentity: minecraft:item_frame 7.5 1.5 24.5 {Facing:3b,Item:{id:\"minecraft:arrow\",count:1},ItemRotation:0b}\nentity: minecraft:item_frame 17.5 1.5 24.5 {Facing:3b,Item:{id:\"minecraft:arrow\",count:1},ItemRotation:1b}\nentity: minecraft:item_frame 27.5 1.5 24.5 {Facing:3b,Item:{id:\"minecraft:arrow\",count:1},ItemRotation:0b}\n";
        assert_eq!(crate::circuits::test_seq_code(text), "a b a");
        let c = Circuit {
            kind: "sequence-lock 2 3".into(),
            axis: "2 buttons, code 3".into(),
            metrics: crate::circuits::parse_metrics("buttons 2, code 3 presses, presses tried 84, opens within 18 gt, closes within 18 gt"),
            inputs: vec!["a".into(), "b".into(), "k0".into(), "k1".into(), "k2".into()],
            input_labels: vec!["A".into(), "B".into(), "K0".into(), "K1".into(), "K2".into()],
            seq_code: "a b a".into(),
            ..Default::default()
        };
        assert_eq!(c.metrics[2], ("presses tried".to_string(), "84".to_string()));
        let b = Build { circuit: Some(c.clone()), blocks: 753, ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "a code of 3 presses on 2 buttons · 753 blocks");
        assert!(card.sub.contains("opens within 0.9 s · closes within 0.9 s · 84 press sequences tested"), "{}", card.sub);
        assert_eq!(crate::kinds::kind_key(&b), "sequence-lock 2 3");
        assert!(one_liner(&c).starts_with("Press its 2 buttons in the order set by the item frames on its back, 3 presses"));
        assert!(try_it(&c, true).starts_with("With its frames as built the code is A, B, A: press them in that order under the view"), "{}", try_it(&c, true));
        assert!(one_liner(&c).contains("a wrong press sends it back to the start and doesn't count as a first press"), "{}", one_liner(&c));
        // (a sliding-window lock: the coordinator's main rule, clk-item 6cd5cf8dd)
        let sliding = Circuit { metrics: crate::circuits::parse_metrics("buttons 2, code 3 presses, semantics sliding window, presses tried 84, opens within 18 gt, closes within 18 gt"), ..c.clone() };
        assert!(one_liner(&sliding).contains("it opens as soon as its last 3 presses match the code, like a keypad"), "{}", one_liner(&sliding));
        assert!(try_it(&sliding, true).contains("a wrong press just becomes part of the next try"));
    }

    #[test]
    fn a_piston_bridge_says_its_gap() {
        // pext's bridge-slime-3 (explore/pext 354ad1b45), as its archive carries it
        let c = Circuit {
            kind: "piston-bridge 3".into(),
            axis: "gap 3".into(),
            value: Some(3),
            metrics: crate::circuits::parse_metrics("gap 3 cells, width 1 cell, extend 3 gt, retract 3 gt, walkway slime block, flips safe (the lever flipped back at any time)"),
            inputs: vec!["in".into()],
            ..Default::default()
        };
        let b = Build { circuit: Some(c.clone()), blocks: 6, flags: crate::circuits::test_flags(&[("flips", "safe (the lever flipped back at any time)")]), ..Default::default() };
        let card = site_model::html::props(&b);
        assert_eq!(card.title, "spans a 3-block gap · 6 blocks");
        assert!(card.sub.contains("out in 3 ticks (0.15 s), back in 3 ticks (0.15 s) · a slime walkway"), "{}", card.sub);
        assert!(card.chips.iter().any(|ch| ch == "safe to flip back any time"), "{:?}", card.chips);
        assert_eq!(one_liner(&c), "Flip the lever on and a row of slime blocks slides out of the wall across a 3-block gap; flip it off and the row slides back flush into the wall, leaving the gap open. It goes out in 3 ticks (0.15 s) and comes back in 3 ticks (0.15 s).");
        assert!(crate::kinds::parametric(&c.kind));
        assert_eq!(site_model::html::value_words("piston-bridge 3", "gap 3"), "a 3-block gap");
        assert_eq!(crate::explain::kind_title("piston-bridge 3"), "Piston bridge: slides out across a gap at the flip of a lever (hidden bridge)");
        // pext 48056ce06: the 1-wide stone walkway beside the slime one of its gap, linked both
        // ways; the 2-wide bridges their own card
        let mk = |name: &str, blocks: u32, volume: u32, walkway: &str, width: u32| {
            let metrics = crate::circuits::parse_metrics(&format!("gap 3 cells, width {width} cell{}, extend 3 gt, retract 3 gt, walkway {walkway}, flips safe (the lever flipped back at any time)", if width == 1 { "" } else { "s" }));
            Build {
                id: format!("circuits/bridge/{name}"),
                kind: "circuit".into(),
                name: name.into(),
                page: format!("circuits/bridge/{name}.html"),
                blocks,
                volume: Some(volume),
                flags: vec!["robust".into(), "flip-safe".into()],
                circuit: Some(Circuit { kind: "piston-bridge 3".into(), study: "bridge".into(), value: Some(3), axis: if width == 1 { "gap 3".into() } else { format!("gap 3, {width} cells wide") }, robustness: "robust".into(), metrics, ..Default::default() }),
                ..Default::default()
            }
        };
        let mut builds = vec![mk("bridge-slime-3", 6, 15, "slime block", 1), mk("bridge-stone-3", 9, 20, "stone", 1), mk("bridge-stone-2wide-3", 38, 96, "stone", 2)];
        assert_eq!(crate::kinds::kind_key(&builds[2]), "piston-bridge 2wide");
        assert_eq!(crate::explain::kind_title("piston-bridge 2wide"), "Piston bridge, 2 blocks wide: slides out across a gap at the flip of a lever (hidden bridge)");
        crate::kinds::select_circuits(&mut builds);
        let get = |n: &str| builds.iter().find(|b| b.name == n).cloned().expect(n);
        assert_eq!(get("bridge-slime-3").circuit.unwrap().alt_walkway.as_deref(), Some("circuits/bridge/bridge-stone-3"));
        let stone = get("bridge-stone-3");
        assert_eq!(stone.circuit.as_ref().unwrap().alt_walkway.as_deref(), Some("circuits/bridge/bridge-slime-3"));
        assert_eq!(stone.circuit.as_ref().unwrap().claim, "a stone walkway, 3 blocks more than the pick of its gap");
        assert!(stone.flags.iter().any(|f| f == "size"));
    }

    #[test]
    fn adders_say_which_adder() {
        // (the adder study, w86: every design is `adder 1`, its axis says which)
        let mk = |axis: &str, inputs: &[&str]| Circuit { kind: "adder 1".into(), axis: axis.into(), inputs: inputs.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        let half = mk("half adder", &["a0", "b0"]);
        let full = mk("full adder", &["a0", "b0", "cin"]);
        let slice = mk("4-bit adder slice", &["a0", "b0", "cin"]);
        assert!(one_liner(&half).starts_with("Adds two one-bit inputs"), "{}", one_liner(&half));
        assert!(one_liner(&full).contains("a carry in (CIN)"), "{}", one_liner(&full));
        assert!(one_liner(&slice).starts_with("A full adder made to be copied"), "{}", one_liner(&slice));
        assert!(try_it(&slice, true).contains("stack copies side by side"));
        assert_eq!(site_model::html::props(&Build { circuit: Some(half.clone()), blocks: 17, ..Default::default() }).title, "half adder: two bits into a sum and a carry · 17 blocks");
        // no axis: a carry input says full
        assert_eq!(site_model::html::adder_variant(&mk("", &["a0", "b0", "cin"])), "full");
        assert_eq!(site_model::html::adder_variant(&mk("", &["a0", "b0"])), "half");
        // a slice's card: its blocks per bit and its time per bit (the judge's `slice` metric)
        let mut s = slice.clone();
        s.metrics = vec![("latency".into(), "18 gt".into()), ("slice".into(), "54 blocks per bit, latency 18 + 12 (N-1) gt".into())];
        let card = site_model::html::props(&Build { circuit: Some(s), blocks: 54, ..Default::default() });
        assert!(card.sub.contains("54 blocks per bit · reacts in 18 ticks (0.9 s), 12 ticks (0.6 s) more for each bit after the first"), "{}", card.sub);
    }

    #[test]
    fn plain_sentences_carry_the_numbers() {
        let mono = circuit("monostable retriggerable", &[("L", "32 gt")]);
        assert_eq!(one_liner(&mono), "One press keeps the output on for 32 ticks (1.6 s); pressing again while it is on restarts the countdown.");
        assert_eq!(ticks("1 gt"), "1 tick (0.05 s)");
        assert_eq!(ticks("4-8 gt"), "4 to 8 ticks");
        assert_eq!(ticks_short("28 gt (from the press)"), "28 ticks");
        let ext = circuit("piston-extender 3", &[("extend", "27 gt"), ("retract", "56 gt (from the press)")]);
        assert!(one_liner(&ext).contains("pushed 3 blocks out") && one_liner(&ext).contains("comes back in 56 ticks"), "{}", one_liner(&ext));
        let gate = circuit("gate and", &[("function", "and"), ("latency", "0 gt")]);
        assert!(one_liner(&gate).contains("both inputs") && one_liner(&gate).contains("instant"));
    }
}
