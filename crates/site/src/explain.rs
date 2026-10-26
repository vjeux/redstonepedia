//! Plain-language explanations for the circuit pages (vjeux, 01:05Z on a counter's page: "Very
//! unclear what that is and what to do"): what each kind of circuit does (with this design's
//! numbers), how to try it in the viewer, what every metric means, the studies' introductions,
//! gate truth tables, and the timing diagram of the judge's test.

use site_model::html::esc;
use site_model::{Build, Circuit, Wave, real_time};

/// `4 gt (0.2 s)`, from a metric value like `4 gt`.
fn with_seconds(v: &str) -> String {
    match v.trim().strip_suffix(" gt").and_then(|n| n.trim().parse::<u32>().ok()) {
        Some(n) => format!("{n} gt ({})", real_time(n)),
        None => v.to_string(),
    }
}

/// A metric of the circuit (`L`, `delay`, `latency`...), empty when absent.
fn metric<'a>(c: &'a Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// The kind's family word: `counter 4` -> `counter`, `gate xor` -> `gate`.
fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or(kind)
}

/// The number after the kind (`counter 4` -> 4).
fn kind_number(kind: &str) -> Option<u32> {
    kind.split_whitespace().nth(1).and_then(|n| n.parse().ok())
}

/// The gate function of a circuit: its `function` metric, else the kind's argument.
fn gate_function(c: &Circuit) -> String {
    let m = metric(c, "function");
    if !m.is_empty() {
        return m.to_string();
    }
    c.kind.split_whitespace().nth(1).unwrap_or("").to_string()
}

/// When a 2-input gate's output is on, in words (inputs A and B).
pub fn gate_rule(f: &str) -> &'static str {
    match f {
        "not" => "the opposite of the input: on while the input is off, off while it is on",
        "buffer" => "the same as the input",
        "and" => "on only while A and B are both on",
        "or" => "on while A or B (or both) is on",
        "xor" => "on while exactly one of A and B is on (not both)",
        "xnor" => "on while A and B are the same (both on or both off)",
        "nand" => "off only while A and B are both on (NOT AND)",
        "nor" => "on only while A and B are both off (NOT OR)",
        "imply" => "on unless A is on and B is off (A implies B)",
        "nimply" => "on only while A is on and B is off (A AND NOT B)",
        "imply-ba" => "on unless B is on and A is off (B implies A)",
        "nimply-ba" => "on only while B is on and A is off (B AND NOT A)",
        "not-a" => "the opposite of A (B is ignored)",
        "not-b" => "the opposite of B (A is ignored)",
        _ => "a fixed function of the inputs (see the truth table)",
    }
}

/// The truth table of a gate function as an HTML table (inputs in name order: A, B).
pub fn truth_table(f: &str, inputs: &[String]) -> String {
    let on = |v: bool| if v { "<td class=\"on\">on</td>" } else { "<td class=\"off\">off</td>" };
    if f == "not" || f == "buffer" || inputs.len() == 1 {
        let name = inputs.first().map(|s| s.to_uppercase()).unwrap_or_else(|| "IN".into());
        let rows: String = [false, true].iter().map(|&a| format!("<tr>{}{}</tr>", on(a), on(if f == "buffer" { a } else { !a }))).collect();
        return format!("<table class=\"truth\"><thead><tr><th>{}</th><th>OUT</th></tr></thead><tbody>{rows}</tbody></table>", esc(&name));
    }
    let Some((_, t)) = circuit::GATES2.iter().find(|(n, _)| *n == f) else { return String::new() };
    let (a, b) = (inputs.first().map(|s| s.to_uppercase()).unwrap_or_else(|| "A".into()), inputs.get(1).map(|s| s.to_uppercase()).unwrap_or_else(|| "B".into()));
    let rows: String = (0..4).map(|i| format!("<tr>{}{}{}</tr>", on(i >= 2), on(i % 2 == 1), on(t[i]))).collect();
    format!("<table class=\"truth\"><thead><tr><th>{}</th><th>{}</th><th>OUT</th></tr></thead><tbody>{rows}</tbody></table>", esc(&a), esc(&b))
}

/// The kind's name for people (`counter 4` -> `Counter (divide by 4)`).
/// A kind's name for people, its base only (vjeux 12:15Z: "Circuit kind should be piston
/// extender, not one per N"): the number (`piston-extender 5`, `divider 3`, `gate and`) is a
/// property, shown in the property line; a short description in parentheses where it helps.
pub fn kind_title(kind: &str) -> String {
    // a kind already in words (a circuit's group): its title from the matching kind id
    let id = if kind.starts_with(|c: char| c.is_uppercase()) { kind_id(kind) } else { kind.to_string() };
    if id.is_empty() {
        return kind.to_string();
    }
    // a registered kind's own words (circuit::registry)
    if let Some((_, d)) = circuit::registry::find(base(&id)) {
        return (d.site.title)(&id);
    }
    // what it does first, the community's name in parentheses (vjeux 13:06Z; critique P1)
    let words = site_model::html::kind_words(&id);
    match base(&id) {
        "rising-edge" => "One pulse when switched on (rising-edge detector)".into(),
        "falling-edge" => "One pulse when switched off (falling-edge detector)".into(),
        "dual-edge" => "One pulse on every switch (dual-edge detector)".into(),
        "pulse-limiter" => "Signal shortener (pulse limiter)".into(),
        "pulse-extender" => "Signal stretcher (pulse extender)".into(),
        "tff" => "Toggle switch: press once on, press again off (T flip-flop)".into(),
        "tff-reset" => "Toggle switch with a reset button (T flip-flop with reset)".into(),
        "delay" => "Delay (delay line)".into(),
        "rs-latch" => "On button + off button memory (RS latch)".into(),
        "d-latch" => "Copy-and-hold memory (D latch)".into(),
        "dff" => "Copies its input on each clock pulse (D flip-flop)".into(),
        "dff-enable" => "Copies its input on each clock pulse when enabled (D flip-flop with enable)".into(),
        "shift" => "Moves bits along on each clock pulse (shift register)".into(),
        "counter" => "Counter: counts pulses and reacts on a set count".into(),
        // (pext's piston bridges: `piston-bridge 2wide` for a walkway 2 blocks wide)
        "piston-bridge" => match id.split_whitespace().nth(1).and_then(|w| w.strip_suffix("wide")) {
            Some(w) => format!("Piston bridge, {w} blocks wide: slides out across a gap at the flip of a lever (hidden bridge)"),
            None => "Piston bridge: slides out across a gap at the flip of a lever (hidden bridge)".into(),
        },
        // (w86's computational circuits)
        "binary-counter" => "Binary counter: shows how many times it was pressed, in binary".into(),
        "decoder" => "Decoder: lights exactly one output, the one its input number names (binary decoder)".into(),
        "mux" => "Multiplexer: passes on the one input its select lines pick (mux)".into(),
        "comparator" => "Number comparator: tells whether A is less than, equal to or greater than B (magnitude comparator)".into(),
        // (it notices changes, not pure block updates: critique, the kind line said "block update
        // detector" over a page warning it does not notice pure updates)
        "bud" => "Block change detector (players call it a BUD)".into(),
        "tnt-duper" => "TNT duper (runs by itself)".into(),
        "tnt-duper-triggered" => "TNT duper, one TNT at a time".into(),
        "burst" => "Several pulses per press (burst)".into(),
        "divider" => "Slows a clock down (clock divider)".into(),
        "monostable" => match id.split_whitespace().nth(1) {
            Some("retriggerable") => "One-shot timer that restarts on every press (retriggerable monostable)".into(),
            Some("non-retriggerable") => "One-shot timer that ignores presses while on (non-retriggerable monostable)".into(),
            _ => "One-shot timer (monostable)".into(),
        },
        "off-delay" => "Stay-on timer (off-delay)".into(),
        "on-delay" => "Hold-to-activate: ignores short presses (on-delay)".into(),
        "pulse-train" => "Rapid fire while on (gated clock)".into(),
        "sorter" => "Item sorter: one item kind out of a line of mixed items (hopper filter)".into(),
        "lock" => "Combination lock: item frames as dials, opens only at the code".into(),
        "sequence-lock" => "Button sequence lock: opens when its buttons are pressed in the right order (password door)".into(),
        // (clk-item's study 16)
        "randomizer" => "Random picker: each press lights one output at random (randomizer)".into(),
        // (w86's study 17)
        "harvester" => format!("{} farm: harvests by itself (automatic farm)", first_cap(&id.strip_prefix("harvester ").unwrap_or("crop").replace('_', " "))),
        // the adder study (w86): the section's title (one section: `adder`) and each kind's
        "adder" => match id.split_whitespace().nth(1) {
            Some("half") => "Half adder: adds two bits into a sum and a carry".into(),
            Some("full") => "Full adder: adds two bits and a carry".into(),
            Some("slice") => "Adder slice: full adders that stack into an adder of any width (ripple-carry adder)".into(),
            // (the study's one section: "Adder", which the page's own title already says)
            _ => words,
        },
        "smelter" if id.ends_with("tileable") => "Auto-smelter row: slices side by side, as many furnaces as you need".into(),
        "smelter" => "Auto-smelter: furnaces fed from one input chest and one fuel chest".into(),
        // (clk-misc's study)
        "item-elevator" => "Item elevator: lifts the items of a chest up into another (dropper elevator)".into(),
        // (the fly study's player elevators)
        "elevator" => "Player elevator: a platform carries you between two floors (flying-machine elevator)".into(),
        // (clk-obs's study 19)
        "station" if id.ends_with(" 2") => "Minecart station on a two-way line: stops the carts going its way, lets the others through".into(),
        "station" if id.ends_with(" 1") => "Minecart station on a one-way line: stops every cart, a button sends it on".into(),
        // (the study's one section, both lines)
        "station" => "Minecart station: stops a cart at the platform, a button sends it on".into(),
        // (clk-obs's hopper-minecart unloaders and loaders: `cart-unloader terminus` ...)
        "cart-unloader" if id.ends_with("through") => "Minecart unloader: empties a hopper minecart into a chest, then sends it on".into(),
        "cart-unloader" => "Minecart unloader: empties a hopper minecart into a chest, then sends the cart back".into(),
        "cart-loader" if id.ends_with("through") => "Minecart loader: fills a hopper minecart from a chest, sends it on when full or when the chest runs dry (an empty cart waits for items)".into(),
        "cart-loader" => "Minecart loader: fills a hopper minecart from a chest, sends it off when full or when the chest runs dry (an empty cart waits for items)".into(),
        // (sorter's whole lines: a loader, its track and an unloader, crate::cartsys)
        "cart-line" => "Minecart line: carries a chest's items to a chest far away, by itself (minecart item transport)".into(),
        // (clk-obs's automatic doors: `auto-door 2x2`, one card per door size)
        "auto-door" => match id.split_whitespace().nth(1) {
            Some(s) if s.contains('x') => format!("Automatic door: opens as you walk up, closes behind you ({} piston door)", s.replace('x', "×")),
            _ => "Automatic door: opens as you walk up, closes behind you (piston door)".into(),
        },
        // (clk-item's auto-crafters: one kind per recipe; the study's section)
        "crafter" => match id.strip_prefix("crafter ").map(str::trim) {
            Some(r) if !r.is_empty() => {
                let w = r.trim_start_matches("minecraft:").replace('_', " ");
                // (TNT is written in capitals)
                let w = if w == "tnt" { "TNT".to_string() } else { first_cap(&w) };
                format!("{w} crafter: crafts by itself from a supply chest (autocrafter)")
            }
            _ => "Auto-crafter: crafts by itself from a supply chest (autocrafter)".into(),
        },
        // (clk-obs's readable 0-9 display: one strength input, a 3×5 face)
        "seven-segment" if id.split_whitespace().any(|w| w == "decimal") => "7-segment display: one digit, 0 to 9, from a container's fill level".into(),
        _ => words,
    }
}

/// The kind id a kind's everyday name stands for (`One-shot timer` -> `monostable`).
fn kind_id(words: &str) -> String {
    const IDS: [&str; 49] = [
        "rising-edge", "falling-edge", "dual-edge", "pulse-limiter", "pulse-extender", "tff", "tff-reset", "delay", "gate", "rs-latch", "d-latch", "dff", "dff-enable", "shift", "counter", "adder",
        "piston-extender", "flying-machine", "shulker-loader", "shulker-unloader", "bud", "tnt-duper", "tnt-duper-triggered", "burst", "divider", "monostable", "off-delay", "on-delay", "pulse-train", "sorter", "smelter",
        "randomizer", "harvester", "item-elevator", "station", "crafter", "elevator", "cart-unloader", "cart-loader", "binary-counter", "decoder", "mux", "comparator", "auto-door", "sequence-lock", "piston-bridge", "brewer", "composter", "cart-line",
    ];
    IDS.iter().copied().chain(circuit::registry::all().map(|(_, d)| d.word)).find(|id| site_model::html::kind_words(id) == words).map(|s| s.to_string()).unwrap_or_default()
}

/// A robustness word whose varying build differs from the base build only in numbers (`position-
/// dependent (rotation 0 at [5, -11, -6]: line terminus, load rate 17.05 items/s, departure delay
/// 17 gt (base build: line terminus, load rate 16.87 items/s, departure delay 17 gt))`): every
/// such metric as (name, base value, other value); None when a word differs too, or it cannot be
/// read.
pub fn numbers_only_variation(r: &str) -> Option<Vec<(String, String, String)>> {
    let (head, base) = r.split_once(" (base build: ")?;
    let base = base.trim_end().strip_suffix("))")?;
    let other = head.split_once("]: ").map(|(_, o)| o)?;
    let (a, b) = (crate::circuits::parse_metrics(other), crate::circuits::parse_metrics(base));
    if a.is_empty() || a.len() != b.len() {
        return None;
    }
    let words = |v: &str| v.split_whitespace().filter(|w| w.parse::<f64>().is_err()).map(String::from).collect::<Vec<_>>();
    let mut diffs = Vec::new();
    for ((ka, va), (kb, vb)) in a.iter().zip(&b) {
        if ka != kb || words(va) != words(vb) {
            return None;
        }
        if va != vb {
            diffs.push((ka.clone(), vb.clone(), va.clone()));
        }
    }
    (!diffs.is_empty()).then_some(diffs)
}

/// A design's "works in any direction, anywhere" line (vjeux 13:08Z: "robustness feels wrong"):
/// the community's terms, directional / locational.
pub fn direction_words(robustness: &str) -> String {
    let r = robustness.trim();
    if r.is_empty() {
        return String::new();
    }
    if r == "robust" {
        return "yes: not directional or locational (the same behavior facing all 4 ways and at 4 different spots in the world, in the real game)".to_string();
    }
    // (a design that passes the same way everywhere and differs only in a number: sorter's
    // ld-chest, "position-dependent" for a load rate of 17.05 at one spot against 16.87: no
    // "locational" headline, the numbers said)
    if let Some(diffs) = numbers_only_variation(r) {
        let what: Vec<String> = diffs.iter().map(|(k, base, other)| format!("its {k} came out {base} or {other} depending on the spot")).collect();
        return format!("yes: it passed the same way facing all 4 ways and at 4 different spots in the real game; only {}", what.join(", and "));
    }
    let rotation = r.starts_with("rotation");
    let which = r.split("rotation ").nth(1).and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next()).and_then(|d| d.parse::<u32>().ok());
    let facing = match which {
        Some(1) => " (breaks when turned a quarter)",
        Some(2) => " (breaks when turned around)",
        Some(3) => " (breaks when turned three quarters)",
        _ => "",
    };
    if rotation {
        format!("no: directional, it depends on which way it faces{facing}")
    } else if r.contains("position") || r.contains("offset") {
        "no: locational, it depends on where it is built".to_string()
    } else {
        format!("no: {}", r.split(':').next().unwrap_or(r))
    }
}

/// `64` -> `64-stack items`, `1` -> `unstackable items`.
fn stack_words(stack: u32) -> String {
    if stack <= 1 { "unstackable items".to_string() } else { format!("{stack}-stack items") }
}

/// What a box holds of items of that max stack, and an example item.
fn box_items(stack: u32) -> (u32, &'static str) {
    match stack {
        0 | 1 => (27, "iron shovels"),
        16 => (432, "snowballs"),
        _ => (27 * stack, "cobblestone"),
    }
}

/// What a kind does in general (no numbers): the study pages' kind list.
pub fn kind_summary(kind: &str) -> String {
    if let Some((_, d)) = circuit::registry::find(base(kind)) {
        return (d.site.summary)(kind);
    }
    match base(kind) {
        "rising-edge" => "when the input turns on, the output gives one short pulse; nothing when it turns off".into(),
        "falling-edge" => "when the input turns off, the output gives one short pulse; nothing when it turns on".into(),
        "dual-edge" => "a short output pulse every time the input changes, on or off".into(),
        "pulse-limiter" => "the output pulse is cut to a set length, however long the input stays on".into(),
        "pulse-extender" => "a short input pulse comes out at least a set length long".into(),
        "tff" => "each press flips the output: on, off, on, off... (a push-button switch)".into(),
        "delay" => "the output is the input, a set number of ticks later (every pulse comes out the same length)".into(),
        "gate" => "the output follows a fixed rule of its inputs: NOT, AND, OR, XOR...; the two-input gates call their inputs A and B".into(),
        "rs-latch" => "a pulse on the set input (S) turns the output on and it stays on; a pulse on the reset input (R) turns it off".into(),
        "d-latch" => "the output copies the data input (D) while the enable input allows it, and keeps its last value otherwise".into(),
        "dff" => "on one edge of the clock input the output takes the value of the data input D, and holds it until the next such edge (a D flip-flop)".into(),
        "dff-enable" => "a D flip-flop with an enable input: on the clock's edge it takes the data input only while the enable input is powered, otherwise it keeps its value".into(),
        "shift" => "a row of memory cells: each clock pulse moves every bit one step along, so the data comes out a set number of clock pulses later".into(),
        "tff-reset" => "a toggle switch with a reset input: each press flips the output, a press on R turns it off".into(),
        "adder" => match kind.split_whitespace().nth(1) {
            // (the adder study, w86: which adder it is, `adder half` ...)
            Some("half") => "adds two one-bit inputs, A and B: the sum output (S) is on when exactly one of them is on, the carry output (COUT) when both are".into(),
            Some("full") => "adds two bits, A and B, and a carry in (CIN): the sum output (S) is on when one or three of them are on, the carry output (COUT) when two or more are".into(),
            Some("slice") => "a full adder made to be copied: stack copies side by side and each copy's carry out feeds the next copy's carry in, so N copies add two N-bit numbers (a ripple-carry adder)".into(),
            _ => "adds two binary numbers, bit by bit with the carry".into(),
        },
        "randomizer" => "each press lights exactly one of its outputs, picked at random, every output as often as the others".into(),
        "harvester" => "harvests its crop by itself: every block that grows is broken and its item sent into a chest, forever, with no replanting".into(),
        "seven-segment" if kind.split_whitespace().any(|w| w == "decimal") => "one comparator reads a chest (or any container), and its 3×5 lamp face shows the signal as a digit, 0 to 9: the fuller the container, the higher the digit".into(),
        "seven-segment" => "four input lines (a 4-bit number) light the seven segments of its digit on a display: 0 to 9, then A b C d E F".into(),
        "counter" => "counts input pulses and changes its output once every few pulses (a set count)".into(),
        "binary-counter" => "each press adds one to the count its outputs show in binary (Q0 the lowest bit)".into(),
        "decoder" => "its inputs make a number in binary; exactly one output is lit, the one with that number".into(),
        "mux" => "its select lines make a number in binary; the output follows the data input with that number".into(),
        "comparator" => "compares two binary numbers, A and B: one of three outputs lights, A less than B, A equal to B, or A greater than B".into(),
        "piston-extender" => "one input pushes a block several blocks out, the next brings it back exactly".into(),
        "piston-bridge" => "flip its lever and a row of blocks slides out of the wall across the gap; flip it back and the row slides back flush, leaving the gap open".into(),
        "flying-machine" => "moves itself through the world, one block at a time, once started".into(),
        "shulker-loader" => "fills empty shulker boxes from a chest of items and sends each box out once it is full".into(),
        "shulker-unloader" => "empties full shulker boxes into a chest of items and sends the empty boxes out".into(),
        "bud" => "fires its output when the block at its target cell is placed, removed or changed, without any redstone signal reaching it".into(),
        "tnt-duper" => "keeps lighting TNT forever without using up its own TNT block".into(),
        "tnt-duper-triggered" => "lights one TNT each time it is triggered, without using up its TNT block".into(),
        "burst" => "turns one press into an exact number of pulses".into(),
        "divider" => "the output goes on and off once for every few input pulses, even from a fast clock".into(),
        "monostable" => "gives one output pulse of a set length for each press, however short the press".into(),
        "off-delay" => "keeps the output on for a set time after the input turns off".into(),
        "on-delay" => "turns the output on only once the input has been on for a set time, so shorter presses are ignored".into(),
        "pulse-train" => "pulses the output at a set beat for as long as the input is on".into(),
        "sorter" => "takes every item of one kind out of a line of mixed items into its chest and lets everything else go on; slices side by side sort one kind each".into(),
        "lock" => "turns its output on only while every item-frame dial shows the same position as its own code frame; set the code by turning the code frames".into(),
        "sequence-lock" => "opens when its buttons are pressed in the order set by the item frames on its back, and closes at the next press".into(),
        "smelter" => "spreads the items of its input chest over its furnaces, feeds them coal from its fuel chest and collects every result in its output chest".into(),
        "brewer" => "brews the water bottles of its bottles chest into potions of swiftness, a nether wart first and a sugar after it, burns blaze powder from its fuel chest and collects every finished potion in its output chest".into(),
        "composter" => "takes the crops out of a line of mixed farm output, one crop a slice, composts them and collects the bone meal in a chest under each composter; everything else runs on to the overflow chest".into(),
        "item-elevator" => "lifts every item of its input chest straight up into its output chest, a set number of blocks higher, as fast as a hopper feeds them".into(),
        "elevator" => "a platform pushed by a flying machine carries a player between two floors; a button on each floor calls it, or sends it away when it is there".into(),
        "station" if kind.ends_with(" 2") => "a cart arriving the station's way stops at the platform, ridden or empty, and waits for the button; carts coming the other way pass straight through".into(),
        "station" if kind.ends_with(" 1") => "a cart arriving stops at the platform, ridden or empty, and waits; a press of the button sends it on its way".into(),
        "station" => "a cart arriving stops at the platform, ridden or empty, and waits; a press of the button sends it on its way (on a two-way line, carts coming the other way pass straight through)".into(),
        "crafter" => "crafts by itself: hoppers fill its crafter from a supply chest, and every full recipe is crafted into the output chest".into(),
        "cart-unloader" => "a hopper minecart full of items arrives, stops, empties every item into the chest and leaves on its own once it is empty".into(),
        "cart-loader" => "an empty hopper minecart arrives, stops, fills from the supply chest and leaves on its own once it is full, or once the chest has run dry with items in the cart; an empty cart waits at an empty chest until items come".into(),
        "cart-line" => "one hopper minecart shuttles by itself between a loader and an unloader at the two ends of a track, carrying a chest's items to a chest far away; with nothing to carry, it waits at the loader".into(),
        "auto-door" => "a piston door that opens by itself when someone walks up from either side and closes once they have passed".into(),
        _ => String::new(),
    }
}

/// The paragraph at the top of a circuit page: what this circuit does, with its numbers.
pub fn what_it_does(c: &Circuit) -> String {
    if let Some(html) = what_machines_do(c) {
        return html;
    }
    let l = with_seconds(metric(c, "L"));
    let delay = with_seconds(metric(c, "delay"));
    let min_pulse = with_seconds(metric(c, "min-pulse"));
    let one_input_pulse = if min_pulse.is_empty() { String::new() } else { format!(" It handles every input pulse of {min_pulse} or longer.") };
    match base(&c.kind) {
        "rising-edge" => format!(
            "<p>When the input turns <b>on</b>, the output gives <b>one short pulse</b> of L = {l}, starting {delay} after the input turned on. When the input turns off, nothing happens.{one_input_pulse}</p><p>Use it to turn a lever (which stays on) into a single short pulse, e.g. to fire a piston once.</p>"
        ),
        "falling-edge" => format!(
            "<p>When the input turns <b>off</b>, the output gives <b>one short pulse</b> of L = {l}, starting {delay} after the input turned off. When the input turns on, nothing happens.{one_input_pulse}</p><p>Use it to react to something stopping, e.g. a lever being switched back.</p>"
        ),
        "dual-edge" => format!(
            "<p>Every time the input <b>changes</b> (turns on or turns off), the output gives <b>one short pulse</b> of L = {l}, {delay} after the change.{one_input_pulse}</p><p>Use it to react to any change of a lever, in both directions.</p>"
        ),
        "pulse-limiter" => format!(
            "<p>However long the input stays on, the output pulse lasts <b>at most L = {l}</b>; it starts {delay} after the input turns on. (An input shorter than L may come out shorter.){one_input_pulse}</p><p>Use it to cut a long signal down to a short, fixed pulse.</p>"
        ),
        "pulse-extender" => format!(
            "<p>Any input pulse, even a 1-tick one, gives an output pulse of <b>at least L = {l}</b>, starting {delay} after the input turns on.{one_input_pulse}</p><p>Use it to make a brief signal (an observer's 2-tick pulse) last long enough for a slower part.</p>"
        ),
        "tff" => {
            let edge = metric(c, "edge");
            let when = match edge {
                "rising" => "when the input turns on",
                "falling" => "when the input turns off",
                _ => "on each input pulse",
            };
            let latency = with_seconds(metric(c, "latency"));
            let min_period = with_seconds(metric(c, "min-period"));
            format!(
                "<p>Each input pulse <b>flips the output</b>: the first pulse turns it on, the next turns it off, and so on; between pulses the output stays put. It flips {when}{}.</p><p>It works like a push-button lamp switch: press once for on, press again for off.{}{}</p>",
                if latency.is_empty() { String::new() } else { format!(", {latency} later") },
                if min_pulse.is_empty() { String::new() } else { format!(" It accepts input pulses of {min_pulse} or longer") },
                if min_period.is_empty() { ".".to_string() } else { format!(" and follows pulses as fast as one every {min_period}.") }
            )
        }
        "delay" => {
            let n = with_seconds(metric(c, "N"));
            let min_gap = with_seconds(metric(c, "min-gap"));
            let one_edge = c.verdict.contains("one-edge");
            format!(
                "<p>The output is <b>a copy of the input, N = {n} later</b>: a pulse goes in, the same pulse comes out N later{}.</p><p>{}{}</p>",
                if one_edge { " (this design delays the two edges by different amounts: a one-edge delay)" } else { ", same length" },
                if min_pulse.is_empty() { String::new() } else { format!("Pulses shorter than {min_pulse} get lost. ") },
                if min_gap.is_empty() { String::new() } else { format!("Two pulses closer than {min_gap} merge.") }
            )
        }
        "gate" => {
            let f = gate_function(c);
            let latency = metric(c, "latency");
            let instant = latency.trim() == "0 gt";
            let n = if c.inputs.len() == 1 { "input" } else { "inputs" };
            format!(
                "<p>A logic gate: the output depends only on the {n} right now. <b>{}</b>: the output is {}.</p>{}<p>{}</p>",
                esc(&f.to_uppercase()),
                gate_rule(&f),
                truth_table(&f, &c.inputs),
                if instant {
                    "It is <b>instant</b>: the output changes in the same game tick as the inputs (0 gt).".to_string()
                } else if latency.is_empty() {
                    String::new()
                } else {
                    format!("The output settles {} after an input changes.", with_seconds(latency))
                }
            )
        }
        "rs-latch" => {
            let qbar = c.outputs.iter().any(|o| o == "qbar");
            format!(
                "<p>A one-bit memory. A pulse on <b>S</b> (set) turns the output <b>on</b>, and it stays on; a pulse on <b>R</b> (reset) turns it <b>off</b>, and it stays off. It remembers which one came last.{}</p>",
                if qbar { " QBAR is always the opposite of Q." } else { "" }
            )
        }
        "d-latch" => {
            let high = metric(c, "enable") != "low";
            let (active, inactive) = if high { ("on", "off") } else { ("off", "on") };
            format!(
                "<p>A one-bit memory with a data input <b>D</b> and an enable input <b>E</b>. While E is <b>{active}</b>, the output Q copies D; when E turns {inactive}, Q keeps the value D had and ignores D until E is {active} again (enable {}).</p>",
                if high { "high" } else { "low" }
            )
        }
        "counter" => {
            let n = kind_number(&c.kind).unwrap_or(0);
            let style = metric(c, "output");
            let how = if style.starts_with("toggles") {
                format!("the output <b>flips once every {n} input pulses</b>: on for {n} pulses, then off for {n}")
            } else if style.starts_with("pulses") {
                format!("the output gives <b>one pulse for every {n} input pulses</b>")
            } else {
                // level style: the judge's own run says in how many of the N windows it is on
                match c.counter_on {
                    Some((k, m)) if m == n && k > 0 && k < n => format!(
                        "the output's state <b>repeats every {n} input pulses</b>: after each pulse it is on for {k} of them and off for the other {}",
                        n - k
                    ),
                    _ => format!("the output's state <b>repeats every {n} input pulses</b>"),
                }
            };
            format!("<p>It <b>counts input pulses</b>: {how}. In other words, it divides the number of pulses by {n}.</p><p>Use it to do something every {} time, or to slow a fast clock down {n} times.</p>", ordinal(n))
        }
        "piston-extender" => {
            let n = kind_number(&c.kind).unwrap_or(0);
            let ext = with_seconds(metric(c, "extend"));
            let ret = with_seconds(metric(c, "retract"));
            format!(
                "<p>Turning the input on pushes a block <b>{n} cell{} out</b> of the machine; turning it off brings the block <b>back exactly</b> where it was, with nothing left behind. A plain piston only reaches 1.</p><p>{}{}</p>",
                if n == 1 { "" } else { "s" },
                if ext.is_empty() { String::new() } else { format!("Extending takes {ext}") },
                if ret.is_empty() { String::new() } else { format!(", retracting {ret}.") }
            )
        }
        "flying-machine" => {
            let speed = metric(c, "speed");
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            if two_way {
                "<p>A <b>two-way flying machine</b>: pistons, slime or honey and observers that push each other along, then come back. Once started it shuttles back and forth by itself.</p>".to_string()
            } else {
                let per_s = speed.strip_suffix(" gt/block").and_then(|n| n.trim().parse::<f64>().ok()).map(|g| format!(" ({:.1} blocks per second)", 20.0 / g)).unwrap_or_default();
                format!(
                    "<p>A <b>flying machine</b>: pistons, slime or honey and observers that push each other along. Once started (the start block is placed), it moves itself one block at a time and keeps going until something stops it.</p><p>{}</p>",
                    if speed.is_empty() { String::new() } else { format!("It moves one block every {}{per_s}.", esc(speed).replace(" gt/block", " gt")) }
                )
            }
        }
        // an automatic door (fly's QA: its In depth only said the kind again): how a door of
        // the kind senses walkers, closes behind them, and why none can feel someone standing in
        // the doorway (designs/briefs/auto-door.md, from the game code); each design's own
        // mechanism is its caption
        "auto-door" => "<p>A 2×2 piston door: four sticky pistons pull the four door blocks into the wall. Its detectors lie in the walkway on both sides of the door, where you step anyway: stone pressure plates, which feel players and mobs but never dropped items (a wooden plate would feel items too), or tripwire strings, which feel anything that crosses them. A plate powers the block under it, and the signal runs under the floor to the redstone that holds the pistons closed: step on a plate and the door opens.</p><p>A stone plate stays down for at least 1 second after it is pressed and checks again every second, letting go once nobody is on it, so the door closes a second or two after the last person has passed. The doorway itself is only one block deep: you cross it, between the plates on either side, in a fraction of a second.</p><p>Nothing can feel someone standing in the doorway: no detector fits there, since the pistons sweep it and a plate or a string pushed into pops off. A door with no memory, the simplest, closes on them; one with memory reads the order its plates were pressed in and stays open for them, for about twice the blocks.</p>".to_string(),
        // a whole minecart line (sorter's crate::cartsys, designs/briefs/cart-transport.md: the
        // rules from the game code): what the cart does at each end and on the way
        "cart-line" => {
            let every = metric(c, "track").split_once("a powered rail every ").and_then(|(_, n)| n.split(|ch: char| !ch.is_ascii_digit()).next()).filter(|n| !n.is_empty()).map(|n| format!("A powered rail every {n} blocks, kept on by a redstone torch, keeps the cart at full speed both ways. ")).unwrap_or_default();
            format!(
                "<p>A hopper minecart pulls one item a tick, 20 items a second, from a chest right above its rail, and holds 320 items (5 stacks). At the loader it stops under the supply chest and fills; it leaves when it is full, or when the chest has run dry and it holds something. Empty, at an empty chest, it stays put, so an idle farm's cart does not shuttle back and forth. At the other end, the unloader holds it over hoppers that empty it into the storage chest, then sends it back.</p><p>{every}A cart only moves in chunks the game is running: with a server's default simulation distance (10 chunks), a line of 128 blocks keeps both ends running while you stand at either one; a longer line works only while you stand near its middle.</p>"
            )
        }
        // a minecart stop's In depth (fly's QA: it said the kind again): where its speed comes
        // from (designs/briefs/cart-transport.md, from the game code)
        "cart-unloader" => "<p>A hopper minecart holds 320 items (5 stacks). A hopper under the cart pulls one item every 8 game ticks, 2.5 items a second, so an unloader is as fast as the hoppers that reach the cart: one under the cart empties a full one in about 2 minutes; two, by holding the cart where it straddles two blocks, in about 1. The station sends the cart back only once its hoppers find nothing more in it.</p>".to_string(),
        "cart-loader" => "<p>A hopper minecart holds 320 items (5 stacks) and pulls one item a tick, 20 items a second, from a chest right above its rail: stopped under the supply chest, it fills in 16 seconds. The station sends it off when it is full, or when the chest has run dry and it holds something; an empty cart at an empty chest stays, so an idle farm's cart does not shuttle back and forth.</p>".to_string(),
        _ => format!("<p>{}</p>", esc(&kind_summary(&c.kind))),
    }
}

/// The meanings of the shulker study's badges and the BUD classes (the variations tables and the
/// pages' "also passes" lists).
pub fn badge_meaning(b: &str) -> &'static str {
    match b {
        "16-stack" => "16-stack items too (snowballs, ender pearls, eggs: 432 a box)",
        "unstackable" => "unstackable items too (tools, armour: 27 a box)",
        "mixed" => "mixed items, full and partial stacks, packed as they come",
        "filter" => "packs one kind of item and passes the rest on",
        "tileable" => "copies side by side work together (one per item kind)",
        "overflow" | "overflow-proof" => "a full chest stops nothing: its items go on along the line, no other slice loses its filter, and it sorts again once emptied",
        "stream-direct" => "a water stream can end right on the first slice: its line hopper takes whole stacks and still lets none of its own kind past",
        "always-on" => "no lever: it runs by itself",
        "restart" => "when the items pause and come back, it carries on by itself",
        "box-starved" => "nothing is lost while the empty-box supply runs out; it resumes when boxes come back",
        "colours" => "boxes keep their colour and their name",
        "flush" => "sends out a part-full box when the input stops for good",
        "top-up" => "fills half-used boxes too",
        "drop-free" => "box items never lie on the ground",
        "combo" => "one station loads and unloads, switched by a lever",
        "any-box" => "empties every kind of box: full, half-full, mixed, already empty",
        "place" => "a block placed at the target",
        "remove" => "a block removed from the target",
        "state" => "the target's block changing state (a log turned)",
        "shape" => "a change that sends shape updates only, the way crops grow: observers see it, pistons do not",
        "update" => "a pure block update next to the target, nothing changing: pistons and dispensers see it (quasi-connectivity), observers do not",
        _ => "",
    }
}

/// The paragraphs of the studies of machines (shulker loaders, BUD switches, TNT dupers).
fn what_machines_do(c: &Circuit) -> Option<String> {
    let badges: Vec<String> = metric(c, "badges").split('+').filter(|s| !s.is_empty()).map(String::from).collect();
    let also = |what: &str| -> String {
        if badges.is_empty() {
            return String::new();
        }
        let items: String = badges.iter().map(|b| format!("<li><b>{}</b>: {}</li>", esc(b), esc(badge_meaning(b)))).collect();
        format!("<p>It also passes these {what} (each a separate run of the judge):</p><ul class=\"badgelist\">{items}</ul>")
    };
    let html = match base(&c.kind) {
        "shulker-loader" | "shulker-unloader" => {
            let stack = kind_number(&c.kind).unwrap_or(64);
            let (per_box, example) = box_items(stack);
            let loader = base(&c.kind) == "shulker-loader";
            let items_s = metric(c, "items/s");
            let cycle = metric(c, "cycle");
            let first = metric(c, "first box");
            let cycle_s = cycle.trim_end_matches(" gt").trim().parse::<u32>().ok().map(real_time).unwrap_or_default();
            let lead = if loader {
                format!(
                    "<p>It <b>packs items into shulker boxes</b>. The judge puts a chest of items at the design's <b>ITEMS</b> cell and a chest of empty shulker boxes at its <b>BOXES</b> cell; the machine places a box, fills it, and once it is completely full sends it away, with everything in it, to the chest at its <b>OUTPUT</b> cell. In the test the judge supplies three boxes' worth of {} ({} {example}, {per_box} a box) and all three full boxes must arrive, with no item lost or duplicated and nothing left on the ground, at 4 positions and 4 rotations in the real game.</p>",
                    esc(&stack_words(stack)),
                    3 * per_box
                )
            } else {
                format!(
                    "<p>It <b>unpacks shulker boxes</b>. The judge puts a chest of full shulker boxes at the design's <b>BOXES</b> cell; the machine places a box, empties it into the chest at its <b>ITEMS</b> cell, and sends the empty box to the chest at its <b>OUTPUT</b> cell. In the test the judge supplies three full boxes of {} ({per_box} {example} a box) and every item and all three empty boxes must arrive, nothing lost or duplicated, at 4 positions and 4 rotations in the real game.</p>",
                    esc(&stack_words(stack))
                )
            };
            let speed = if items_s.is_empty() {
                String::new()
            } else {
                format!(
                    "<p>It moves <b>{} items per second</b>{}{}.</p>",
                    esc(items_s),
                    if cycle_s.is_empty() { String::new() } else { format!(": one box every {} ({})", esc(cycle), cycle_s) },
                    if first.is_empty() { String::new() } else { format!("; the first box arrives {} after the start", esc(&with_seconds(first))) }
                )
            };
            format!("{lead}{speed}{}", also("scenarios"))
        }
        "bud" => {
            let detects: Vec<&str> = metric(c, "detects").split_whitespace().collect();
            let misses: Vec<&str> = ["place", "remove", "state", "shape", "update"].into_iter().filter(|k| !detects.contains(k)).collect();
            let list = |v: &[&str]| v.iter().map(|k| format!("<b>{k}</b>")).collect::<Vec<_>>().join(", ");
            let latency = with_seconds(metric(c, "latency"));
            let pulse = with_seconds(metric(c, "pulse"));
            let rearm = with_seconds(metric(c, "re-arm"));
            let jams = metric(c, "jams");
            format!(
                "<p>A <b>block change detector</b> (players call it a BUD switch, for block update detector) fires its output when something happens at its <b>target</b> cell, next to it, although nothing there powers it. The judge tries five kinds of change at the target, three times each: <b>place</b> (a block placed), <b>remove</b> (removed), <b>state</b> (the block changes state), <b>shape</b> (a change that sends shape updates only, the way crops grow) and <b>update</b> (a block update next to the target with nothing changing). Each detected change must give exactly one output pulse, and nothing may fire in between.</p><p>This design detects {}{}.</p><p>{}{}{}{}</p>{}",
                if detects.is_empty() { "nothing".to_string() } else { list(&detects) },
                if misses.is_empty() { "; it misses nothing".to_string() } else { format!("; it does not see {}", list(&misses)) },
                if pulse.is_empty() { String::new() } else { format!("Each detection gives a {pulse} output pulse") },
                if latency.is_empty() { String::new() } else { format!(", {latency} after the change. ") },
                if rearm.is_empty() { String::new() } else { format!("It can detect the next change {rearm} later (re-arm). ") },
                if jams.is_empty() { String::new() } else { format!("Two changes close together jam it: {}.", esc(jams)) },
                // the kind's mention (the coordinator 11:47Z): the latched classic, the observer
                crate::kinds::mention(&c.study, "bud").map(|m| format!("<p>{}.</p>", esc(&first_cap(m)))).unwrap_or_default()
            )
        }
        "tnt-duper" => {
            let rate = metric(c, "rate");
            let period = with_seconds(metric(c, "period"));
            let per = metric(c, "per period");
            format!(
                "<p>It <b>keeps lighting TNT forever without using up its TNT block</b>{}. Every {} a piston pushes the TNT block: a piston copies every block it moves before it clears the old cells, and a TNT that gets a block update while powered at that instant is lit where it stands (a primed TNT that falls and explodes 80 gt, 4 s, later) while its moved copy lands as a TNT block again.{}</p>",
                if rate.is_empty() { String::new() } else { format!(": <b>{}</b>", esc(&site_model::html::tnt_rate_words(rate))) },
                if period.is_empty() { "cycle".to_string() } else { period },
                if per.is_empty() { String::new() } else { format!(" Each push lights {} TNT.", esc(per)) }
            )
        }
        "tnt-duper-triggered" => {
            let per = metric(c, "per pulse");
            let latency = with_seconds(metric(c, "latency"));
            let gap = with_seconds(metric(c, "gap"));
            format!(
                "<p>Each input pulse <b>lights {} TNT</b>{} and the TNT block is back in place for the next pulse: the TNT is never used up.{}</p><p>How: a piston copies every block it moves before it clears the old cells, and a TNT that gets a block update while powered at that instant is lit where it stands while its moved copy lands as a TNT block again.</p>",
                if per.is_empty() { "one".to_string() } else { esc(per) },
                if latency.is_empty() { String::new() } else { format!(" ({latency} after the pulse)") },
                if gap.is_empty() { String::new() } else { format!(" Pulses must be at least {gap} apart.") }
            )
        }
        "burst" | "divider" | "monostable" | "off-delay" | "on-delay" | "pulse-train" => what_timer_does(c),
        _ => return None,
    };
    Some(html)
}

/// `20 gt` -> `1 pulse a second`, `8 gt` -> `2.5 pulses a second`, `40 gt` -> `a pulse every 2 s`.
fn rate_words(v: &str) -> String {
    let Some(n) = v.split_whitespace().next().and_then(|n| n.parse::<f64>().ok()).filter(|n| *n > 0.0) else {
        return String::new();
    };
    let per_s = 20.0 / n;
    if per_s < 1.0 {
        format!("a pulse every {} s", trim_num(n / 20.0))
    } else if (per_s - 1.0).abs() < 1e-9 {
        "1 pulse a second".to_string()
    } else {
        format!("{} pulses a second", trim_num(per_s))
    }
}

/// `2.50` -> `2.5`, `3.00` -> `3`.
fn trim_num(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Study 11 (pulse multipliers, dividers, monostables and timers): what the design does, with
/// its numbers.
fn what_timer_does(c: &Circuit) -> String {
    let m = |k: &str| with_seconds(metric(c, k));
    let n = kind_number(&c.kind).or_else(|| metric(c, "N").parse().ok()).unwrap_or(0);
    let p = |s: String| if s.is_empty() { String::new() } else { format!("<p>{}</p>", s.trim()) };
    match base(&c.kind) {
        "burst" => {
            let (pulse, period, latency, span) = (metric(c, "pulse").to_string(), metric(c, "period").to_string(), m("latency"), m("span"));
            let shape = match (pulse.is_empty(), period.is_empty()) {
                (false, false) => format!(" ({pulse} each, one every {period})"),
                (false, true) => format!(" ({pulse} each)"),
                _ => String::new(),
            };
            let timing = match (latency.is_empty(), span.is_empty()) {
                (false, false) => format!("The first comes {latency} after the input; the whole burst takes {span}. "),
                (false, true) => format!("The first comes {latency} after the input. "),
                _ => String::new(),
            };
            let min_pulse = m("min-pulse");
            let retrigger = match metric(c, "retrigger") {
                "mixed" => "A second press during the burst: one that comes within a few game ticks is swallowed, a later one adds a second burst. ".to_string(),
                "ignores" => "A second press during the burst is ignored. ".to_string(),
                "adds" => format!("A second press during the burst adds {n} more pulses. "),
                "restarts" => "A second press during the burst restarts it. ".to_string(),
                "" => String::new(),
                other => format!("A second press during the burst: {}. ", esc(other)),
            };
            let repeat = m("repeat");
            format!(
                "<p>Each input pulse gives <b>exactly {n} output pulses</b>{shape}: a pulse multiplier. Builds use it to fire a dispenser or a dropper exactly {n} times, or push a piston {n} times, from one button press.</p>{}{}",
                p(timing),
                p(format!(
                    "{}{retrigger}{}",
                    if min_pulse.is_empty() { String::new() } else { format!("Any input of {min_pulse} or longer gives the same burst (a button, a plate or a held lever alike). ") },
                    if repeat.is_empty() { String::new() } else { format!("Pressed again and again it keeps up with presses {repeat} apart: fed a clock that fast, it multiplies its frequency by {n}.") }
                ))
            )
        }
        "divider" => {
            let square = metric(c, "output") == "square";
            let duty = metric(c, "duty");
            let shape = if square {
                format!(
                    " Its output is a <b>square wave</b>: on for half of each cycle, off for the other half{}.",
                    if n % 2 == 1 { format!(" (for an odd N like {n} that takes both edges of the input clock: the output changes every {} input pulses)", trim_num(n as f64 / 2.0)) } else { String::new() }
                )
            } else if !duty.is_empty() {
                format!(" Its output is on {} of the time (a short pulse per cycle).", esc(duty))
            } else {
                String::new()
            };
            let latency = m("latency");
            let min_period = metric(c, "min-period");
            format!(
                "<p>It <b>divides a clock by {n}</b>: for every {n} pulses that come in, the output goes through one on-off cycle, so it runs {n} times slower than its input.{shape}</p>{}",
                p(format!(
                    "{}{}",
                    if latency.is_empty() { String::new() } else { format!("Each output cycle starts {latency} after the input pulse that begins it, always the same. ") },
                    if min_period.is_empty() { String::new() } else { format!("It keeps up with input clocks down to one pulse every {min_period} ({}); faster ones break the count.", rate_words(min_period)) }
                ))
            )
        }
        "monostable" => {
            let l = m("L");
            let retrig = c.kind.contains("non-retriggerable") || metric(c, "retrigger") == "non-retriggerable";
            let kind_text = if retrig {
                "It is <b>non-retriggerable</b>: input pulses that come while the output is on are ignored (one shot per press, however hard the button is mashed).".to_string()
            } else if c.kind.contains("retriggerable") || metric(c, "retrigger") == "retriggerable" {
                format!("It is <b>retriggerable</b>: another input pulse while the output is on restarts the count, so the output stays on while pulses keep coming and turns off {l} after the last one (a light that stays on while there is activity; inverted, an alarm for a clock that stopped).")
            } else {
                String::new()
            };
            let (delay, min_pulse, rearm) = (m("delay"), m("min-pulse"), m("re-arm"));
            format!(
                "<p>Each input pulse, however short, turns the output on for <b>{l}</b>: a one-shot timer. {kind_text}</p>{}",
                p(format!(
                    "{}{}{}",
                    if delay.is_empty() { String::new() } else { format!("The output turns on {delay} after the input. ") },
                    if min_pulse.is_empty() { String::new() } else { format!("It reacts to input pulses of {min_pulse} or longer. ") },
                    if rearm.is_empty() {
                        String::new()
                    } else if c.kind.contains("retriggerable") && !c.kind.contains("non-retriggerable") {
                        // (critique C49: "re-arm" next to "retriggerable" read as a contradiction)
                        format!("Two presses at least {rearm} apart give two separate output pulses; closer together, the output just stays on longer.")
                    } else {
                        format!("Two input pulses at least {rearm} apart give two full output pulses.")
                    }
                ))
            )
        }
        "off-delay" => {
            let l = m("L");
            let latency = metric(c, "latency");
            let min_pulse = m("min-pulse");
            format!(
                "<p>The output turns on with the input{} and <b>stays on {l} after the input turns off</b>: every output pulse is the input's length plus {l}. Input pulses closer together than that merge into one long output pulse (a light or a door that stays on a while after the last press of a pressure plate).</p>{}",
                if latency == "0 gt" { " (in the same game tick)".to_string() } else if latency.is_empty() { String::new() } else { format!(" ({} later)", with_seconds(latency)) },
                p(if min_pulse.is_empty() { String::new() } else { format!("It reacts to input pulses of {min_pulse} or longer.") })
            )
        }
        "on-delay" => {
            let t = m("T");
            let shortening = m("shortening");
            let latency = metric(c, "latency");
            format!(
                "<p>The output turns on only <b>once the input has been on for {t}</b>, and turns off with it: shorter inputs give nothing at all. It filters out glitches (the 1-2 gt pulses of observers, flicker) and makes \"hold the button\" controls.</p>{}",
                p(format!(
                    "{}{}",
                    if shortening.is_empty() { String::new() } else { format!("An input longer than {t} comes out shortened by {shortening}") },
                    match (shortening.is_empty(), latency) {
                        (false, "0 gt") => "; the output falls in the same game tick as the input.".to_string(),
                        (false, "") => ".".to_string(),
                        (false, l) => format!("; the output falls {} after the input.", with_seconds(l)),
                        _ => String::new(),
                    }
                ))
            )
        }
        _ => {
            // pulse-train
            let (period, pulse, latency) = (metric(c, "P"), metric(c, "pulse"), m("latency"));
            let stop = metric(c, "stop").split_whitespace().next().and_then(|v| v.parse::<i64>().ok());
            format!(
                "<p>While the input is on, the output <b>pulses every {period}</b> ({}{}): a clock with an on switch, for rapid fire from dispensers and droppers while a lever is on.</p>{}",
                rate_words(period),
                if pulse.is_empty() { String::new() } else { format!(", each {pulse} long") },
                p(format!(
                    "{}{}",
                    if latency.is_empty() { String::new() } else { format!("The first pulse comes {latency} after the input turns on. ") },
                    match stop {
                        Some(s) if s <= 0 => "When the input turns off, it stops at once.".to_string(),
                        Some(s) => format!("When the input turns off, no pulse starts more than {} later.", with_seconds(&format!("{s} gt"))),
                        None => String::new(),
                    }
                ))
            )
        }
    }
}

/// `2nd`, `3rd`, `11th`, `21st`.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The input's control in the viewer, as the page names it (the viewer labels its bar buttons
/// with the port name uppercased).
fn input_label(name: &str) -> String {
    name.to_uppercase()
}

/// The kinds whose inputs are steady (toggled) rather than pulsed (webrender's input mode).
pub fn input_mode(kind: &str) -> &'static str {
    match base(kind) {
        // a shulker machine's start lever (and the combo's mode lever) stays where it is put; a
        // gated pulse train and an off-delay timer act on the input's level (on / off)
        "gate" | "d-latch" | "piston-extender" | "piston-bridge" | "shulker-loader" | "shulker-unloader" | "pulse-train" | "off-delay" => "toggle",
        _ => "pulse",
    }
}

/// The reason a design cannot be simulated, for a page, without the simulator's name, the tick
/// and the world coordinates (`not modeled by redsim (tick 28 of its run): dropper at (0,0,0)
/// picks a random slot among 3 stacks (not modeled)` -> `dropper picks a random slot among 3
/// stacks`).
/// Why a design's view shows it as built (`Circuit::unmodeled`, from the viewer's own session, so
/// it follows the viewer: a kind webrender makes exact again plays again, its pages with it), in a
/// visitor's words: what the browser's simulator does not run (`it uses blast furnaces, which the
/// browser's simulator does not run yet`) before the viewer's own "this view does not replay our
/// test exactly yet" (the coordinator 2026-10-22: crop farms, super smelters, brewers and
/// composters shown as built, webrender's `circuit::not_replayed_yet`).
pub fn as_built_why(unmodeled: &str) -> String {
    let parts: Vec<&str> = unmodeled.split("; ").map(str::trim).filter(|p| !p.is_empty()).collect();
    let replay = |p: &str| p.starts_with(mcview::circuit::NOT_REPLAYED);
    let first = parts.iter().copied().find(|p| !replay(p)).or_else(|| parts.first().copied()).unwrap_or("");
    if replay(first) {
        return "it does not replay our test exactly yet".to_string();
    }
    // (`block "blast_furnace" is not modeled`: what it is, in words)
    if let Some(name) = first.strip_prefix("block \"").and_then(|r| r.split_once("\" is not modeled")).map(|(n, _)| n.trim_start_matches("minecraft:").replace('_', " ")) {
        let many = if name.ends_with('s') { name } else { format!("{name}s") };
        return format!("it uses {many}, which the browser's simulator does not run yet");
    }
    let why = plain_reason(first);
    if why.starts_with("it uses ") || why.starts_with("the browser's simulator") { why } else { format!("the browser's simulator cannot run it yet ({why})") }
}

pub fn plain_reason(e: &str) -> String {
    let mut s = e.trim().to_string();
    // the viewer's prefix: `not modeled by redsim (tick N of its run): `
    if let Some(i) = s.find("of its run): ") {
        s = s[i + "of its run): ".len()..].to_string();
    } else if let Some(rest) = s.strip_prefix("not modeled by redsim: ") {
        s = rest.to_string();
    }
    // an entity with its raw data (critique 41f1ec62a: "entity minecraft:item_frame with Facing in
    // its data ({Facing:4b,Item:{...}})" on the lock's page): what it is, in words
    if let Some(rest) = s.strip_prefix("entity minecraft:") {
        let id = rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next().unwrap_or("").replace('_', " ");
        return format!("it uses {id}s, which the browser's simulator does not run yet");
    }
    // world coordinates mean nothing to a visitor: ` at (0,0,0)`
    while let Some(i) = s.find(" at (") {
        match s[i..].find(')') {
            Some(j) => s.replace_range(i..i + j + 1, ""),
            None => break,
        }
    }
    let s = s.trim_end_matches(" (not modeled)");
    s.replace(" by redsim", "").replace("redsim", "the simulator").trim().to_string()
}

/// Where the 3D view's demo stops (`Circuit::view_halt`, raw: `tick 44: dropper at (1,0,2) picks a
/// random slot among 9 stacks (not modeled)`), in words: (the tick, why it stops there).
pub fn halt_words(raw: &str) -> (Option<u32>, String) {
    let (tick, rest) = match raw.strip_prefix("tick ").and_then(|r| r.split_once(": ")) {
        Some((t, rest)) => (t.trim().parse::<u32>().ok(), rest),
        None => (None, raw),
    };
    let why = plain_reason(rest);
    // a dropper or dispenser holding several stacks fires a random one of them in the game
    // (`picks a random slot among 9 stacks (not modeled)`, `... among 2 stacks of different items
    // (not modeled without the level random)`)
    let why = match why.split_once(" picks a random slot among ") {
        Some((what, n)) => {
            let n = n.split(" (not modeled").next().unwrap_or(n).trim();
            let n = if n.contains("item") { n.to_string() } else { format!("{n} of items") };
            format!("its {what} holds {n} and the game fires a random one of them, which the browser's simulator does not model")
        }
        None => format!("the browser's simulator does not model what comes next ({why})"),
    };
    (tick, why)
}

/// The probe design for [`viewer_turns_frames`]: one item frame, holding an item, on a stone, and
/// the `frame` input in its cell.
const FRAME_PROBE: &str = "name: probe\ncircuit: lock 1\norigin: 0 0 0\nport input.d0: 0 1 0 frame\nport output: 2 0 0\nentity: minecraft:item_frame 0.5 1.5 0.5 {Facing:1b,Item:{id:\"minecraft:arrow\",count:1},ItemRotation:0b}\npalette:\n  A minecraft:stone\nlayer 0:\n  A\n";

/// The 3D view with the frame probe loaded: its input pressed, a few ticks run (None when the
/// view cannot run it). The frames' rotations before and after.
fn frame_probe_run() -> Option<(Vec<u8>, Vec<u8>)> {
    let mut s = mcview::session::Session::new(FRAME_PROBE, &mcview::session::Options::default()).ok()?;
    if !s.simulated() || s.circuit_inputs.is_empty() || s.world.frames.is_empty() {
        return None;
    }
    let before: Vec<u8> = s.world.frames.iter().map(|f| f.rotation).collect();
    s.pulse_input(0, 2);
    for _ in 0..6 {
        s.step();
    }
    Some((before, s.world.frames.iter().map(|f| f.rotation).collect()))
}

/// Whether the 3D view's input buttons turn an item frame, tried on the viewer itself: the probe's
/// input pressed must turn its frame (until it does, a `frame` input falls to the viewer's
/// default, a block written or driven into its cell, and a page must not send a visitor to those
/// buttons: critique 058332315). Flips by itself when the viewer learns frames.
pub fn viewer_turns_frames() -> bool {
    static ONCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ONCE.get_or_init(|| frame_probe_run().is_some_and(|(before, after)| before != after))
}

/// How to try the circuit in the viewer: the demo, the input buttons, the markers.
pub fn how_to_try(c: &Circuit, has_viewer: bool) -> String {
    if !has_viewer {
        return "<p>Download the design (below) to build it in the game; the 3D view needs a browser with WebGL.</p>".into();
    }
    let mut s = String::new();
    // a design the browser's simulator cannot run yet (the shulker study has its own words); the
    // reason in words, not the simulator's raw data (critique 41f1ec62a)
    if !c.unmodeled.is_empty() && !base(&c.kind).starts_with("shulker") {
        // (the build only, the reason in a visitor's words, then the game: the coordinator
        // 2026-10-22, "the view shows the build only, so download it to watch it run in the game")
        s.push_str(&format!(
            "<p>The 3D view shows the build only (<b>as built</b>): {}. Our test ran it in the real game; its numbers are in the measurements below. Drag to look around, pinch or scroll to zoom; download the design (below) to watch it run in the game.</p>",
            esc(&as_built_why(&c.unmodeled))
        ));
        return s;
    }
    // a demo the view stops early: said, with why (webrender's QA: the 100- and 576-item counters
    // stop at tick 44, their droppers holding several stacks)
    if !c.view_halt.is_empty() {
        let (tick, why) = halt_words(&c.view_halt);
        s.push_str(&format!(
            "<p>The 3D view shows this design as built and runs our test up to {}, then stops: {}. The judge tested it in the real game; its timing is in the measurements below. Drag to look around, pinch or scroll to zoom; download the design (below) to build it in the game.</p>",
            tick.map(|t| format!("tick {t}")).unwrap_or_else(|| "its first steps".to_string()),
            esc(&why)
        ));
        return s;
    }
    if base(&c.kind) == "flying-machine" {
        // (the docking-station machine's lever is a real input: the coordinator 00:45Z)
        if c.devices.iter().any(|d| d == "lever") {
            s.push_str("<p>The machine starts by itself (its start block is placed when the view begins): press <b>play</b> and watch it shuttle between its docking stations. Flip its lever with the <span class=\"tag-in\">IN</span> button under the view: off, it docks at the lever's station and stays; on, it leaves again. Drag to look around, pinch or scroll to zoom.</p>");
        } else {
            s.push_str("<p>The machine starts by itself (its start block is placed when the view begins): press <b>play</b> and watch it fly along its corridor; <b>reset</b> to see it again. Drag to look around, pinch or scroll to zoom.</p>");
        }
        return s;
    }
    // an item sorter: items in, a chest, no lamp (critique: the generic lamp text)
    if base(&c.kind) == "sorter" {
        let item = c.kind.split_whitespace().nth(1).map(|i| format!("the {}s", i.replace('_', " "))).unwrap_or_else(|| "the items of its kind".to_string());
        s.push_str(&format!("<p><b>The sorter runs in the view</b> the way our test does: mixed items come in at the cell tagged ITEMS and travel along the line of hoppers; {item} drop into the chest at OUT, and everything else goes on to the end of the line. The numbers over each container show what it holds; speed it up with the bar's speed dropdown. Drag to look around, pinch or scroll to zoom.</p>"));
        return s;
    }
    // a whole minecart line: shown as built with our test's chests, no cart running in the view
    if base(&c.kind) == "cart-line" {
        s.push_str("<p><b>The view shows the line as built</b>, with our test's two double chests: the supply chest at the loader's end and the storage chest at the unloader's. The browser's simulator does not run its cart yet. Drag to look around, pinch or scroll to zoom. In the game, put one hopper minecart on the track and items in the supply chest: the cart fills, carries them to the storage chest and comes back for more.</p>");
        return s;
    }
    // the kinds shown with their judge's chests standing (mcview's STATIC_CHEST_KINDS: their item
    // runs are not replayed; fly's QA item 1, the generic lamp text on a minecart stop, an
    // auto-crafter and an item elevator, whose outputs are chests)
    let what = match base(&c.kind) {
        "cart-unloader" | "cart-loader" => Some("the station as built, with our test's chest and no cart: the browser's simulator does not run hopper minecarts here"),
        "crafter" => Some("the crafter as built, with our test's chests and none of the items our test feeds it"),
        "item-elevator" => Some("the elevator as built, with our test's chests and none of the items our test sends up it"),
        _ => None,
    };
    if let Some(w) = what {
        s.push_str(&format!("<p><b>The view shows {w}.</b> Drag to look around, pinch or scroll to zoom. Its numbers are our test's, in the real game, below.</p>"));
        return s;
    }
    // an automatic door: no walkers in the view (fly's QA: its How to try it was empty)
    if base(&c.kind) == "auto-door" {
        s.push_str("<p><b>The view shows the door as built</b>, closed: the browser's simulator has no walkers, so nothing steps on its plates here. Drag to look around, pinch or scroll to zoom. Its pressure plates lie in the walkway on both sides of the door, where you step anyway: in the game, walk through from either side.</p>");
        return s;
    }
    // the 0-9 display: its input is a chest's level, which our test's walk sets to each digit (the
    // generic text spoke of "the lamp the judge adds" at an OUT it does not have)
    if site_model::html::decimal_display(c) {
        s.push_str("<p><b>Press play under the view to watch our test</b> on a loop: it fills the chest tagged A to every level from 0 to 9, one change at a time (every change between two digits), and the face shows each digit; the badge names the step. The A button under the view only empties the chest or fills it to the top (15, beyond the digits it shows). Drag to look around, pinch or scroll to zoom.</p>");
        return s;
    }
    // a crop farm: the crop, the judge's chests at its outputs, no lamp (fly's QA: the generic
    // lamp text on the farms)
    if base(&c.kind) == "harvester" {
        let what = if site_model::html::stem_crop(c) {
            let goes = if site_model::html::harvest_words(c).ends_with("slices") { "its slices drop" } else { "it drops" };
            format!("every {} that grows beside a stem is broken and {goes}", site_model::html::crop_words(c))
        } else {
            "every block that grows is broken and its item drops".to_string()
        };
        let chests = if c.outputs.len() > 1 { format!("the {} chests at OUT", c.outputs.len()) } else { "the chest at OUT".to_string() };
        let faster = site_model::html::farm_speed_words(c);
        s.push_str(&format!("<p><b>The farm runs in the view</b> the way our test does: the view plants the crop and puts our test's chests at its outputs, then grows it {faster} than the game; {what} into {chests}. The numbers over each chest show what it holds; speed it up with the bar's speed dropdown. Drag to look around, pinch or scroll to zoom.</p>"));
        return s;
    }
    // an auto-smelter: chests in and out, furnaces, no lamp
    if base(&c.kind) == "smelter" {
        s.push_str("<p><b>The smelter runs in the view</b> the way our test does: items to smelt start in the chest tagged ITEMS and coal in the chest tagged FUEL; hoppers carry them into the furnaces, and every result drops into the chest at OUT. The numbers over each container show what it holds; speed it up with the bar's speed dropdown. Drag to look around, pinch or scroll to zoom.</p>");
        return s;
    }
    // an auto-brewer: its four supply chests and the output, no lamp
    if base(&c.kind) == "brewer" {
        s.push_str("<p><b>The brewer runs in the view</b> the way our test does: water bottles start in the chest tagged BOTTLES, blaze powder in FUEL, nether wart in WART and sugar in SUGAR; the machine puts one wart, then one sugar, into each brewing stand for every three bottles, and every finished potion drops into the chest at OUT. The numbers over each container show what it holds; speed it up with the bar's speed dropdown (a brew takes 20 seconds). Drag to look around, pinch or scroll to zoom.</p>");
        return s;
    }
    // an auto-composter: the input chest, the overflow chest, the bone meal chest, no lamp
    if base(&c.kind) == "composter" {
        s.push_str("<p><b>The composter runs in the view</b> the way our test does: the farm's mixed output starts in the chest tagged ITEMS and travels along the line of hoppers; the slice's crop drops into the composter, the bone meal falls into the chest at OUT and everything else runs on to the chest tagged PASS (the overflow). The numbers over each container show what it holds; speed it up with the bar's speed dropdown. Drag to look around, pinch or scroll to zoom.</p>");
        return s;
    }
    // a combination lock: the dials in words, never the raw port names (critique 058332315: "the
    // D0, D1, D2, K0, K1 and K2 buttons"); its buttons only when they turn the frames
    if base(&c.kind) == "lock" {
        if viewer_turns_frames() {
            // (the view's own tags, from webrender's input_label: "DIAL 1".."CODE 3")
            let tags = |prefix: char| -> Vec<String> { c.inputs.iter().zip(&c.input_labels).filter(|(n, _)| n.starts_with(prefix)).map(|(_, l)| l.to_uppercase()).collect() };
            let span = |v: &[String]| match v {
                [] => String::new(),
                [one] => one.clone(),
                [first, .., last] => format!("{first} to {last}"),
            };
            let (dials, codes) = (tags('d'), tags('k'));
            if dials.is_empty() || codes.is_empty() {
                s.push_str("<p>The dials are the item frames tagged on one side, the code frames those on the other: tap one, or its button under the view, to turn it one step (8 steps around). The output, tagged OUT, turns on only while every dial shows the same position as its code frame. Drag to look around, pinch or scroll to zoom.</p>");
            } else if dials.len() == 1 {
                // (the 1-dial lock: one of each, said so)
                s.push_str(&format!(
                    "<p>The dial is the item frame tagged {}, the code frame the one tagged {}: tap a tag, or its button under the view, to turn that frame one step (8 steps around). The output, tagged OUT, turns on only while the dial shows the same position as the code frame. Drag to look around, pinch or scroll to zoom.</p>",
                    esc(&span(&dials)),
                    esc(&span(&codes))
                ));
            } else {
                s.push_str(&format!(
                    "<p>The dials are the item frames tagged {}, the code frames those tagged {}: tap a tag, or its button under the view, to turn that frame one step (8 steps around). The output, tagged OUT, turns on only while every dial shows the same position as its code frame. Drag to look around, pinch or scroll to zoom.</p>",
                    esc(&span(&dials)),
                    esc(&span(&codes))
                ));
            }
        } else {
            s.push_str("<p>The view runs the lock's redstone, but its buttons cannot turn item frames yet (they do not stand for a right-click): try the dials in the game, or read the measurements below, from our test in the real game. Drag to look around, pinch or scroll to zoom.</p>");
        }
        return s;
    }
    match base(&c.kind) {
        "shulker-loader" | "shulker-unloader" => {
            let loader = base(&c.kind) == "shulker-loader";
            if !c.unmodeled.is_empty() {
                s.push_str(&format!(
                    "<p>The 3D view shows the machine <b>as built</b>, with the judge's chests at the cells tagged ITEMS, BOXES and OUT and a shulker box in front of the dispenser: the browser's simulator cannot follow its run yet ({}). The judge tested it in the real game; drag to look around, pinch or scroll to zoom, and download the design (below) to build it.</p>",
                    esc(&plain_reason(&c.unmodeled))
                ));
                return s;
            }
            // the view's own run sent out no box (run at build time for 60000 gt): said, never "the
            // machine runs in the view"
            if c.view_run.as_ref().is_some_and(|v| v.boxes == 0) {
                s.push_str(&format!(
                    "<p>The 3D view runs the machine with the judge's chests at the cells tagged ITEMS, BOXES and OUT, but <b>no box comes out there yet</b>: the browser's simulator does not run this design the way our test did (we ran the view for {} without a box). The judge's numbers are in the measurements below; drag to look around, pinch or scroll to zoom, and download the design (below) to build it.</p>",
                    real_time(c.view_run.as_ref().map(|v| v.ticks).unwrap_or(0))
                ));
                return s;
            }
            let badges = metric(c, "badges");
            let sorts = badges.split('+').any(|b| b == "filter");
            let supply = if loader {
                "The view runs it the way the judge does, without end: the chests at the cells tagged ITEMS and BOXES are kept full (items, empty shulker boxes) and the one at OUT is emptied as boxes arrive"
            } else {
                "The view runs it the way the judge does, without end: the chest at the cell tagged BOXES is kept full of full shulker boxes, and the chests at ITEMS and OUT are emptied as the items and the empty boxes arrive"
            };
            let what = if sorts { " (with the one kind of item it packs, 64 to a stack)." } else { ". The items are unstackable (27 fill a box)." };
            // the numbers of the view's own run (the viewer's session, run at build time)
            let gt = |t: f64| -> String {
                let n = t.round().max(0.0) as u32;
                format!("{n} gt ({})", real_time(n))
            };
            let timing = match c.view_run.as_ref().map(|v| (v.first, v.cycle, v.lines)) {
                Some((Some(f), Some(cy), lines)) => format!(
                    " In the view the first box goes out {} after the machine starts, then {}one every {}{}{}.",
                    gt(f as f64),
                    if lines > 1 { format!("from its {lines} lines together, ") } else { String::new() },
                    gt(cy),
                    if lines > 1 { " on average" } else { "" },
                    if cy >= 1200.0 { format!(", or one every {:.0} s at 100× speed", cy / 2000.0) } else { String::new() }
                ),
                Some((Some(f), None, _)) => format!(" In the view the first box goes out {} after the machine starts.", gt(f as f64)),
                _ => String::new(),
            };
            s.push_str(&format!("<p><b>The machine runs in the view.</b> {supply}{what}{timing} The numbers over each chest, hopper and dispenser show what it holds. Speed it up with the bar's speed dropdown (up to 100×).</p>"));
            // the start lever is the visitor's (the judge flips it at its start); the combo's
            // mode lever switches loading and unloading
            let mut levers: Vec<String> = Vec::new();
            for (i, name) in c.inputs.iter().enumerate() {
                if c.devices.get(i).map(String::as_str) != Some("lever") {
                    continue;
                }
                let label: String = name.to_uppercase().chars().take(3).collect();
                if name == "mode" || name.starts_with("mode.") {
                    levers.push(format!("Its <b>{label}</b> lever switches it between loading and unloading (the lime {label} button in the bar, or tap the lever)."));
                } else {
                    levers.push(format!("<b>Flip its start lever</b> to start it, as the judge does: the lime {label} button in the bar, or tap the lever."));
                }
            }
            if !levers.iter().any(|l| l.contains("start lever")) {
                levers.insert(0, "It starts by itself (no lever to flip).".to_string());
            }
            s.push_str(&format!("<p>{} Drag to look around, pinch or scroll to zoom.</p>", levers.join(" ")));
            return s;
        }
        "tnt-duper" => {
            s.push_str("<p>The duper runs by itself as soon as it is built: press play under the view to watch it. Watch the piston push the TNT block every cycle and a lit TNT (flashing white) drop out of the machine each time, while the TNT block lands again in its place; a lit TNT vanishes when its fuse runs out instead of exploding. Pause and step one game tick at a time with the bar's buttons; drag to look around, pinch or scroll to zoom.</p>");
            return s;
        }
        "bud" => {
            s.push_str("<p><b>Press play under the view to watch the judge's own test</b> on a loop: the judge places, removes and changes a block at the cell tagged TARGET (in cyan), and the badge names each step (the same test is drawn as a timing diagram below). The output is tagged OUT: watch it fire on the changes the design detects. Touch any control to stop the demo; drag to look around, pinch or scroll to zoom.</p>");
            return s;
        }
        _ => {}
    }
    if !c.stimulus.is_empty() {
        match study11_demo(c) {
            Some(demo) => s.push_str(&format!("<p><b>Press play under the view to watch the judge's own test</b> on a loop: {demo} The badge reads <i>demo</i> with the step it is on, and the same test is drawn as a timing diagram below. Touch any control to stop it and try the circuit yourself.</p>")),
            None => s.push_str("<p><b>Press play under the view to watch the judge's own test</b> on a loop (the badge reads <i>demo</i> with the step it is on; the same test is drawn as a timing diagram below). Touch any control to stop it and try the circuit yourself.</p>"),
        }
    }
    let mut controls: Vec<String> = Vec::new();
    for (i, name) in c.inputs.iter().enumerate() {
        let device = c.devices.get(i).map(String::as_str).unwrap_or("redstone block");
        // (the button as the view labels it, when known: webrender's input_label)
        let label = c.input_labels.get(i).filter(|l| !l.is_empty()).cloned().unwrap_or_else(|| input_label(name));
        let what = match device {
            "lever" => format!("click the <b>lever</b> tagged <b>{label}</b> (or the {label} button of the bar) to switch it on or off"),
            "button" => format!("click the <b>button</b> tagged <b>{label}</b> (or the {label} button of the bar) to press it"),
            _ if input_mode(&c.kind) == "toggle" => {
                format!("tap the <b>{label}</b> button of the bar (or the cell tagged {label}) to turn that input on or off (it powers the cell with a redstone block)")
            }
            _ => format!("press and hold the <b>{label}</b> button of the bar (or the cell tagged {label}) to power that input; a quick tap sends a short 2 gt pulse"),
        };
        controls.push(what);
    }
    if !controls.is_empty() {
        let hint = study11_try(c);
        s.push_str(&format!("<p>To try it: {}{}.</p>", controls.join("; "), if hint.is_empty() { String::new() } else { format!(": {hint}") }));
    }
    let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
    if !outs.is_empty() {
        // webrender bbe3bf7ba: the judge's own lamp stands in every air output cell and shows the
        // output; an output cell that holds a block of the design gets the orange marker instead
        let shows = if c.output_lamp {
            format!(
                "the output is tagged <span class=\"tag-out\">{}</span>: the <b>redstone lamp</b> in that cell (the lamp the judge itself adds to read the output) <b>lights while the output is on</b>",
                esc(&outs.join(", "))
            )
        } else {
            format!("the output is tagged <span class=\"tag-out\">{}</span>: the orange glass marker next to it <b>glows while the output is on</b>", esc(&outs.join(", ")))
        };
        s.push_str(&format!(
            "<p>Inputs are tagged in <span class=\"tag-in\">lime</span>; {shows} (the output strip by the demo badge shows its last 40 game ticks, 2 seconds). Drag to look around, pinch or scroll to zoom, step one game tick at a time with the step button.</p>"
        ));
    }
    s
}

/// What a metric means.
pub fn metric_definition(kind: &str, k: &str) -> &'static str {
    // a registered kind's own definitions first (circuit::registry)
    if let Some((_, d)) = circuit::registry::find(base(kind))
        && let Some(t) = (d.site.metric)(k)
    {
        return t;
    }
    // (a crop farm's "per cane" is per plant: a bamboo stalk, a kelp plant, w86)
    if kind.starts_with("harvester") && !kind.ends_with("sugar_cane") {
        let bamboo = kind.ends_with("bamboo");
        match k {
            "per cane" => return if bamboo { "blocks placed for each bamboo stalk it harvests" } else { "blocks placed for each plant it harvests" },
            "items per hour per cane" => return if bamboo { "bamboo harvested each hour for each stalk, at the game's normal growth speed (random tick speed 3), measured over 10 hours of game time by our simulator's twin of the game" } else { "items harvested each hour for each plant, at the game's normal growth speed (random tick speed 3), measured over 10 hours of game time by our simulator's twin of the game" },
            "harvest height" => return if bamboo { "how tall a stalk gets before the farm cuts it, in blocks" } else { "how tall a plant gets before the farm cuts it, in blocks" },
            _ => {}
        }
    }
    match (base(kind), k) {
        // (a whole minecart line's: sorter's system run, crate::cartsys; first, before the generic
        // "bill")
        ("cart-line", "minutes for a double chest") => "minutes from a full supply double chest (3,456 items) to the last item in the storage chest, one cart making every trip",
        ("cart-line", "items/s") => "items carried each second, over that whole run",
        ("cart-line", "trips") => "the cart's round trips for the double chest",
        ("cart-line", "by the quarter") => "when a quarter, half, three quarters and all of the double chest had arrived: a steady line delivers at an even pace",
        ("cart-line", "bill") => "what its blocks and the cart cost to craft, the two chests aside: iron for the rails, hoppers and cart, gold for the powered rails, quartz for the comparators",
        ("cart-line", "trickle") => "a farm's slow output: a few items now and then, then nothing; every item must arrive, and then the cart must stay parked at the loader rather than shuttle back and forth empty",
        ("cart-line", "track") => "the track between its two stations, and how often a powered rail keeps the cart at speed",
        ("off-delay", "L") => "how long the output stays on after the input turns off",
        ("monostable", "L") => "the length of each output pulse, whatever the input's length",
        ("monostable", "delay") => "game ticks from the input turning on to the output turning on",
        ("monostable", "retrigger") => "retriggerable: a pulse while the output is on restarts L; non-retriggerable: it is ignored",
        ("monostable", "re-arm") => "the shortest time between two presses that still gives two separate output pulses (closer presses restart the timer, or are ignored by a non-restarting one)",
        ("burst", "N") => "output pulses per input pulse",
        ("burst", "pulse") => "the length of each output pulse",
        ("burst", "period") => "game ticks from one output pulse's start to the next one's",
        ("burst", "latency") => "game ticks from the input turning on to the first output pulse",
        ("burst", "span") => "game ticks from the first output pulse's start to the last one's end",
        ("burst", "retrigger") => "what a second press during the burst does: ignored, restarts, adds N more, or mixed (depends on when)",
        ("burst", "repeat") => "the fastest presses (one every this many game ticks) that still each give exactly N pulses",
        ("divider", "N") => "input pulses per output cycle",
        ("divider", "duty") => "the share of each output cycle the output is on",
        ("divider", "output") => "square = on for exactly half of each cycle, pulse = a shorter on time",
        ("divider", "latency") => "game ticks from the input pulse that starts an output cycle to the output turning on",
        ("divider", "min-period") => "the fastest input clock it divides correctly: one pulse every this many game ticks",
        ("on-delay", "T") => "how long the input must stay on before the output turns on",
        ("on-delay", "shortening") => "how much shorter than the input the output pulse is",
        ("on-delay", "latency") => "game ticks from the input turning off to the output turning off",
        ("off-delay", "latency") => "game ticks from the input turning on to the output turning on",
        ("pulse-train", "P") => "game ticks between two output pulses while the input is on",
        ("pulse-train", "pulse") => "the length of each output pulse",
        ("pulse-train", "latency") => "game ticks from the input turning on to the first output pulse",
        ("pulse-train", "stop") => "the latest an output pulse starts after the input turns off (0 or less: none after)",
        (_, "L") => "the length of the output pulse",
        ("piston-extender", "N") => "the reach: how many cells the block is pushed out",
        ("counter", "N") => "how many input pulses one output cycle takes",
        (_, "N") => "the delay: how much later the output copies the input",
        ("flying-machine", "latency") => "game ticks from the start until the machine first moves",
        ("bud", "latency") => "game ticks from the change at the target to the output pulse",
        ("tnt-duper", "latency") => "game ticks from the start to the first lit TNT",
        ("tnt-duper-triggered", "latency") => "game ticks from the input pulse to the lit TNT",
        ("tnt-duper", "period") => "game ticks between two pushes (the engine's cycle)",
        ("bud", "pulse") => "the length of each output pulse",
        (_, "boxes/min") => "full boxes delivered per minute, at the steady rate",
        (_, "items/s") => "items moved per second at the steady rate (from 10% to 90% of the total)",
        ("item-elevator", "first item") => "game ticks from the start until the first item reaches the output chest",
        // (a player elevator's: the fly study)
        ("elevator", "height") => "blocks between the two floors",
        ("elevator", "trip up") => "game ticks from the press at the bottom floor until the platform docks at the top, a rider on it",
        ("elevator", "down") => "game ticks for the trip down",
        ("elevator", "rider") => "what our test puts on the platform: a player-sized rider",
        ("elevator", "calls and sends") => "a button on each floor: it sends the platform to the other floor when the platform is there, else it calls it",
        // (a minecart station's: clk-obs's study 19; b/t = blocks a game tick, 20 a second)
        ("station", "line") => "the kind of line: one-way (every cart goes the same way) or two-way",
        ("station", "stop distance") => "how far the fastest cart rolls past the platform's first rail before it stops",
        ("station", "departure delay") => "game ticks from the button press until the cart starts to move",
        ("station", "leaves at") => "the cart's speed as it leaves the station, in blocks a game tick (20 game ticks a second)",
        ("station", "other way passes at") => "the slowest a cart coming the other way crosses the station, in blocks a game tick",
        // (w86's binary counter)
        ("binary-counter", "per bit") => "blocks placed for each bit it counts",
        ("binary-counter", "edge") => "rising: each press counts when the button turns on",
        // (a hopper-minecart unloader's or loader's: clk-obs's circuit::cartio)
        ("cart-unloader", "unload rate") => "items a second moved from the cart into the chest while the cart stands in the station",
        ("cart-loader", "load rate") => "items a second moved from the supply chest into the cart while the cart stands in the station",
        ("cart-unloader", "departure delay") => "game ticks from the cart being empty to its first move",
        ("cart-loader", "departure delay") => "game ticks from the cart being full, or the supply running dry, to its first move",
        ("cart-unloader" | "cart-loader", "line") => "terminus: the cart leaves back the way it came; through line: it goes on",
        // (the 0-9 display's: clk-obs's judge, designs/briefs/display-7seg.md)
        ("seven-segment", "latency") => "the longest from the container's level changing to the whole face showing the new digit, over every change between two digits (the slowest change is named)",
        ("seven-segment", "glitches") => "lamp changes that come and go within a change between two digits, too brief to see (under 4 game ticks); counted, not shown",
        ("seven-segment", "on-blips") => "a lamp that lights wrongly for long enough to see (4 game ticks or more) during a change between two digits: 0 means no wrong lamp ever flashes",
        // (an auto-smelter's: study 17, sorter's `chest` run for the super smelters)
        ("smelter", "minutes for a double chest") => "minutes from a fresh build with a double chest of raw iron (3,456) in its input chest and the coal it needs plus one stack (496, never refilled) in its fuel chest, until the last ingot is in the output chest",
        ("smelter", "of the ceiling") => "how close it comes to its furnaces' own limit: every furnace busy from the first tick, 10 seconds an item in a furnace, 5 in a blast furnace",
        ("smelter", "first ingot") => "game ticks from loading the chests until the first ingot reaches the output chest",
        ("smelter", "coal left") => "coal still in the fuel chest and the machine after the double chest is smelted (it started with 496: the 432 the load burns and a stack)",
        ("smelter", "coal needed") => "coal the fuel chest must start with for a double chest: the 432 the load burns and one stack, 496 in all (7 to 8 stacks)",
        ("smelter", "spread") => "items each furnace smelted in our test",
        ("smelter", "items/min per furnace") => "items smelted a minute by each furnace at the steady rate (a furnace's own limit is 6, a blast furnace's 12)",
        ("smelter", "first item") => "game ticks from loading the chests until the first result reaches the output chest",
        ("smelter", "items per coal") => "items smelted for each coal burnt (a coal smelts 8 items in a furnace that never runs dry)",
        ("smelter", "blocks per furnace") => "blocks placed for each furnace",
        // (an automatic door's: clk-obs's walkers)
        ("auto-door", "open wait") => "the longest a walker waited at the closed door before it opened, in game ticks",
        ("auto-door", "close delay") => "game ticks from the last walker leaving the doorway to the door being flush again",
        ("auto-door", "door") => "the size of the doorway it opens",
        ("auto-door", "stop in doorway") => "what the door does when someone stops and stands still in the doorway: stays open for them, or closes on them (it squeezes them and hurts them, half a heart per hurt, until they step out); reported, not required",
        // (a button sequence lock's: clk-item's judge)
        ("sequence-lock", "buttons") => "buttons a player presses",
        ("sequence-lock", "code") => "presses in its code",
        ("sequence-lock", "presses tried") => "press sequences our test tried: right, wrong, too short, and every wrong press at every step",
        ("sequence-lock", "opens within") => "the longest from the code's last right press to the output turning on, in game ticks",
        ("sequence-lock", "closes within") => "the longest from the next press to the output turning off again, in game ticks",
        ("sequence-lock", "semantics") => "sliding window: it opens whenever its last presses match the code, like a keypad; strict reset: a wrong press sends it back to the start and does not count as a first press",
        // (an auto-crafter's: clk-item's study)
        ("crafter", "crafts/min") => "crafts a minute at the steady rate, fed at hopper speed",
        ("crafter", "first craft") => "game ticks from the first item in until the first craft",
        ("crafter", "crafts") => "crafts counted in our test, every one exact (and how many partial sets it was left waiting with, then completed)",
        ("crafter", "recipe class") => "3x3: a recipe that fills the whole crafting grid; 2x2: a smaller one (the crafter's other slots disabled)",
        ("crafter", "supply") => "the port its items come in by",
        (_, "at rest") => "whether the build comes to rest once its work is done (no: it keeps running, like an elevator's clock)",
        (_, "first box") => "game ticks from the start until the first box arrives",
        (_, "cycle") => "game ticks per box at the steady rate",
        (_, "hoppers awake") => "hoppers left unlocked at rest: each one looks for items every game tick (lag on busy servers)",
        (_, "badges") => "the extra scenarios it passes, each a separate run of the judge (see what it does above)",
        (_, "bill") => "what its blocks cost to craft: iron, quartz, redstone, slime",
        (_, "detects") => "the kinds of change at the target that fire the output (place, remove, state, shape, update)",
        (_, "re-arm") => "the shortest time between two changes that are both detected",
        (_, "jams") => "changes too close together for it",
        (_, "footprint") => "the cells around the design that must be left alone for it to work",
        (_, "rate") => "lit TNT per minute",
        (_, "per period") => "TNT lit by each push",
        (_, "per pulse") => "TNT lit by each input pulse",
        // (a piston bridge's gap and a randomizer's latency mean their own things: below the
        // generic arms they could not be reached, and the bridge pages read a TNT duper's "gap")
        ("piston-bridge", "gap") => "the gap it spans, in blocks",
        ("randomizer", "latency") => "game ticks from a press to its output turning on, the slowest in our test",
        (_, "gap") => "the shortest time between two input pulses that both light their TNT",
        (_, "latency") => "the worst time from an input change to the output's change",
        (_, "delay") => "the time from the input's edge to the start of the output pulse",
        (_, "min-pulse") => "the shortest input pulse handled correctly (every longer one works too)",
        (_, "min-gap") => "the shortest gap between two input pulses that still comes out as two pulses",
        (_, "min-period") => "the fastest train of pulses it follows: one pulse every this many game ticks",
        (_, "edge") => "which input edge flips the output: rising = the input turning on, falling = turning off",
        (_, "function") => "the logic function the truth table names",
        (_, "class") => "instant = the output changes in the same game tick as the input (0 gt latency)",
        (_, "glitches") => "extra output changes while the inputs change",
        // (a piston extender's: clk-misc's flip check)
        ("piston-extender", "flips") => "what happens when the lever is flipped back before the block is all the way out: safe (nothing is stranded at any time) or the earliest flip that strands blocks",
        // (a piston bridge's: pext's bridge judge)
        ("piston-bridge", "flips") => "what happens when the lever is flipped back before the bridge is all the way out: safe (nothing is stranded at any time) or the earliest flip that strands blocks",
        ("piston-bridge", "width") => "how wide its walkway is, in blocks",
        ("piston-bridge", "extend") => "game ticks from the lever on to the bridge all the way out",
        ("piston-bridge", "retract") => "game ticks from the lever off to the bridge back in the wall",
        ("piston-bridge", "walkway") => "what you walk on",
        // (a randomizer's: crate::random)
        ("randomizer", "style") => "hold: the chosen output stays on until the next press; pulse: it gives one pulse",
        ("randomizer", "pulse") => "the shortest output pulse",
        ("randomizer", "presses") => "presses in each run of our test (24 per output, at least 48)",
        // (a crop farm's: crate::farm)
        ("harvester", "per cane") => "blocks placed for each cane it harvests (a farm made to be copied: its blocks per copy over its canes per copy)",
        // (a fuel farm's: w86's fuel-farm judge, designs/briefs/fuel-farm.md: its crop as furnace
        // fuel, at the game's normal growth speed)
        ("harvester", "items smelted per hour per block") => "furnace items its harvest smelts each hour as fuel, for each block placed, at the game's normal growth speed (a furnace smelts one item per 200 game ticks of fuel: bamboo burns 50, a quarter of an item each)",
        ("harvester", "items smelted per hour") => "furnace items the farm's whole harvest smelts each hour as fuel, at the game's normal growth speed (an 8-furnace super smelter at full speed takes 2,880 an hour)",
        // (the farm judge's speed, w86 2026-10-23: what the 3D view replays the farm at)
        ("harvester", "game rts") => "how fast our test grew the plants, as the game's random tick speed (normal play is 3): a farm of up to 8 plants at 300, a bigger one slower so its collection keeps up; the 3D view plays the farm at this speed",
        ("harvester", "items per hour per cane") => "items harvested each hour for each cane, at the game's normal growth speed (random tick speed 3), measured over 10 hours of game time by our simulator's twin of the game",
        ("harvester", "items per hour per block") => "items harvested each hour for each block placed, at the game's normal growth speed (random tick speed 3), measured over 10 hours of game time by our simulator's twin of the game: the farm's efficiency, what its pick is ranked by",
        ("harvester", "items per hour") => "items the whole design harvests each hour, at the game's normal growth speed (random tick speed 3), measured over 10 hours of game time by our simulator's twin of the game",
        ("harvester", "harvest height") => "how tall a cane gets before the farm cuts it: 2, each block is harvested as soon as it grows; 3, when the cane is full",
        ("harvester", "wait") => "the longest a grown block stood before the farm harvested it",
        (_, "carry") => "carry in and out: it takes a carry input and gives a carry output, so copies chain into wider adders; carry out: a carry output only",
        (_, "enable") => "high: the latch follows D while E is on; low: while E is off",
        (_, "output") => "how the output shows the count: toggles (flips every N), pulses (one pulse every N) or level (its state repeats every N)",
        (_, "extend") => "game ticks from switching the input on until the block is fully out",
        (_, "retract") => "game ticks from switching the input off until the block is back",
        (_, "speed") => "game ticks per block travelled (20 gt = 1 second)",
        (_, "period") => "game ticks per cycle of the engine (one push of each piston)",
        (_, "two-way") => "goes and comes back by itself",
        _ => "",
    }
}

/// Study 11: what the judge's test (the view's demo) does to the input, in words.
fn study11_demo(c: &Circuit) -> Option<String> {
    let n = kind_number(&c.kind).unwrap_or(0);
    let l = with_seconds(metric(c, "L"));
    Some(match base(&c.kind) {
        "burst" => format!("single input pulses of 1 to 40 gt (each must give exactly {n} output pulses), then pairs of presses closer and closer together (what a second press does), then trains of 6 presses faster and faster (how fast it keeps up)."),
        "divider" => format!("a square clock on the input, first slow (a pulse every 40 gt, 2 s), then faster and faster down to a pulse every 2 gt; the output lamp goes through one on-off cycle for every {n} input pulses, until the clock gets faster than the design can follow."),
        "monostable" => format!(
            "single input pulses of 1 to 160 gt (each gives one output pulse of {l}), then pairs of short pulses closer and closer together: {}",
            if c.kind.contains("non-retriggerable") { "a second pulse while the output is on is ignored." } else if c.kind.contains("retriggerable") { "a second pulse while the output is on restarts the count, so the output turns off L after the second one." } else { "what a second pulse while the output is on does." }
        ),
        "off-delay" => format!("input pulses of 1 to 80 gt (each output pulse lasts {l} longer than its input), then groups of 3 short pulses with growing gaps (closer than L, they merge into one output pulse)."),
        "on-delay" => format!("input pulses of 1 to 80 gt: the ones shorter than {} give nothing, the longer ones come out shortened.", with_seconds(metric(c, "T"))),
        "pulse-train" => format!("the input held on for 20, 60 and then 200 gt: the output pulses every {} while it is on and stops after.", with_seconds(metric(c, "P"))),
        _ => return None,
    })
}

/// Study 11: what to do with the input to see the design work.
fn study11_try(c: &Circuit) -> String {
    let n = kind_number(&c.kind).unwrap_or(0);
    let l = with_seconds(metric(c, "L"));
    match base(&c.kind) {
        "burst" => format!("each tap gives exactly {n} flashes of the output"),
        "divider" => format!("tap it {n} times and the output goes through one on-off cycle"),
        "monostable" if c.kind.contains("non-retriggerable") => format!("a tap keeps the output on for {l}; tapping again while it is on changes nothing"),
        "monostable" => format!("a tap keeps the output on for {l}; tap again while it is on and it stays on {l} after your last tap"),
        "off-delay" => format!("turn the input on, then off: the output stays on {l} after you turn it off"),
        "on-delay" => format!("a quick tap gives nothing; hold it longer than {} and the output lights until you let go", with_seconds(metric(c, "T"))),
        "pulse-train" => format!("turn the input on and the output pulses every {} until you turn it off", with_seconds(metric(c, "P"))),
        _ => String::new(),
    }
}

/// Study 11: the judge's phase labels in words, for the view's demo badge and the timing diagram
/// (`clock 40` -> `clock, a pulse every 40 gt`); other kinds keep theirs.
pub fn phase_label(kind: &str, label: &str) -> String {
    phase_label_min(kind, label, None)
}

/// [`phase_label`] knowing the shortest input the design handles: the pulse circuits' sweep
/// (`pulse 2`, `pulse 3` ... `pulse 60`) reads `a 2-tick input: too short, ignored` below it
/// (critique: the sculk filter's demo played 2-tick inputs it ignores, with no word about it).
pub fn phase_label_min(kind: &str, label: &str, min_pulse: Option<u32>) -> String {
    if matches!(base(kind), "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender") {
        let mut w = label.split_whitespace();
        if let (Some("pulse"), Some(n), None) = (w.next(), w.next().and_then(|n| n.parse::<u32>().ok()), w.next()) {
            return match min_pulse {
                Some(m) if n < m => format!("a {n}-tick input: too short, ignored"),
                _ => format!("a {n}-tick input"),
            };
        }
    }
    phase_label_any(kind, label)
}

fn phase_label_any(kind: &str, label: &str) -> String {
    // a BUD's judge: `place 1 t`, `state 2b t`, `setup t`, `probe 3 place`, `rearm place 6 2`
    if base(kind) == "bud" {
        return bud_phase(label);
    }
    let mut w = label.split_whitespace();
    let (what, v) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    // a memory's inputs by name: `s pulse 4` -> `a 4-tick press on set (S)` (critique: the demo
    // badge read "demo: r: a 4 gt pulse")
    if let (Some("pulse"), Some(n), None) = (Some(v), w.clone().next(), w.clone().nth(1)) {
        if n.parse::<u32>().is_ok() {
            let input = match (base(kind), what) {
                (_, "s") => "set (S)".to_string(),
                (_, "r") => "reset (R)".to_string(),
                (_, "e") => "enable (E)".to_string(),
                (_, "d") => "data (D)".to_string(),
                (_, "c" | "clk") => "clock (C)".to_string(),
                (_, other) => other.to_uppercase(),
            };
            return format!("a {n}-tick press on {input}");
        }
    }
    if v.is_empty() || w.next().is_some() || v.parse::<u32>().is_err() {
        return label.to_string();
    }
    match (base(kind), what) {
        ("divider", "clock") => format!("a clock pulsing every {v} ticks"),
        ("pulse-train", "pulse") => format!("input on for {v} ticks"),
        ("burst" | "monostable" | "off-delay" | "on-delay", "pulse") => format!("a {v}-tick press"),
        ("burst" | "monostable", "pair") => format!("2 presses {v} ticks apart"),
        ("burst", "train") => format!("6 presses, {v} ticks apart"),
        ("off-delay", "train") => format!("3 presses, {v}-tick gaps"),
        _ => label.to_string(),
    }
}

/// A BUD judge's phase in words (the demo badge, the diagram): `place 1 t` -> `place #1`, `state
/// 2b t` -> `change back #2`, `shape 1 t` -> `shape-only #1`, `update 3 t` -> `pure update #3`,
/// `setup t` -> `setting up`, `probe 3 place` -> `the cells around (3)`, `rearm place 6 2` -> `2
/// changes 6 gt apart`.
fn bud_phase(label: &str) -> String {
    let w: Vec<&str> = label.split_whitespace().collect();
    let class = |c: &str| -> String {
        match c {
            "place" => "place",
            "remove" => "remove",
            "state" => "change",
            "shape" => "shape-only",
            "update" => "pure update",
            other => other,
        }
        .to_string()
    };
    match w.as_slice() {
        ["idle"] => "idle".to_string(),
        ["setup", ..] => "setting up".to_string(),
        ["relatch", ..] => "a second change while it holds".to_string(),
        ["probe", k, ..] => format!("the cells around ({k})"),
        ["rearm", _, g, ..] => format!("2 changes {g} gt apart"),
        [c @ ("place" | "remove" | "state" | "shape" | "update"), k, ..] => {
            let (n, back) = match k.strip_suffix('b') {
                Some(n) => (n, true),
                None => (*k, false),
            };
            format!("{}{} #{n}", class(c), if back { " back" } else { "" })
        }
        _ => label.to_string(),
    }
}

/// A metric's value with seconds (`4 gt (0.2 s)`).
pub fn metric_value(v: &str) -> String {
    let v = v.trim();
    if let Some(n) = v.strip_suffix(" gt").and_then(|n| n.trim().parse::<u32>().ok()) {
        return format!("{n} gt ({})", real_time(n));
    }
    // a strength judge's worst change (`32 gt (a goes 0 -> 2 from a=0)`: clk-obs's displays) in
    // words: `32 gt (1.6 s; the slowest change: 0 to 2)`
    if let Some((head, rest)) = v.split_once(" gt (")
        && let Ok(n) = head.trim().parse::<u32>()
        && let Some((_, change)) = rest.trim_end_matches(')').split_once(" goes ")
        && let Some((from, to)) = change.split_once(" -> ")
    {
        let to = to.split_whitespace().next().unwrap_or(to);
        return format!("{n} gt ({}; the slowest change: {from} to {to})", real_time(n));
    }
    v.to_string()
}

/// The studies, in order: (folder, title, how we test and rank them). What a visitor would use
/// them for is [`STUDY_USES`].
pub const STUDIES: [(&str, &str, &str); 25] = [
    (
        "pulse",
        "Pulse shapers",
        "Circuits that turn one signal shape into another. Edge detectors make one short pulse when the input turns on (rising edge), off (falling edge) or both (dual edge); pulse limiters cut long pulses down to a fixed length; pulse extenders stretch short pulses to at least that length. For each output pulse length we keep the design with the fewest blocks placed (then the smallest box); each kind's page lists them, length by length.",
    ),
    (
        "tff",
        "Toggle switches (T\u{2011}flip\u{2011}flops)",
        "Each input pulse flips the output (on, off, on...). We tested one design per mechanism (the blocks that change state: copper bulbs, observers and pistons, droppers and hoppers, comparators...) and show the one with the fewest blocks placed; we also measure how short a pulse and how fast a series of presses it follows.",
    ),
    (
        "delay",
        "Delays",
        "The output is the input a fixed number of game ticks later, both when it turns on and when it turns off, pulse lengths kept. For each delay we keep the design with the fewest blocks placed. Families that grow by a repeated step (one more repeater or comparator per 2 ticks) cover every delay.",
    ),
    (
        "gates",
        "Logic gates",
        "NOT and every function of two inputs (AND, OR, XOR, XNOR, NAND, NOR, and the A AND NOT B kinds), each checked in every input combination and every change between them. For each function we keep the design with the fewest blocks placed, then the fastest; a gate and its mirror (A and B swapped) are one.",
    ),
    (
        "memory",
        "Memory and counters",
        "Circuits that remember: on/off memories with a set and a reset input (RS latches), copy-and-hold memories (D latches), clocked memories and counters that react every Nth input pulse. For each function (and each N for counters) we keep the design with the fewest blocks placed.",
    ),
    (
        "pext",
        "Piston extenders",
        "Machines that push a block several blocks out with one input and bring it back exactly with the next, nothing left behind (a plain piston reaches 1). For each reach we keep the most compact design, the one with the smallest box in any shape (every cell it fills while it runs), then the fewest blocks, then the fastest out and back; the extender's page lists every reach.",
    ),
    (
        "bridge",
        "Piston bridges",
        "A piston bridge hides in a wall: flip its lever and a row of blocks slides out across a gap; flip it back and the row slides back flush into the wall, leaving the gap open. Our test runs it in the real game, in every position and rotation, and flips the lever back at every moment too: the bridge must never leave a block behind. For each gap length we keep the smallest box, then the fewest blocks, then the fastest.",
    ),
    (
        "fly",
        "Flying machines",
        "Self-propelled structures of pistons, slime or honey and observers: once started they move one block at a time by themselves. We show three: the smallest flat machine (its engine side by side, 1 block tall), the smallest tall machine (its engine stacked; it carries extra blocks) and the smallest machine with docking stations, which flies there and back.",
    ),
    (
        "shulker",
        "Shulker box loaders and unloaders",
        "Only a dispenser can place a shulker box and only a piston can break one back into an item with its contents, so every design is a dispenser, hoppers, a comparator that knows when the box is full (or empty), a piston and a hopper collecting the drop. Our test, in the real game, supplies the items and the boxes from chests at the design's ports and collects what comes out: a loader must deliver three full boxes from three boxes' worth of items, an unloader must empty three full boxes, nothing lost or duplicated, at 4 positions and facing 4 ways, then 64 more runs with other drop draws (a design that loses a drop now and then fails). Extra scenarios give badges: 16-stack and unstackable items, mixed items, a filter, always on, a box supply that runs out, a paused input, colours and names kept, a part-full box flushed when the input stops, no item left on the floor, tileable side by side, loader and unloader in one... We show the smallest unloader, and the loader that tiles side by side (1 block wide), the one builders asked for; each card's badges list the scenarios it passes.",
    ),
    (
        "tnt",
        "TNT dupers",
        "A piston push copies every moved block before it clears the old cells; a TNT that gets a block update while powered in that instant is lit where it stands, and its moved copy still lands as a TNT block. We show the self-running duper picked for builders (it works standing on the ground), the precise ones (every TNT goes off at one exact point: in mid-air, or straight under the machine) and the smallest duper that lights TNT on a press; each page says where it can stand.",
    ),
    (
        "bud",
        "Block change detectors (BUD switches)",
        "A block placed, removed or changed at the target cell fires the output, although nothing there powers it (players call such a detector a BUD, for block update detector). Our test tries five kinds of change at the target (place, remove, a state change, a shape-only change, a pure update with nothing changing) and measures which ones a design notices, how fast it reacts, its pulse, how soon it can fire again and which cells around it must stay free. We show the smallest, a single observer; its page says which changes it misses.",
    ),
    (
        "pulsemult",
        "Timers and pulse multipliers",
        "A burst turns one press into exactly N pulses; a clock slower (divider) slows a running clock by N; a one-shot timer (monostable) makes one pulse of a fixed length per press, and either ignores presses while it is on or restarts its countdown on every press; a stay-on timer (off-delay) keeps the output on a while after the input turns off; a hold-to-activate (on-delay) ignores presses shorter than a set time; a rapid-fire circuit (gated clock) pulses at a fixed beat while its input is on. Our test measures each one's main number and how it behaves under fast or repeated presses; for each value we keep the design with the fewest blocks placed, and each kind's page lists them value by value. A one-shot timer fed single presses is a pulse shaper's edge detector, slowing a clock by 2 is a toggle switch and counting slow pulses is a counter: those are in their own collections.",
    ),
    (
        "sorter",
        "Item sorters",
        "A line of hoppers carries mixed items across a row of slices; each slice takes one item kind into its chest and lets the rest go on. Its filter hopper holds that kind plus a few filler items, so a comparator reading it sees one item more than usual and briefly unlocks the hopper under it, which drops exactly that item into the chest. Our test, in the real game, sends mixed items down the line and counts every one: each kind must end in its chest, the rest at the end of the line, nothing lost; then slices side by side, and a full chest that must not jam the line or drain a neighbour. We show the classic one-block-wide slice, the one builders tile into storage halls.",
    ),
    (
        "lock",
        "Combination locks",
        "The dials are item frames: a right-click turns a frame's item one step of eight, and a comparator reading the frame through the block it hangs on gives that step plus one. Each dial has its own code frame, on the other side of the build; the lock compares the two values both ways with two subtracting comparators, so any difference, up or down, reaches the output's torch and keeps the door shut. Our test, in the real game, turns the frames like a player, one right-click at a time: it tries every combination of the dials (all 512 for three dials), checks the lamp lights at the code and nowhere else, never even for a moment on the way through, then sets a new code by turning the code frames and tries every combination again. The study also holds button sequence locks (a password door): their buttons must be pressed in the order the item frames on the back set; our test presses right, wrong and too-short sequences, and the output must turn on after the code's last right press and nowhere else, and turn off at the next press.",
    ),
    (
        "display",
        "Displays",
        "A 7-segment display shows a number as a digit lit in lamps: its decoder turns a signal into the lit segments of a 3×5 lamp face with its corners lit (one lamp per segment does not read as a digit). The 0–9 display reads one signal, 0 to 15, from a container through one comparator, and shows 0 to 9. Our test, in the real game, fills the input chest to every level and makes every change between two digits (all 90), checks every lamp of the face after each change, measures the slowest change, and counts the lamps that light wrongly on the way: briefly (glitches) or long enough to see (on-blips). For each kind we keep the design with the fewest blocks placed (the face's lamps and the input chest not counted).",
    ),
    (
        "adder",
        "Adders",
        "An adder adds binary numbers with redstone: a half adder adds two bits into a sum and a carry, a full adder also takes a carry in, and full adders chained carry to carry add numbers of any width (an adder slice is one made to be copied side by side). Our test, in the real game, drives every input combination in every order, one input and all inputs at once, and the outputs must settle to the right sum and carry every time; a slice is judged as 1, 2, 3, 4 and 8 copies. For each kind we keep the design with the fewest blocks placed.",
    ),
    (
        "random",
        "Randomizers",
        "A randomizer turns a press into a random choice: each press lights exactly one of its outputs, every output as often as the others (a coin flip with 2 outputs, a die with 6). The randomness is the game's own: a dropper or a dispenser fires a random one of its filled slots. Our test, in the real game, presses it like a button, 24 times per output at several paces, in 16 builds, and checks that every press lights exactly one output; then that the outputs came up equally often over all those presses (a chi-square test: a split as uneven as 1 in 1000 fair ones fails). For each number of outputs we keep the design with the smallest box.",
    ),
    (
        "farm",
        "Crop farms",
        "A crop farm harvests by itself: the crop grows by random ticks, the farm breaks every grown block and sends the item into a chest, forever. Our test, in the real game, plants the crop, speeds growth up 100 times for 20 game minutes, drops foreign items onto it halfway, and every harvested item must end in the chest, in every position and rotation (a farm made to be copied is tested as copies side by side too). For each crop we keep the full-size farm (64 plants or more) that gathers the most an hour for each block placed, at the game's normal growth speed, with the best small farm linked beside it; a crop with no full-size farm yet shows its best small one.",
    ),
    (
        "smelter",
        "Auto-smelters",
        "Hoppers carry items from an input chest into the top of each furnace and coal from a fuel chest into its side; the hopper under each furnace takes the results to one output chest. Our test, in the real game, fills the input chest with a mixed load (raw gold and copper, cobblestone, sand, logs, kelp, iron tools) and the fuel chest with coal, and counts everything: every item must come out smelted, no coal in the results or in a furnace's input, no more coal burnt than the items needed plus one a furnace, and every furnace must get its share; then slices side by side, and stray items dropped on every open hopper. The arrays players build (8 or 16 furnaces) are also timed on a double chest: 3,456 raw iron dumped in the input chest and the coal they need plus one stack (496, never refilled) in the fuel chest, until the last ingot is in the output chest; every raw iron must come out an ingot, nothing jams, nothing stays in the machine but coal. We show the quickest for each number of furnaces (then the fewest blocks; one not timed that way, the smallest), with its blast-furnace version linked beside it (twice as fast, but blast furnaces smelt only ores and raw metals).",
    ),
    (
        "brew",
        "Auto-brewers",
        "Brewing stands fed by hoppers: water bottles and blaze powder come from their chests, and the ingredients of each stage (a nether wart for awkward potions, then a sugar for swiftness) are put in one at a time, in order; the hopper under each stand takes the finished potions to one output chest. Our test, in the real game, fills the chests for four batches a stand and counts everything: every bottle must come out as a potion of swiftness (no water bottle, awkward or mundane potion: an unfinished batch or the wrong order), every wart and sugar used, no more blaze powder burnt than the brews need, nothing left in the machine; then stray items dropped on every open hopper, at 4 positions and facing 4 ways. We show the smallest build for each number of stands.",
    ),
    (
        "compost",
        "Auto-composters",
        "A composter takes items only from a hopper above it and only compostable ones, so a hopper over a composter would keep every other item for good: an auto-composter first sorts its crops out of the line with the item sorter's filter, one crop a slice, and drops them one by one into the slice's composter; the hopper under the composter pulls the bone meal into a chest. Our test, in the real game, feeds a farm's mixed output (wheat seeds, wheat, kelp and sugar cane, with cobblestone, iron ingots and iron tools between them) into the line and counts everything: every crop must be composted, every other item must reach the overflow chest, all the bone meal must reach its chests and no composter may end full; at 4 positions and facing 4 ways, as a row of slices. Composting is random (each item has its own chance of filling the composter), so the bone meal varies; we rank by blocks per composter.",
    ),
    (
        "elevator-items",
        "Item elevators",
        "An item elevator lifts items straight up: a hopper takes them from a chest at the bottom, a column of droppers fires them up block by block, and the top one pushes them into a chest at the top. Our test, in the real game, fills the input chest with 193 mixed items, empties the output chest every tick, and every item must arrive at the top, none lost or stuck; it measures how many items a second it lifts and how soon the first one arrives. For each height we keep the design with the fewest blocks placed.",
    ),
    (
        "rail",
        "Minecart stations",
        "A minecart station on a rail line: a cart arriving stops at the platform, and a button press sends it on its way. Our test, in the real game, sends 17 carts in, with and without a rider, from a slow crawl to full booster speed. Every one must stop where the rider can reach and see the button, wait for the press, then leave in its direction. A two-way station must also let carts coming the other way pass straight through. The study also holds hopper-minecart unloaders and loaders: our test sends 12 hopper minecarts in at speeds from a crawl to full booster speed, with mixed loads, full stacks, 16-stackables, single tools and empty carts, and counts every item: the unloader must put every item in the chest and send each cart back only once it is empty, the loader must fill each cart until it is full or the supply runs out, then send it off, and an empty cart at an empty supply must wait there until items come; stray items are dropped on the open hoppers too. For each kind of line we keep the design with the fewest blocks. A whole line, a loader and an unloader with 128 blocks of track between them, is tested end to end: a double chest (3,456 items) carried by one cart, timed until the last item arrives with every item counted, then a farm's slow trickle, after which the cart must stay parked at the loader.",
    ),
    (
        "crafter",
        "Auto-crafters",
        "An auto-crafter crafts by itself with a crafter block: a hopper fills the crafter from a supply chest, a comparator sees when a full recipe is in, and a pulse crafts it into the output chest. Our test, in the real game, feeds it full and partial stacks at hopper speed: a partial set must wait without jamming and be crafted when the rest arrives, the count of crafts must be exact, and nothing may end up loose in the world. For each recipe we keep the design with the smallest box.",
    ),
    (
        "autodoor",
        "Automatic doors",
        "An automatic door opens by itself when someone walks up to it and closes behind them, with no button or lever to press. Our test, in the real game, sends player-sized walkers through it both ways, walking and sprinting, on the left, in the middle and on the right, two in a row; then two people, one behind the other at every gap from touching to 6 blocks, and from opposite sides crossing in the doorway: the door must open for every one of them, never touch them, never stay open with nobody inside, and close after they have gone. Last, twice, a walker stops in the doorway for 20 seconds: we report whether the door stays open for them or closes on them, without requiring either, and each page says which. We keep the design with the fewest blocks.",
    ),
];

/// What a visitor would use each study's builds for, with a concrete example (critique P5): the
/// first thing a study page and the circuits index say; the ranking rules fold under "How we test".
pub const STUDY_USES: [(&str, &str); 25] = [
    ("pulse", "Turn a lever, a button or an observer's flicker into exactly the pulse a machine needs: one short pulse when a lever is flipped (fire a dispenser once), a long press cut short, or a 1-tick blip stretched so a piston notices it."),
    ("tff", "Turn a button into an on/off switch: press once to open a door, press again to close it."),
    ("delay", "Make something happen a moment later: pistons that fire one after another, a door that waits before it closes. Pick the delay you need: its page lists the best design for every length."),
    ("gates", "Combine signals: a door that needs two levers (AND), two buttons for one door (OR), a light switched from both ends of a hallway (XOR), a lamp that is on while a lever is off (NOT)."),
    ("memory", "Remember things: a light that stays on after someone crosses a tripwire until you reset it, the digits of a combination lock, or doing something every 4th item through a hopper."),
    ("pext", "Push a block several blocks away with one lever and pull it back exactly: hidden doors, extending bridges, secret staircases. The reach is how far the block goes."),
    ("bridge", "A hidden bridge over a moat, a ravine or a lava pit: out at the flip of a lever, gone again at the next."),
    ("fly", "Machines that move themselves through the world with no power line: tree farms, quarries, world eaters, slime trains. Started once, they keep flying."),
    ("shulker", "Box up a farm's output automatically, or unpack shulker boxes into a storage system, without opening them by hand."),
    ("tnt", "Endless lit TNT without crafting any, for world eaters, tunnel bores and cannons."),
    ("bud", "Notice a change you cannot otherwise see: a crop or a tree that has grown, a block placed or mined, to start a harvester or an alarm."),
    ("pulsemult", "Timers and pulse tools: a 5-second light from one button press, however long the button is held (one-shot timer); a light that stays on 10 seconds after you step off a pressure plate (stay-on timer); a door that ignores accidental taps (hold-to-activate); a dispenser that fires exactly 3 times per press (burst); a clock slowed down (clock divider)."),
    ("sorter", "Sort a farm's mixed output into chests, one item kind per chest: build one slice per kind side by side under a line of hoppers."),
    ("lock", "A secret door or vault: it opens only when every item frame on the wall is turned to the right rotation, or when its buttons are pressed in the right order."),
    ("display", "Show how full a chest is as a digit you read at a glance: put any container behind the display, and its 3×5 lamp face shows 0 to 9 (one digit for now)."),
    ("adder", "Add numbers in redstone: a calculator, a counter display that adds, the arithmetic unit of a redstone computer."),
    ("random", "Make a game random: a coin flip, a dice roll, a random door in a maze, a loot or lottery machine, a mob arena that picks its next wave."),
    ("farm", "Endless crops without replanting or collecting: sugar cane for paper, books and rockets, bamboo for scaffolding, kelp to dry into fuel blocks, pumpkins and melons to trade or eat, cactus for green dye."),
    ("smelter", "Smelt a pile of ore, sand or logs without standing at a furnace: dump the items in the input chest and coal in the fuel chest, and collect everything smelted from one chest."),
    ("brew", "Brew potions without standing at a brewing stand: fill one chest with water bottles, one with blaze powder and one each with the ingredients, and collect finished potions of swiftness from one chest."),
    ("compost", "Turn a farm's spare crops into bone meal without sorting them by hand: send the farm's hopper line through one slice per crop, collect the bone meal from the chests, and anything else from the overflow chest at the end."),
    ("elevator-items", "Send items up: from a farm's collection chest at the bottom of a base up to the storage room, or from a mine up to the surface, with no water streams or ladders of hoppers."),
    ("rail", "Stops on a rail line: get off at a station, press the button to ride on; hopper-minecart stops that empty a cart into a chest or fill it from one; and a whole line that carries a farm's items to storage 128 blocks away by itself."),
    ("crafter", "Craft without clicking: iron blocks for storage, sticks and crafting tables in bulk, fed straight from a farm or a storage system."),
    ("autodoor", "A door that opens as you walk up and closes behind you: a base entrance, a shop or a vault, with no button to press and no door left open."),
];

/// A study's plain intro ([`STUDY_USES`]).
pub fn study_uses(folder: &str) -> &'static str {
    if let Some(s) = circuit::registry::study(folder) {
        return s.site_uses;
    }
    STUDY_USES.iter().find(|s| s.0 == folder).map(|s| s.1).unwrap_or("")
}

/// A study's one line on the circuits index: what you get, concretely (critique: "Combine
/// signals." said nothing).
/// What a kind's obvious design takes, said in its Measurements (the coordinator 2026-10-22: body
/// only, not on the card): (study, kind, the line), from the study's brief (clk-obs,
/// designs/briefs/display-7seg.md: the 0-9 obvious bar, 1668 blocks).
const OBVIOUS: [(&str, &str, &str); 1] = [("display", "seven-segment 4 hex decimal", "For comparison: a straightforward one-hot decoder built by our generator takes 1,668 blocks for the same digit.")];

/// A kind's obvious-design line ([`OBVIOUS`]), for its Measurements.
pub fn obvious_line(c: &Circuit) -> Option<&'static str> {
    OBVIOUS.iter().find(|(s, k, _)| *s == c.study && *k == c.kind).map(|(_, _, l)| *l)
}

pub fn study_blurb(folder: &str) -> &'static str {
    if let Some(s) = circuit::registry::study(folder) {
        return s.site_blurb;
    }
    match folder {
        "pulse" => "One short pulse from a lever, long presses cut short, quick blips stretched.",
        "tff" => "One button that turns things on, then off.",
        "delay" => "The same signal, a set number of ticks later.",
        "gates" => "AND, OR, NOT, XOR: a door that needs two levers, a light switched from both ends.",
        "memory" => "Remember an event until reset, or react on every 2nd, 3rd... pulse.",
        "pext" => "Push a block 2 to 12 blocks out and pull it back exactly.",
        "bridge" => "Bridges that slide out of a wall across a gap when you flip a lever, and slide back flush to leave the gap open.",
        "fly" => "Machines that fly by themselves: trains, tree farms, world eaters.",
        "shulker" => "Box a farm's output into shulker boxes, or unpack them into storage.",
        "tnt" => "Endless lit TNT, without crafting any.",
        "bud" => "Notice a block placed, removed or grown next to it.",
        "pulsemult" => "One-shot timers, stay-on timers, several pulses per press, clock dividers.",
        "sorter" => "Sort mixed items into chests, one kind per slice, slices side by side.",
        "lock" => "Open a door only when every item frame shows its code, or when its buttons are pressed in the right order.",
        "display" => "Show a container's fill level as a 7-segment digit, 0 to 9.",
        "adder" => "Add bits: half adders, full adders, and slices that chain into any width.",
        "random" => "One press lights one of several outputs at random: coin flips, dice, lotteries.",
        "farm" => "Self-harvesting crop farms: the whole farm's harvest an hour, and per block placed.",
        "smelter" => "Furnaces fed from one input chest and one fuel chest, every result in one chest.",
        "brew" => "Brewing stands fed from chests, wart then sugar in order, finished potions in one chest.",
        "compost" => "Composters fed from a farm's hopper line, one crop a slice, the bone meal in chests.",
        "elevator-items" => "Lift items N blocks up from a chest into a chest, as fast as a hopper feeds them.",
        "rail" => "Stop a cart at the platform, send it on with a button; load and unload hopper minecarts; carry a chest's items far away.",
        "crafter" => "Crafters that craft by themselves from a supply chest: iron blocks, crafting tables, sticks.",
        "autodoor" => "A 2×2 piston door that opens by itself as you walk up and closes behind you.",
        _ => "",
    }
}

pub fn study(folder: &str) -> Option<(&'static str, &'static str)> {
    // a registered study's own words (circuit::registry)
    if let Some(s) = circuit::registry::study(folder) {
        return Some((s.site_title, s.site_intro));
    }
    STUDIES.iter().find(|s| s.0 == folder).map(|s| (s.1, s.2))
}

/// Every study page, in order: [`STUDIES`], then the registered studies not among them
/// (circuit::registry, in its order); a registered study's words win.
pub fn studies() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str, &'static str)> = STUDIES.to_vec();
    for s in circuit::registry::all_studies() {
        match v.iter_mut().find(|e| e.0 == s.id) {
            Some(e) => *e = (s.id, s.site_title, s.site_intro),
            None => v.push((s.id, s.site_title, s.site_intro)),
        }
    }
    v
}


/// A caption without its simulator attribution (vjeux: "we don't need the attribution"): every
/// sentence naming redsim goes.
pub fn scrub_attribution(caption: &str) -> String {
    if !caption.to_lowercase().contains("redsim") {
        return caption.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    for s in caption.split_inclusive(". ") {
        if !s.to_lowercase().contains("redsim") {
            out.push(s);
        }
    }
    out.concat().trim().trim_end_matches(|c: char| c == ',' || c == ';').to_string()
}

/// The design's caption without its bookkeeping prefix (`Counter 2, dropper ring, 8 blocks
/// (track memory, ...): ` -> the explanation after it).
pub fn how_sentence(caption: &str) -> String {
    site_model::html::esc(&how_text(caption))
}

/// [`how_sentence`] without the escaping (a schematic's description).
pub fn how_text(caption: &str) -> String {
    // the bookkeeping head first (`Counter 2, dropper ring, 8 blocks (track memory, ...): `), then
    // the source paths and tool commands left in the rest
    let c = caption.trim();
    if let Some(i) = c.find("): ") {
        let head = &c[..i];
        if i < 220 && head.contains('(') && (head.contains("track") || head.contains("census") || head.contains("found by") || head.contains("family")) {
            return first_cap(&drop_source_paths(&c[i + 3..]));
        }
        // a head naming the track that found it (`Dropper ring (clk-item): `, 448 clock
        // captions): the name stays, the track id goes (critique: raw ids on the page)
        if let Some(j) = head.rfind(" (clk-").filter(|&j| i < 220 && !head[j + 2..].contains(' ')) {
            return drop_source_paths(&format!("{}: {}", &head[..j], &c[i + 3..]));
        }
    }
    drop_source_paths(c)
}

/// [`caption_plain`] without the escaping: a caption a visitor can read, or nothing.
pub fn caption_text(caption: &str) -> String {
    let c = caption.trim();
    let lc = c.to_lowercase();
    let log = lc.starts_with("agents ") || lc.starts_with("agent ") || lc.starts_with("from track") || lc.starts_with("track ") || lc.starts_with("found by") || lc.starts_with("clock:") || lc.starts_with("circuit:") || lc.contains("`mctas ") || lc.contains("mctas clock") || lc.contains("mctas door") || lc.contains("census-k");
    if c.is_empty() || log {
        return String::new();
    }
    how_text(&drop_provenance(c))
}

/// A caption without its provenance notes: a parenthesis that says who asked for the design or
/// when (`(vjeux 2026-10-21: "Can we have an on off switch for all the tnt"; coordinator: ...)`,
/// `(site QA 2026-10-21: ...)`) is the archive's bookkeeping, not how the design works.
pub fn drop_provenance(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('(') {
        // the matching parenthesis (quotes may hold any character)
        let mut depth = 0;
        let mut end = None;
        for (k, ch) in rest[i..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i + k);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(j) = end else { break };
        let inner = &rest[i + 1..j];
        let note = ["vjeux", "coordinator", "site QA", "critique"].iter().any(|w| inner.contains(w)) || inner.contains("2026-");
        if note {
            out.push_str(rest[..i].trim_end_matches(' '));
        } else {
            out.push_str(&rest[..=j]);
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

/// A caption a visitor can read (escaped HTML), or nothing: agent logs ("Agents c77 + pockets3:
/// ...", "From track c77.") and tool logs ("Found by `mctas clock census` ...") are dropped
/// (critique C9 / C30); source paths are dropped from the rest.
pub fn caption_plain(caption: &str) -> String {
    site_model::html::esc(&caption_text(caption))
}

/// A door's community-regulation check in words, or None for the internal ones (critique C9:
/// "components S H G N E F, hipster -, t_player 48/50/15/26" mean nothing to a visitor).
pub fn door_prop(key: &str, value: &str) -> Option<(String, String)> {
    let v = value.trim();
    let yes_no = |v: &str| match v {
        "yes" | "true" => "yes".to_string(),
        "no" | "false" | "-" => "no".to_string(),
        other => other.to_string(),
    };
    let (label, value) = match key {
        "clean" => ("Nothing left in the doorway", yes_no(v)),
        "dents" => ("Dents in the wall", if v == "dentless" { "none".to_string() } else { v.to_string() }),
        "frame" => ("Frame blocks in place when open", v.replace('/', " of ")),
        "lever_support" => ("Lever on its own support block", yes_no(v)),
        "lever_surface" => ("Lever on the wall's surface", yes_no(v)),
        "risky" => ("Blocks that can be set off by accident", if v == "-" || v.is_empty() { "none".to_string() } else { site_model::html::human_blocks(&v.replace(',', "+")) }),
        "seamless" => ("Seamless", if v == "-" || v.is_empty() { "no".to_string() } else { site_model::html::seamless_words(v) }),
        "seamless_strict" => ("Seamless even while moving", yes_no(v)),
        _ => return None,
    };
    Some((label.to_string(), value))
}

/// Captions sometimes name the tool's source file in parentheses (`a comparator ring
/// (crates/circuit-timer/src/rings.rs): ...`): bookkeeping a visitor does not need.
fn drop_source_paths(c: &str) -> String {
    let mut s = c.to_string();
    let mut from = 0;
    while let Some(i) = s[from..].find('(').map(|i| i + from) {
        let Some(j) = s[i..].find(')').map(|j| j + i) else { break };
        let inner = &s[i + 1..j];
        // a source path, a tool command, a repo file or an agent track: not for visitors
        // (critique C15)
        if inner.contains("crates/") || inner.ends_with(".rs") || inner.contains(".rs ") || inner.contains("mctas ") || inner.contains(".md") || inner.contains("designs/") || inner.starts_with("track ") || inner.contains("; track ") {
            let start = if i > 0 && s.as_bytes()[i - 1] == b' ' { i - 1 } else { i };
            s.replace_range(start..=j, "");
            from = start;
        } else {
            from = j + 1;
        }
    }
    s
}

fn first_cap(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

/// The timing diagram of the judge's test (redsim): one row per input and output, lime for
/// inputs, orange for outputs (an output's height = its strength 0..15), the checker's phases
/// as separators with labels when they fit. Pixel sizes fixed (scrolls when long).
pub fn wave_svg(w: &Wave) -> String {
    wave_svg_sized(w, 960.0, false)
}

/// A timing diagram drawn about `target` px wide that scales to its container (`class="wave
/// fit"`): a clock's timeline fits a phone and fills a desktop column.
pub fn wave_svg_fit(w: &Wave, target: f64) -> String {
    wave_svg_sized(w, target, true)
}

fn wave_svg_sized(w: &Wave, target: f64, fit: bool) -> String {
    let ticks = w.ticks.max(1) as f64;
    let px = (target / ticks).clamp(if fit { 0.2 } else { 0.6 }, 12.0);
    let label_w = 64.0;
    let top = 22.0;
    // a position track is taller (its values 0..N each get a step)
    let height_of = |r: &site_model::WaveRow| if r.scale > 0 { 70.0 } else { 34.0 };
    let width = label_w + ticks * px + 8.0;
    let height = top + w.rows.iter().map(height_of).sum::<f64>() + 22.0;
    let x = |t: f64| label_w + t * px;
    let mut s = if fit {
        format!("<svg class=\"wave fit\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" aria-label=\"timing diagram\">")
    } else {
        format!(
            "<svg class=\"wave\" xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" aria-label=\"timing diagram of the judge's test\">"
        )
    };
    // phases: a separator at each start, a label when it fits; a start-up span is shaded
    for (from, to, label) in &w.phases {
        let (a, b) = (x(*from as f64), x(*to as f64));
        if label.starts_with("starting") {
            s.push_str(&format!("<rect class=\"warm\" x=\"{a:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\"/>", top - 4.0, (b - a).max(0.0), height - top - 14.0));
        }
        s.push_str(&format!("<line class=\"ph\" x1=\"{a:.1}\" y1=\"{:.1}\" x2=\"{a:.1}\" y2=\"{:.1}\"/>", top - 4.0, height - 18.0));
        if (b - a) >= label.len() as f64 * 6.2 + 6.0 {
            s.push_str(&format!("<text class=\"phl\" x=\"{:.1}\" y=\"{:.1}\">{}</text>", a + 3.0, top - 8.0, esc(label)));
        }
    }
    let mut row_top = top;
    for r in &w.rows {
        let row_h = height_of(r);
        let y0 = row_top + row_h - 8.0; // baseline
        let hmax = row_h - 14.0;
        row_top += row_h;
        let name = if r.name == "out" { "OUT".to_string() } else { r.name.to_uppercase() };
        s.push_str(&format!("<text class=\"wl\" x=\"4\" y=\"{:.1}\">{}</text>", y0 - 2.0, esc(&name)));
        s.push_str(&format!("<line class=\"base\" x1=\"{:.1}\" y1=\"{y0:.1}\" x2=\"{:.1}\" y2=\"{y0:.1}\"/>", x(0.0), x(ticks)));
        let level_y = |l: u8| -> f64 {
            if r.scale > 0 {
                y0 - hmax * (l as f64 / r.scale as f64).min(1.0)
            } else {
                // any signal shows: a weak comparator output (strength 1) still stands 40% tall
                let frac = if l == 0 { 0.0 } else { 0.4 + 0.6 * (l as f64 / 15.0) };
                y0 - hmax * frac
            }
        };
        if r.scale > 0 {
            // a position track: its scale on the left, the target as a dashed line, a step line
            s.push_str(&format!("<text class=\"ax\" x=\"4\" y=\"{:.1}\">{}</text>", y0 - hmax + 4.0, r.scale));
            if let Some(n) = r.target {
                let yt = level_y(n.min(255) as u8);
                s.push_str(&format!("<line class=\"tgt\" x1=\"{:.1}\" y1=\"{yt:.1}\" x2=\"{:.1}\" y2=\"{yt:.1}\"/><text class=\"tgtl\" x=\"{:.1}\" y=\"{:.1}\">N = {n}</text>", x(0.0), x(ticks), x(ticks) - 34.0, yt - 3.0));
            }
            let mut d = String::new();
            for (k, (t, l)) in r.steps.iter().enumerate() {
                let end = r.steps.get(k + 1).map(|n| n.0 as f64).unwrap_or(ticks);
                let y = level_y(*l);
                d.push_str(&format!("{}{:.1},{y:.1} L{:.1},{y:.1} ", if k == 0 { "M" } else { "L" }, x(*t as f64), x(end)));
            }
            s.push_str(&format!("<path class=\"wpos\" d=\"{}\"/>", d.trim_end()));
            continue;
        }
        // the judge's actions at a BUD's target: one mark each, coloured by what it does
        if !r.marks.is_empty() {
            for (f, what) in &r.marks {
                let xm = x(*f as f64);
                s.push_str(&format!("<line class=\"mk mk-{}\" x1=\"{xm:.1}\" y1=\"{y0:.1}\" x2=\"{xm:.1}\" y2=\"{:.1}\"/>", esc(what), y0 - hmax));
            }
            continue;
        }
        // the step outline as a filled polygon
        let mut pts: Vec<String> = vec![format!("{:.1},{y0:.1}", x(0.0))];
        for (k, (t, l)) in r.steps.iter().enumerate() {
            let end = r.steps.get(k + 1).map(|n| n.0 as f64).unwrap_or(ticks);
            let y = level_y(*l);
            pts.push(format!("{:.1},{y:.1}", x(*t as f64)));
            pts.push(format!("{:.1},{y:.1}", x(end)));
        }
        pts.push(format!("{:.1},{y0:.1}", x(ticks)));
        s.push_str(&format!("<polygon class=\"{}\" points=\"{}\"/>", if r.input { "win" } else { "wout" }, pts.join(" ")));
    }
    // a time axis: a label every 5, 10, 20, 50, 100... ticks, the unit said once at the axis's
    // left (critique: "5 gt, 10 gt, 15 gt"); a clock's chart (equal "cycle N" phases) puts its
    // labels on the cycle lines, every k cycles (critique: "5 ticks" sat across the cycle-2 line
    // drawn at tick 6), as close as the numbers fit (critique 41f1ec62a: "0, 5 ticks, 13" skipped
    // the 9 of a 4-tick clock)
    let cycles: Vec<&(u32, u32, String)> = w.phases.iter().filter(|(_, _, l)| l.starts_with("cycle ")).collect();
    let fits = 6.5 * (ticks as u32).to_string().len() as f64 + 12.0;
    let (first, step) = match cycles.first() {
        Some((a, b, _)) if b > a && cycles.iter().all(|(x, y, _)| y - x == b - a) => {
            let p = b - a;
            (*a, p * ((fits.max(24.0) / (p as f64 * px)).ceil() as u32).max(1))
        }
        _ => (0, [5u32, 10, 20, 50, 100, 200, 500, 1000].into_iter().find(|s| *s as f64 * px >= fits.max(40.0)).unwrap_or(2000)),
    };
    let ya = height - 6.0;
    let mut marks: Vec<u32> = Vec::new();
    if first > 0 && (first as f64) * px >= fits {
        marks.push(0);
    }
    let mut t = first;
    while (t as f64) <= ticks {
        marks.push(t);
        t += step;
    }
    s.push_str(&format!("<text class=\"ax\" x=\"4\" y=\"{ya:.1}\">ticks</text>"));
    for t in marks {
        // (a label that would run past the right edge is left out)
        let label = t.to_string();
        if t == 0 || x(t as f64) + 6.5 * label.len() as f64 <= width {
            s.push_str(&format!("<text class=\"ax\" x=\"{:.1}\" y=\"{ya:.1}\">{label}</text>", x(t as f64)));
        }
    }
    s.push_str("</svg>");
    s
}

/// A short line under a circuit card: its kind for people.
pub fn card_kind(b: &Build) -> Option<String> {
    b.circuit.as_ref().map(|c| kind_title(&c.kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsimulated_reasons_read_plainly() {
        // a pulse circuit's sweep says which inputs are too short for the design
        assert_eq!(phase_label_min("dual-edge", "pulse 2", Some(20)), "a 2-tick input: too short, ignored");
        assert_eq!(phase_label_min("dual-edge", "pulse 20", Some(20)), "a 20-tick input");
        assert_eq!(phase_label("bud", "place 1 t"), "place #1");
        assert_eq!(phase_label("bud", "state 2b t"), "change back #2");
        assert_eq!(phase_label("bud", "rearm place 6 t"), "2 changes 6 gt apart");
        assert_eq!(phase_label("bud", "probe 3 update"), "the cells around (3)");
        // every published kind has a title and a summary (the study pages' headings)
        for k in ["dff", "dff-enable", "shift 2", "tff-reset", "burst 2", "divider 3", "monostable retriggerable", "off-delay", "on-delay", "pulse-train"] {
            assert!(!kind_summary(k).is_empty(), "{k}");
        }
        assert_eq!(drop_source_paths("a comparator ring (crates/circuit-timer/src/rings.rs): 6 dust (C2) carry"), "a comparator ring: 6 dust (C2) carry");
        assert_eq!(plain_reason("not modeled by redsim (tick 28 of its run): dropper at (0,0,0) picks a random slot among 3 stacks (not modeled)"), "dropper picks a random slot among 3 stacks");
        assert_eq!(plain_reason("not modeled by redsim (tick 0 of its run): 2 container entities at (0,0,0): the game picks one at random (not modeled)"), "2 container entities: the game picks one at random");
        assert_eq!(plain_reason("block \"oak_fence\" is not modeled by redsim"), "block \"oak_fence\" is not modeled");
        assert!(!plain_reason("redsim does not model shulker boxes").contains("redsim"));
    }

    fn circuit(kind: &str, metrics: &[(&str, &str)], inputs: &[&str]) -> Circuit {
        Circuit {
            kind: kind.into(),
            metrics: metrics.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: vec!["out".into()],
            ..Default::default()
        }
    }

    #[test]
    fn counters_and_gates_read_plainly() {
        let c = circuit("counter 2", &[("N", "2"), ("output", "level every 2 pulses")], &["in"]);
        let w = what_it_does(&c);
        assert!(w.contains("counts input pulses"), "{w}");
        assert!(w.contains("divides the number of pulses by 2"), "{w}");
        let g = circuit("gate xor", &[("function", "xor"), ("latency", "0 gt"), ("class", "instant")], &["a", "b"]);
        let w = what_it_does(&g);
        assert!(w.contains("exactly one of A and B"), "{w}");
        assert!(w.contains("instant"), "{w}");
        // XOR's truth table: off on on off
        let t = truth_table("xor", &["a".into(), "b".into()]);
        assert_eq!(t.matches("class=\"on\">on</td></tr>").count(), 2, "{t}");
        let n = truth_table("not", &["in".into()]);
        assert!(n.contains("<th>IN</th>"), "{n}");
    }

    #[test]
    fn edge_and_delay_numbers_have_seconds() {
        let c = circuit("rising-edge", &[("L", "2 gt"), ("delay", "5 gt"), ("min-pulse", "4 gt")], &["in"]);
        let w = what_it_does(&c);
        assert!(w.contains("L = 2 gt (0.1 s)") && w.contains("5 gt (0.25 s) after"), "{w}");
        assert_eq!(metric_value("4 gt"), "4 gt (0.2 s)");
        assert_eq!(metric_definition("tff", "min-period"), "the fastest train of pulses it follows: one pulse every this many game ticks");
        assert_eq!(metric_definition("counter 3", "N"), "how many input pulses one output cycle takes");
    }

    #[test]
    fn how_to_try_names_the_controls() {
        let mut c = circuit("counter 2", &[], &["in"]);
        c.devices = vec!["redstone block".into()];
        c.stimulus = "in 40+ 44-".into();
        let h = how_to_try(&c, true);
        assert!(h.contains("press and hold the <b>IN</b> button"), "{h}");
        assert!(h.contains("judge's own test"), "{h}");
        let mut g = circuit("gate and", &[], &["a", "b"]);
        g.devices = vec!["lever".into(), "redstone block".into()];
        let h = how_to_try(&g, true);
        assert!(h.contains("click the <b>lever</b> tagged <b>A</b>") && h.contains("tap the <b>B</b> button"), "{h}");
        // the output: the judge's lamp when every output cell gets one, else the orange marker
        assert!(h.contains("orange glass marker"), "{h}");
        g.output_lamp = true;
        let h = how_to_try(&g, true);
        assert!(h.contains("<b>redstone lamp</b>") && !h.contains("orange glass"), "{h}");
    }

    #[test]
    fn captions_lose_their_bookkeeping() {
        assert_eq!(
            how_sentence("Counter 2, dropper ring, 8 blocks (track memory, `mctas circuit memory ring --n 2`): the input cell fires 2 droppers."),
            "The input cell fires 2 droppers."
        );
        assert_eq!(how_sentence("An observer watching the input."), "An observer watching the input.");
        assert_eq!(scrub_attribution("Two hoppers trade a sword. redsim: P=8 gt, warmup 2"), "Two hoppers trade a sword.");
        assert_eq!(scrub_attribution("No mention."), "No mention.");
    }

    #[test]
    fn wave_draws_rows_and_phases() {
        let w = Wave {
            ticks: 100,
            rows: vec![site_model::WaveRow::from_levels("in", true, &[0, 0, 15, 15, 0]), site_model::WaveRow::from_levels("out", false, &[0, 0, 0, 7, 15])],
            phases: vec![(0, 50, "pulse 2".into()), (50, 100, "pulse 3".into())],
        };
        let s = wave_svg(&w);
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
        assert_eq!(s.matches("<polygon").count(), 2);
        assert!(s.contains(">pulse 2</text>") && s.contains(">IN</text>") && s.contains(">OUT</text>"));
    }

    #[test]
    fn the_frame_probe_is_a_design_the_viewer_reads() {
        // (viewer_turns_frames presses the probe's input in the viewer: the probe must load, run,
        // and hold its one frame and one input, or the answer means nothing)
        let (before, after) = frame_probe_run().expect("the viewer runs the frame probe");
        eprintln!("frame probe: rotations {before:?} -> {after:?} (the view's buttons turn frames: {})", before != after);
        assert_eq!(before, vec![0]);
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn a_caption_head_loses_its_track_id() {
        // (critique e92a18d12: raw ids on an extended clock's page)
        assert_eq!(how_text("Dropper ring (clk-item): a hopper ring with dropper(s) in its bottom row."), "Dropper ring: a hopper ring with dropper(s) in its bottom row.");
        // a parenthesis that is not a track id stays
        assert_eq!(how_text("Ring (two droppers): a ring."), "Ring (two droppers): a ring.");
    }

    #[test]
    fn a_halt_in_words() {
        // the old redsim wording and the new one (explore/tff: "of different items (not modeled
        // without the level random)"): one plain sentence each, no "of items" twice
        let (t, why) = halt_words("tick 44: dropper at (1,0,2) picks a random slot among 9 stacks (not modeled)");
        assert_eq!((t, why.as_str()), (Some(44), "its dropper holds 9 stacks of items and the game fires a random one of them, which the browser's simulator does not model"));
        let (_, why) = halt_words("tick 44: dropper at (1,0,0) picks a random slot among 2 stacks of different items (not modeled without the level random)");
        assert_eq!(why, "its dropper holds 2 stacks of different items and the game fires a random one of them, which the browser's simulator does not model");
    }

    #[test]
    fn a_caption_loses_its_provenance_notes() {
        // (the TNT lever captions: who asked, when; not how it works)
        let c = "PRECISE DROP with an on/off LEVER (vjeux 2026-10-21: \"Can we have an on off switch for all the tnt\"; coordinator: clean stop, nothing left in the channel). A hopper ring (two droppers) releases it.";
        assert_eq!(caption_text(c), "PRECISE DROP with an on/off LEVER. A hopper ring (two droppers) releases it.");
        assert_eq!(caption_text("One TNT per BUTTON press (site QA 2026-10-21: a real button holds 20-30 gt)."), "One TNT per BUTTON press.");
    }
}
