//! One world-best design per kind (vjeux 21:47Z: "I want to have ONE world best implementation of
//! each of these instead of a bunch of working but mid implementations"). A kind is what a builder
//! asks for (a rising-edge detector, an AND gate, a shulker loader); a parametric kind (a piston
//! extender's reach, a counter's N, a delay's length, a flying machine's direction and load) is
//! one card whose page has a table of every size, each size's record keeping its page. Every other
//! design is not published (no card, no page, no search hit); its file stays in the repository.
//! `mctas site kinds` prints the plan for the coordinator.

use site_model::Build;
use std::collections::{BTreeMap, BTreeSet};

/// (Every kind's design is chosen by rule now, none by name: the coordinator 06:56Z, "pick the
/// smallest judged design of the kind automatically instead of a hard-coded id, so new records
/// don't need a site change". A size of a parametric kind still can be: SIZE_PICKS.)

/// What a kind's design must have where the ranking alone (robust, fewest blocks, fastest) would
/// pick against what builders ask for: (study/kind, what it must do, the test). The pick is the
/// best-ranked design that does it, so a new record needs no site change (the coordinator
/// 06:56Z: "pick the smallest judged design of the kind automatically instead of a hard-coded id").
type Rule = (&'static str, &'static str, fn(&Build) -> bool);
const RULES: [Rule; 9] = [
    // vjeux asked for 1-wide tileable shulker loaders (the unloaders that go with them likewise),
    // working with their chests kept full the way a player keeps them (the 3D view's endless run;
    // the judge gives a loader 3 boxes and a spare: webrender found the 28-block loader jams once
    // its box chest holds about 20)
    ("shulker/shulker-loader", "tiles side by side and keeps sending boxes out with its chests kept full", tiles_and_flows),
    ("shulker/shulker-unloader", "tiles side by side and keeps sending boxes out with its chests kept full", tiles_and_flows),
    // every TNT duper with an on/off switch (vjeux, via the coordinator 14:08Z: the lever
    // versions are the picks); the self-running one safe where it is meant to stand (vjeux 11:49Z:
    // its declared stand height, `stand: N`, the ground when it declares none); the designs without
    // a switch stay beside the picks ([`DEVICE_KINDS`]), the one that sits right on the ground too
    ("tnt/tnt-duper", "has an on/off lever and is safe at its own stand height", switch_and_safe),
    ("tnt/tnt-duper precise drop", "has an on/off lever", switchable),
    ("tnt/tnt-duper precise air burst", "has an on/off lever", switchable),
    // the flying machine with docking stations and an on/off lever (the coordinator 00:45Z); the
    // one-way machines started by their own lever (16:10Z), the ones without beside them
    ("fly/flying-machine two-way", "has an on/off lever", has_lever),
    ("fly/flying-machine flat", "starts with its own lever", has_start),
    ("fly/flying-machine tall", "starts with its own lever", has_start),
    // a true BUD switch (vjeux 11:46Z: "a single observer is not a BUD switch", it misses pure
    // block updates, the defining case): it notices the pure update, and resets itself (a pulse is
    // what starts a farm or an alarm; the coordinator 11:47Z)
    ("bud/bud", "notices a pure block update and resets itself", true_bud),
];

/// A full-size crop farm ([`site_model::html::full_size_farm`]: 64 plants or more).
fn full_size(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(site_model::html::full_size_farm)
}

/// The rule a kind's pick must meet ([`RULES`]), and every crop farm's: a full-size farm (the
/// coordinator via w86 2026-10-22: "farms rank by items/h per block among FULL-SIZE farms (64+
/// plants); the 1-to-4-plant modules stay as variants"; a crop with no full-size farm yet keeps
/// its best module, noted).
fn rule_for(path: &str) -> Option<Rule> {
    RULES.iter().copied().find(|(p, ..)| *p == path).or_else(|| path.split_once('/').is_some_and(|(_, k)| k.starts_with("harvester ")).then_some(("farm/harvester", "is a full-size farm (64 plants or more)", full_size as fn(&Build) -> bool)))
}

/// One line a kind's card says beside its pick (the coordinator 11:47Z): (study/kind, the line).
const MENTIONS: [(&str, &str); 1] = [("bud/bud", "the simpler 2-block BUD switch (a sticky piston and a redstone block) latches until you reset it; for visible block changes a single observer is enough")];

/// A kind's mention ([`MENTIONS`]), for its page too.
pub fn mention(study: &str, kind: &str) -> Option<&'static str> {
    MENTIONS.iter().find(|(p, _)| *p == format!("{study}/{kind}")).map(|(_, m)| *m)
}

fn metric<'a>(b: &'a Build, k: &str) -> &'a str {
    b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(n, _)| n == k)).map(|(_, v)| v.as_str()).unwrap_or("")
}

fn tiles_side_by_side(b: &Build) -> bool {
    metric(b, "badges").split('+').any(|x| x == "tileable(1)")
}

/// Tiles side by side, and its 3D view's run (the judge's chests kept full without end, as a
/// player keeps them; run at build time) sends boxes out (no run: no evidence against it).
fn tiles_and_flows(b: &Build) -> bool {
    tiles_side_by_side(b) && b.circuit.as_ref().and_then(|c| c.view_run.as_ref()).is_none_or(|v| v.boxes > 0)
}

fn on_the_ground(b: &Build) -> bool {
    metric(b, "safe drop").split_whitespace().next() == Some("0")
}

/// The ground variant a kind keeps beside its pick when the pick stands higher (vjeux 11:49Z: the
/// 22-block clocked duper, "sits right on the ground"): the best design of the kind that works on
/// the ground.
const GROUND_VARIANTS: [&str; 1] = ["tnt/tnt-duper"];

/// The ground variants of the chosen picks ([`GROUND_VARIANTS`]): (variant, pick) pairs.
fn ground_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        let path = format!("{study}/{kind}");
        let Some(&pick) = keep.first() else { continue };
        if !GROUND_VARIANTS.contains(&path.as_str()) || stand_height(&builds[pick]) == 0 {
            continue;
        }
        let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| on_the_ground(b) && b.id != builds[pick].id).collect();
        if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
            out.push((v, pick));
        }
    }
    out
}

/// The height a design is meant to stand at (`stand: N`, the judge's `stands` metric), 0: on the
/// ground.
pub fn stand_height(b: &Build) -> u32 {
    metric(b, "stands").split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// Safe at its own stand height: the judge's safe drop no higher than where it is meant to stand.
fn safe_at_stand(b: &Build) -> bool {
    metric(b, "safe drop").split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).is_some_and(|safe| safe <= stand_height(b))
}

/// Has an on/off switch: its axis says `switchable`, or the judge measured its switch (`switch
/// lever off: stops within 2 gt; ...`).
fn switchable(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.axis.contains("switchable")) || !metric(b, "switch").is_empty()
}

/// The self-running duper: an on/off switch (vjeux: on every TNT duper) and safe where it is
/// meant to stand.
fn switch_and_safe(b: &Build) -> bool {
    switchable(b) && safe_at_stand(b)
}

/// A one-way flying machine started by its own lever (the judge's `start flip the lever`; the
/// coordinator 16:10Z: "the one-way picks must include their start device").
fn has_start(b: &Build) -> bool {
    has_lever(b) || !metric(b, "start").is_empty()
}

/// The kinds whose pick has a device of its own, the best design without it kept beside it, a
/// page linked from the pick's (a property, not a kind): (study/kind, the device in words, its
/// test). The TNT dupers' on/off switch (the coordinator 14:08Z: the no-lever dupers are variants,
/// "no on/off switch, smaller"), the one-way flying machines' start lever (16:10Z: "no start
/// lever").
type Device = (&'static str, &'static str, fn(&Build) -> bool);
const DEVICE_KINDS: [Device; 5] = [
    ("tnt/tnt-duper", "on/off switch", switchable),
    ("tnt/tnt-duper precise drop", "on/off switch", switchable),
    ("tnt/tnt-duper precise air burst", "on/off switch", switchable),
    ("fly/flying-machine flat", "start lever", has_start),
    ("fly/flying-machine tall", "start lever", has_start),
];

/// The device a design's kind keeps on its pick, in words ([`DEVICE_KINDS`]): `on/off switch`,
/// `start lever`.
pub fn device_words(b: &Build) -> &'static str {
    let path = b.circuit.as_ref().map(|c| format!("{}/{}", c.study, kind_key(b))).unwrap_or_default();
    DEVICE_KINDS.iter().find(|(p, ..)| *p == path).map(|(_, w, _)| *w).unwrap_or("on/off switch")
}

/// The device-less variants of the chosen picks ([`DEVICE_KINDS`]): (variant, pick, the device in
/// words).
fn deviceless_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize, &'static str)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        let Some(&pick) = keep.first() else { continue };
        let path = format!("{study}/{kind}");
        let Some(&(_, words, has)) = DEVICE_KINDS.iter().find(|(p, ..)| *p == path) else { continue };
        if !has(&builds[pick]) {
            continue;
        }
        let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| !has(b)).collect();
        if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
            out.push((v, pick, words));
        }
    }
    out
}

/// A piston bridge's walkway width in blocks (its `width` metric, else its axis' "N cells wide").
fn bridge_width(c: &site_model::Circuit) -> u32 {
    c.metrics
        .iter()
        .find(|(k, _)| k == "width")
        .and_then(|(_, v)| v.split_whitespace().next()?.parse().ok())
        .or_else(|| c.axis.split(", ").find_map(|p| p.strip_suffix(" cells wide")?.trim().parse().ok()))
        .unwrap_or(1)
}

/// A piston bridge's best design with another walkway than its gap's pick (pext: "a 1-wide stone
/// walkway is the property beside the slime design of the same gap, both linked"): (variant,
/// pick) for each gap that has one.
fn walkway_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let walk = |b: &Build| b.circuit.as_ref().map(site_model::html::walkway_word).unwrap_or_default();
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if base(kind) != "piston-bridge" {
            continue;
        }
        for &pick in keep {
            let row = row_of(&builds[pick]).map(|r| r.0);
            let mine = walk(&builds[pick]);
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| row_of(b).map(|r| r.0) == row && !walk(b).is_empty() && walk(b) != mine).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// A piston extender's best design that is safe to flip back any time (clk-misc's `flips safe`),
/// for each size whose pick is not (the coordinator 2026-10-21: "a variant link from their size's
/// pick, like the TNT no-switch variants"): (variant, pick), the variant of the pick's own reach.
/// A crop farm whose pick sweeps its field with a flying machine: its crop's best full-size farm
/// without one, a page linked from the pick's and back (the coordinator 2026-10-22: the sweeper
/// is the pick, the piston lattice "its variant"; both pages say which to build): (variant, pick).
fn machine_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let machines = |b: &Build| b.circuit.as_ref().map(|c| c.flying_machines).unwrap_or(0);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !kind.starts_with("harvester ") {
            continue;
        }
        for &pick in keep {
            if machines(&builds[pick]) == 0 {
                continue;
            }
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| machines(b) == 0 && (full_size(b) || !full_size(&builds[pick]))).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// An automatic door whose pick closes on someone standing in the doorway: its kind's best door
/// that stays open for them, a page linked from the pick's and back (the coordinator 2026-10-22:
/// the obvious door the card, lock-v12 its "stays open for someone in the doorway" variant; the
/// judge reports the stop, it does not require it): (variant, pick).
fn doorway_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let stays = |b: &Build| b.circuit.as_ref().and_then(site_model::html::door_stays_open);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !kind.starts_with("auto-door") {
            continue;
        }
        for &pick in keep {
            if stays(&builds[pick]) != Some(false) {
                continue;
            }
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| stays(b) == Some(true)).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// The stations of every chosen minecart line (`Circuit::parts`, the site's `cart-line`: sorter
/// 2026-10-22, line-b is ld-chest, its track and ul-gate): (station, line, its kind's pick when the
/// station is not it). Each station gets a page, linked from the line's and back; one that is not
/// its kind's pick is that pick's variant (ul-gate, twice as fast as the smallest unloader).
fn line_stations(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize, Option<usize>)> {
    let mut out = Vec::new();
    for ((_, kind), keep) in chosen {
        if kind != "cart-line" {
            continue;
        }
        for &line in keep {
            for id in builds[line].circuit.as_ref().map(|c| c.parts.clone()).unwrap_or_default() {
                let Some(s) = builds.iter().position(|b| b.id == id) else { continue };
                let key = (builds[s].circuit.as_ref().map(|c| c.study.clone()).unwrap_or_default(), kind_key(&builds[s]));
                let pick = chosen.get(&key).and_then(|v| v.first().copied()).filter(|&p| p != s);
                out.push((s, line, pick));
            }
        }
    }
    out
}

/// A line's station kept beside its kind's pick, in a builder's words: how much faster it moves
/// items (`unload rate` / `load rate`), and that it is the line's.
fn station_claim(station: &Build, pick: &Build) -> String {
    let role = if station.circuit.as_ref().is_some_and(|c| c.kind.starts_with("cart-unloader")) { "the unloader" } else { "the loader" };
    match (station_rate(station), station_rate(pick)) {
        (Some(s), Some(p)) if p > 0.0 && s > p * 1.1 => {
            let r = s / p;
            let speed = if (1.8..=2.2).contains(&r) { "about twice as fast".to_string() } else { format!("{r:.1} times as fast") };
            format!("{role} of the minecart line, {speed} as the pick of its kind ({s} items a second against {p})")
        }
        _ => format!("{role} of the minecart line"),
    }
}

/// A minecart loader or unloader whose pick is not its smallest design (the pick moves the most
/// items a second): the smallest, a page linked from the pick's and back (the coordinator
/// 2026-10-22: cart-unloader-t3 beside ul-gate, "9 blocks smaller, half the speed"): (variant,
/// pick).
fn smaller_station_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !(kind.starts_with("cart-unloader") || kind.starts_with("cart-loader")) {
            continue;
        }
        for &pick in keep {
            let smallest = all.get(&(study.clone(), kind.clone())).into_iter().flatten().copied().filter(|&i| robust(&builds[i])).min_by_key(|&i| (builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), builds[i].name.clone()));
            if let Some(v) = smallest.filter(|&v| v != pick && builds[v].blocks < builds[pick].blocks) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// [`smaller_station_variants`]' claim: how many blocks smaller, and its speed against the pick's
/// ("half the speed").
fn smaller_station_claim(small: &Build, pick: &Build) -> String {
    let n = pick.blocks.saturating_sub(small.blocks);
    let blocks = format!("{n} block{} smaller", if n == 1 { "" } else { "s" });
    match (station_rate(small), station_rate(pick)) {
        (Some(s), Some(p)) if p > 0.0 && (0.45..=0.55).contains(&(s / p)) => format!("{blocks}, half the speed"),
        (Some(s), Some(p)) if p > 0.0 => format!("{blocks}, {s} items a second against the pick's {p}"),
        _ => blocks,
    }
}

/// A crop farm whose pick is full-size: its crop's full-size farm of the same kind (a flying
/// machine sweeps it, or none does) on about half the pick's field (40% to 60% of its plants),
/// the best such, a page linked from the pick's and back (the reviewer via the coordinator
/// 2026-10-22: bamboo-sweeper-10x32 beside the 10x64, "half the field"): (variant, pick).
fn half_field_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let machines = |b: &Build| b.circuit.as_ref().map(|c| c.flying_machines > 0).unwrap_or(false);
    let plants = |b: &Build| b.circuit.as_ref().map(|c| c.crops).unwrap_or(0) as f64;
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !kind.starts_with("harvester ") {
            continue;
        }
        for &pick in keep {
            if !full_size(&builds[pick]) {
                continue;
            }
            let p = &builds[pick];
            let list: Vec<&Build> = all
                .get(&(study.clone(), kind.clone()))
                .into_iter()
                .flatten()
                .map(|&i| &builds[i])
                .filter(|b| b.id != p.id && full_size(b) && robust(b) && machines(b) == machines(p) && (0.4..=0.6).contains(&(plants(b) / plants(p).max(1.0))))
                .collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// A crop farm whose pick is full-size: its crop's best small module (fewer than 64 plants), a
/// page linked from the pick's and back (the coordinator via w86 2026-10-22: "the 1-to-4-plant
/// modules stay as variants"): (variant, pick).
fn module_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !kind.starts_with("harvester ") {
            continue;
        }
        for &pick in keep {
            if !full_size(&builds[pick]) {
                continue;
            }
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| !full_size(b)).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// An auto-smelter's size whose pick has plain furnaces: that size's best design built of blast
/// furnaces (twice as fast, ores and raw metals only), a page linked from the pick's and back
/// (the coordinator 2026-10-22: "Blast-furnace versions are linked as variants"): (variant, pick).
fn blast_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let blast = |b: &Build| b.circuit.as_ref().is_some_and(site_model::html::smelter_blast);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if base(kind) != "smelter" {
            continue;
        }
        for &pick in keep {
            if blast(&builds[pick]) {
                continue;
            }
            let row = row_of(&builds[pick]).map(|r| r.0);
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| blast(b) && row_of(b).map(|r| r.0) == row).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

fn flip_safe_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let safe = |b: &Build| b.flags.iter().any(|f| f == "flip-safe");
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !matches!(kind.as_str(), "piston-extender" | "sand-elevator") {
            continue;
        }
        for &pick in keep {
            if safe(&builds[pick]) {
                continue;
            }
            let row = row_of(&builds[pick]).map(|r| r.0);
            let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| safe(b) && row_of(b).map(|r| r.0) == row).collect();
            if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// A true BUD switch: it notices the pure block update, and resets itself (not latched).
fn true_bud(b: &Build) -> bool {
    metric(b, "detects").split_whitespace().any(|w| w == "update") && !b.circuit.as_ref().is_some_and(|c| c.axis.contains("latched")) && !metric(b, "reset").contains("manual")
}

fn has_lever(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.devices.iter().any(|d| d == "lever"))
}

/// The size a parametric kind's card pictures, where its smallest is not the one builders know:
/// (kind, row). The 3-dial combination lock (the coordinator 06:56Z); the 8-furnace super smelter
/// (the coordinator 2026-10-22: players build 8 to 16 furnace arrays; its card leads with its
/// double chest's minutes).
const CARD_ROWS: [(&str, i64); 2] = [("lock", 3), ("smelter", 8)];

/// Per-size picks: (study/kind, size, design). None now: reach 2 is the smallest box like every
/// reach (pext-n2-rider-flat, 6x2x1; vjeux via pext 11:55Z).
const SIZE_PICKS: [(&str, i64, &str); 0] = [];

/// The kinds whose designs differ by a number a builder picks (the reach, the count, the length):
/// one card per kind, a table of the archived sizes on its page. (Flying machines are not: one
/// design per engine shape, vjeux 00:33Z.)
pub fn parametric(kind: &str) -> bool {
    matches!(
        base(kind),
        "piston-extender" | "counter" | "divider" | "burst" | "delay" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "monostable" | "off-delay" | "on-delay" | "pulse-train" | "shift" | "lock" | "randomizer" | "item-elevator" | "smelter" | "elevator" | "binary-counter" | "decoder" | "mux" | "comparator" | "piston-bridge" | "bedrock-breaker"
    )
}

fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or(kind)
}

/// The kind a page's kind line and Kind row name ([`crate::explain::kind_title`]): the published
/// kind ([`kind_key`]) where its title depends on more than the judge's kind (which adder, a
/// tileable smelter row, an automatic door's size, a cart stop's line), else the judge's kind.
pub fn title_key(b: &Build) -> String {
    match b.circuit.as_ref() {
        Some(c) if ["adder", "smelter", "auto-door", "cart-unloader", "cart-loader"].iter().any(|k| c.kind.starts_with(k)) => kind_key(b),
        Some(c) => c.kind.clone(),
        None => b.kind.clone(),
    }
}

/// A circuit's kind as a builder asks for it: a gate's function (a mirror pair is one kind), a
/// timer's restart rule, a two-way flying machine, a sand elevator; else the kind's first word.
pub fn kind_key(b: &Build) -> String {
    let Some(c) = &b.circuit else { return b.kind.clone() };
    let mut w = c.kind.split_whitespace();
    let k0 = w.next().unwrap_or("");
    match k0 {
        "gate" => format!("gate {}", w.next().unwrap_or("").trim_end_matches("-ba")),
        "monostable" => format!("monostable {}", w.next().unwrap_or("")),
        "flying-machine" if c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way") => "flying-machine two-way".into(),
        // flying machines by engine shape (vjeux 00:31Z, the coordinator): a tall engine (stacked:
        // the one that carries extra blocks) and a flat one (side by side, 1 block tall); the
        // other one-way machines (up, down, trains) are not published
        "flying-machine" if c.feature.starts_with("carries") => "flying-machine tall".into(),
        "flying-machine" if c.feature.is_empty() && (c.direction.is_empty() || c.direction == "horizontal") => "flying-machine flat".into(),
        "flying-machine" => format!("-flying-machine {}", if c.direction.is_empty() { "other" } else { c.direction.as_str() }),
        // the precise dupers are kinds of their own (vjeux 00:14Z: "We still need another
        // variation for precise drop location"): the air burst (every TNT explodes at one point in
        // mid-air) and the precise drop (every TNT falls straight down to one point)
        // (an on/off switch is a property, not a kind: the coordinator 14:08Z, "precise drop,
        // switchable" is the precise drop with a lever)
        "tnt-duper" if c.axis.contains("precise") => {
            let v = c.axis.split_once("TNT/min").map(|(_, v)| v.trim()).unwrap_or("precise");
            format!("tnt-duper {}", v.replace("switchable", "").trim().trim_end_matches(',').trim())
        }
        "piston-extender" if b.group == "Sand elevator" => "sand-elevator".into(),
        // the adder study (w86): half, full, slice (every design is `adder 1`)
        "adder" => format!("adder {}", site_model::html::adder_variant(c)),
        // a crop farm: one kind per crop (`harvester sugar_cane`)
        "harvester" => c.kind.clone(),
        // an auto-smelter row that tiles (slices side by side, as many furnaces as wanted) is a kind
        // of its own; the others, one table per number of furnaces ("We show the smallest build for
        // each number of furnaces")
        "smelter" if c.axis.starts_with("tileable") => "smelter tileable".into(),
        // a minecart station: one-way and two-way lines are two kinds (`station 1`, `station 2`)
        "station" => c.kind.clone(),
        // an auto-crafter: one kind per recipe (`crafter iron_block`), each its own record
        "crafter" => c.kind.clone(),
        // a hopper-minecart unloader / loader: one kind per line (terminus, through line)
        "cart-unloader" | "cart-loader" => format!("{k0} {}", if c.axis.contains("through") { "through" } else { "terminus" }),
        // an automatic door: one kind per door size (its axis, `2x2`)
        "auto-door" => format!("auto-door {}", c.axis.split_whitespace().next().unwrap_or("2x2")),
        // a button sequence lock: one kind per (buttons, presses) (clk-item's board)
        "sequence-lock" => c.kind.clone(),
        // an iron farm: one kind per tier (`mob-farm iron_ingot middle`; vjeux via the decisions
        // inbox 2026-11-08: one page per tier, as a crop farm has one kind per crop); a farm in no
        // tier (the brief's obvious design, the bar to beat) is no published kind: catalog only
        "mob-farm" if c.kind.split_whitespace().count() >= 3 => c.kind.clone(),
        "mob-farm" => format!("-{}", c.kind),
        // a piston bridge: one kind per walkway width (pext: the 2-wide stone bridges are their
        // own table; a 1-wide stone walkway is a property beside the slime one, `walkway_variants`)
        "piston-bridge" => match bridge_width(c) {
            w if w > 1 => format!("piston-bridge {w}wide"),
            _ => "piston-bridge".into(),
        },
        _ => k0.to_string(),
    }
}

/// The row a design takes in its kind's table: its size, or a flying machine's direction or load.
/// (sort key, words).
pub fn row_of(b: &Build) -> Option<(i64, String)> {
    let c = b.circuit.as_ref()?;
    if c.kind.starts_with("flying-machine") {
        let (k, w) = if c.feature.starts_with("carries") {
            (3, "carries extra blocks")
        } else if c.feature.starts_with("pulls") {
            (4, "pulls trailers")
        } else {
            match c.direction.as_str() {
                "up" => (1, "flies up"),
                "down" => (2, "flies down"),
                _ => (0, "flies sideways"),
            }
        };
        return Some((k, w.to_string()));
    }
    c.value.map(|v| (v, v.to_string()))
}

/// Robust: the same behaviour at 4 positions and 4 rotations in the game.
fn robust(b: &Build) -> bool {
    // (one that passed the same way everywhere, only a number different at one spot, counts as
    // robust: sorter's ld-chest, its load rate 17.05 at one spot against 16.87)
    b.circuit.as_ref().is_some_and(|c| c.robustness == "robust" || c.robustness.is_empty() || crate::explain::numbers_only_variation(&c.robustness).is_some())
}

/// [`robust`] for the dominance audit (`audit::dominated`: a robust design is never beaten by one
/// that is not; the ranking puts robust designs first).
pub fn is_robust(b: &Build) -> bool {
    robust(b)
}

/// [`rank_size`] for the dominance audit: what "smaller" means for the design's kind (the box
/// first for the kinds ranked by box: a piston extender's 6×2 frame is smaller than a 5×3 one
/// with a block fewer, and it is the pick of its reach).
pub fn size_key(b: &Build) -> (u32, u32) {
    rank_size(b)
}

/// The study's first speed metric (lower is better), for ties.
fn speed(b: &Build) -> f64 {
    let Some(c) = &b.circuit else { return f64::MAX };
    let num = |k: &str| c.metrics.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.split_whitespace().next()?.rsplit('-').next()?.parse::<f64>().ok());
    match base(&c.kind) {
        "shulker-loader" | "shulker-unloader" | "sorter" | "item-elevator" => -num("items/s").unwrap_or(0.0),
        "crafter" => -num("crafts/min").unwrap_or(0.0),
        "cart-unloader" => -num("unload rate").unwrap_or(0.0),
        "cart-loader" => -num("load rate").unwrap_or(0.0),
        "smelter" => -num("items/min per furnace").unwrap_or(0.0),
        "brewer" => -num("potions/min per stand").unwrap_or(0.0),
        "composter" => -num("items composted/min").unwrap_or(0.0),
        "tnt-duper" => -num("rate").unwrap_or(0.0),
        "piston-extender" => num("extend").unwrap_or(f64::MAX),
        "piston-bridge" => num("extend").unwrap_or(f64::MAX),
        "flying-machine" => num("speed").unwrap_or(f64::MAX),
        _ => num("latency").or_else(|| num("delay")).unwrap_or(f64::MAX),
    }
}

/// Why the one design of its kind (no other tested) is worth building, from its own facts (no
/// "the only one tested": critique, said of a duper picked over seven that had left the archive).
fn lone_reason(b: &Build) -> String {
    let Some(c) = &b.circuit else { return String::new() };
    let mut why: Vec<&str> = Vec::new();
    // (standing on the ground is in a TNT duper's title already: not repeated)
    // (a sorter: what builders tile it for; "tiles side by side" is its chip)
    if c.kind.starts_with("sorter") {
        why.push("the classic one-block-wide slice");
    }
    // (a whole minecart line is one end-to-end run, not 4 facings at 4 spots: no such claim)
    if robust(b) && !c.kind.starts_with("cart-line") {
        why.push("passed at every position and facing we tested");
    }
    why.join(", ")
}

/// The candidates of one kind, best first: robust ones first, then the smallest ([`rank_size`]),
/// then the fastest, then the name.
pub fn ranked<'a>(list: &[&'a Build]) -> Vec<&'a Build> {
    let mut v: Vec<&Build> = list.to_vec();
    // (a mirror pair's tie goes to the A-first version: "A AND NOT B", not "B AND NOT A")
    let mirror = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.kind.split_whitespace().nth(1).is_some_and(|f| f.ends_with("-ba")));
    v.sort_by(|a, b| (!robust(a), rank_size(a), speed(a), mirror(a)).partial_cmp(&(!robust(b), rank_size(b), speed(b), mirror(b))).unwrap_or(std::cmp::Ordering::Equal).then(a.name.cmp(&b.name)));
    v
}

/// What "smallest" means for a design's kind, as a sort key: fewest blocks, then the smallest box
/// (vjeux 13:11Z, "sort by blocks then volume"); the box first, then blocks, for the kinds vjeux
/// asked by volume ([`by_box`]), a TNT duper's box with the air it needs under it
/// (`html::effective_volume`: the coordinator 15:18Z).
fn rank_size(b: &Build) -> (u32, u32) {
    // an iron farm: its tier's rank (designs/briefs/farm-iron.md), then blocks
    if let Some(r) = mob_farm_rank(b) {
        return r;
    }
    // a crop farm: the most harvest an hour for each block placed first (vjeux 19:03Z: "farm
    // efficiency = items gathered per hour ÷ blocks placed"), then its box
    if let Some((per_block, _)) = b.circuit.as_ref().and_then(site_model::html::farm_rates) {
        return (u32::MAX - (per_block * 10_000.0).round().clamp(0.0, (u32::MAX - 1) as f64) as u32, b.volume.unwrap_or(u32::MAX));
    }
    // an auto-smelter: the quickest to smelt a double chest of raw iron first (sorter's `chest`
    // run: the player's number, the coordinator 2026-10-22), then blocks; plain furnaces before
    // blast furnaces (ores only: the blast version of a size is its pick's variant); one not
    // timed on a double chest after those timed, by blocks
    if let Some(c) = b.circuit.as_ref().filter(|c| base(&c.kind) == "smelter") {
        let blast = if site_model::html::smelter_blast(c) { 4_000_000 } else { 0 };
        return match site_model::html::double_chest_minutes(c) {
            Some(m) => (blast + (m * 10.0).round().min(1_999_999.0) as u32, b.blocks),
            None => (blast + 2_000_000 + b.blocks.min(1_999_999), b.volume.unwrap_or(u32::MAX)),
        };
    }
    if by_box(b) {
        (site_model::html::effective_volume(b).unwrap_or(u32::MAX), b.blocks)
    } else if let Some(r) = station_rate(b) {
        // a minecart loader or unloader: the most items a second first (the number the reviewer
        // passed, the coordinator 2026-10-22: ul-gate's 4.79 before cart-unloader-t3's 2.48), then
        // blocks
        (u32::MAX - (r * 1000.0).round().clamp(0.0, (u32::MAX - 1) as f64) as u32, b.blocks)
    } else {
        (b.blocks, b.volume.unwrap_or(u32::MAX))
    }
}

/// An iron farm's rank in its tier (designs/briefs/farm-iron.md revision 3), as a sort key: the
/// day-1 farm the least iron to build, the middle-ground farm the most iron an hour for each block
/// placed, the fastest the most iron an hour for each group of villagers; then the fewest blocks.
fn mob_farm_rank(b: &Build) -> Option<(u32, u32)> {
    let c = b.circuit.as_ref()?;
    if !c.kind.starts_with("mob-farm ") {
        return None;
    }
    let num = |k: &str| metric(b, k).split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
    let most = |v: Option<f64>| v.map(|v| u32::MAX - (v * 1000.0).round().clamp(0.0, (u32::MAX - 1) as f64) as u32).unwrap_or(u32::MAX);
    Some(match c.kind.split_whitespace().nth(2) {
        Some("day-1") => (num("bill").map(|v| v.round() as u32).unwrap_or(u32::MAX), b.blocks),
        Some("middle") => (most(num("iron per hour per block")), b.blocks),
        _ => (most(num("iron per hour per module")), b.blocks),
    })
}

/// An iron farm's fastest pick (one group of villagers): its tier's fastest build of several
/// (copies of the module side by side, the brief's "N x module"), a page linked from the pick's
/// and back (vjeux via the decisions inbox 2026-11-08: the scaled build shown on the fastest
/// tier's page): (variant, pick).
fn scaled_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let modules = |b: &Build| metric(b, "modules").parse::<u32>().unwrap_or(1);
    let rate = |b: &Build| metric(b, "iron per hour").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        if !kind.starts_with("mob-farm ") {
            continue;
        }
        for &pick in keep {
            if modules(&builds[pick]) != 1 {
                continue;
            }
            let scaled = all.get(&(study.clone(), kind.clone())).into_iter().flatten().copied().filter(|&i| modules(&builds[i]) > 1 && robust(&builds[i]));
            if let Some(v) = scaled.max_by(|&a, &b| rate(&builds[a]).total_cmp(&rate(&builds[b]))) {
                out.push((v, pick));
            }
        }
    }
    out
}

/// A minecart loader's or unloader's items a second (`load rate` / `unload rate`).
fn station_rate(b: &Build) -> Option<f64> {
    let c = b.circuit.as_ref()?;
    let k = if c.kind.starts_with("cart-unloader") {
        "unload rate"
    } else if c.kind.starts_with("cart-loader") {
        "load rate"
    } else {
        return None;
    };
    metric(b, k).split_whitespace().next()?.parse().ok()
}

/// How a pick's claim says "smallest" ([`rank_size`]): the word, and what it counts.
fn small_words(b: &Build) -> (&'static str, &'static str) {
    let air = b.circuit.as_ref().map(site_model::html::tnt_air).unwrap_or(0);
    match b.circuit.as_ref().map(|c| c.kind.as_str()) {
        Some(k) if k.starts_with("tnt-duper") && air > 0 => ("smallest", " (counting the air it needs under it)"),
        // a crop farm ranks by its harvest an hour for each block placed (rank_size)
        Some(k) if k.starts_with("harvester") => ("the biggest harvest per block,", ""),
        // an auto-smelter timed on a double chest ranks by its minutes (rank_size); plain
        // furnaces smelt anything, blast furnaces only ores (their version is a variant)
        Some(k) if k.starts_with("smelter") && b.circuit.as_ref().and_then(site_model::html::double_chest_minutes).is_some() => {
            if b.circuit.as_ref().is_some_and(site_model::html::smelter_blast) { ("the quickest with blast furnaces,", "") } else { ("the quickest that smelts anything,", "") }
        }
        // a minecart stop ranks by the items it moves a second (rank_size)
        Some(k) if k.starts_with("cart-unloader") || k.starts_with("cart-loader") => ("the fastest", ""),
        // an iron farm ranks by its tier's rule (rank_size)
        Some(k) if k.starts_with("mob-farm ") => match k.split_whitespace().nth(2) {
            Some("day-1") => ("the cheapest to build,", ""),
            Some("middle") => ("the most iron for each block placed,", ""),
            _ => ("the most iron for each group of villagers,", ""),
        },
        _ if by_box(b) => ("smallest box", ""),
        _ => ("smallest", ""),
    }
}

/// A kind whose pick is its smallest box (vjeux's minimum-volume rule, for the kinds he asked it
/// for: piston extenders, via pext 11:55Z, "the smallest VOLUME per reach in any 3D shape", the
/// box being the game judge's, every cell over the whole run; TNT dupers and shulker loaders and
/// unloaders, via the coordinator 13:55Z); the sand elevator, whose sand falls through air it
/// does not own, and every other kind: blocks first.
fn by_box(b: &Build) -> bool {
    let Some(c) = &b.circuit else { return false };
    match base(&c.kind) {
        "piston-extender" => b.group != "Sand elevator",
        // (pext's piston bridges: the smallest box per gap, as for the extenders)
        "piston-bridge" => true,
        // (the randomizers: the smallest box per N, the coordinator via clk-item 2026-10-21; the
        // auto-crafters, smallest volume first: clk-item; the button sequence locks, the smallest
        // box per buttons and presses: clk-item)
        "tnt-duper" | "tnt-duper-triggered" | "shulker-loader" | "shulker-unloader" | "randomizer" | "crafter" | "sequence-lock" => true,
        _ => false,
    }
}


/// A gate with its own levers (the basics): not a gate a builder wires in (the coordinator 21:55Z:
/// the lever basics are dropped).
fn basic(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("gate ")) && b.flags.iter().any(|f| f == "lever")
}

/// The kinds of a set of circuit builds: (study, kind) -> the candidates (no basics, no other
/// layouts).
fn kinds_of(builds: &[Build]) -> BTreeMap<(String, String), Vec<usize>> {
    let mut by: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.circuit else { continue };
        if c.layout_of.is_some() || basic(b) {
            continue;
        }
        // (a kind key starting with "-" is no published kind: its designs stay unpublished)
        let key = kind_key(b);
        if key.starts_with('-') {
            continue;
        }
        by.entry((c.study.clone(), key)).or_default().push(i);
    }
    by
}

/// What each kind keeps: a non-parametric kind its one design, a parametric kind the record of
/// each row of its table. (study, kind) -> the kept builds' indices, the table's order.
fn chosen(builds: &[Build]) -> BTreeMap<(String, String), Vec<usize>> {
    let mut out = BTreeMap::new();
    for ((study, kind), idx) in kinds_of(builds) {
        let list: Vec<&Build> = idx.iter().map(|&i| &builds[i]).collect();
        let at = |name: &str| idx.iter().copied().find(|&i| builds[i].name == name);
        let path = format!("{study}/{kind}");
        let keep: Vec<usize> = if parametric(&kind) {
            let mut rows: BTreeMap<i64, usize> = BTreeMap::new();
            for b in ranked(&list) {
                let i = idx.iter().copied().find(|&i| builds[i].id == b.id).unwrap();
                if let Some((k, _)) = row_of(b) {
                    rows.entry(k).or_insert(i);
                }
            }
            for (p, size, name) in SIZE_PICKS {
                if p == path
                    && let Some(i) = at(name)
                {
                    rows.insert(size, i);
                }
            }
            rows.into_values().collect()
        } else {
            // a rule narrows the candidates to what builders ask for (none qualifying: all of them)
            let pool: Vec<&Build> = match rule_for(&path) {
                Some((_, _, test)) if list.iter().any(|b| test(b)) => list.iter().copied().filter(|b| test(b)).collect(),
                _ => list.clone(),
            };
            ranked(&pool).first().and_then(|b| idx.iter().copied().find(|&i| builds[i].id == b.id)).into_iter().collect()
        };
        out.insert((study, kind), keep);
    }
    out
}

/// Keeps one design per kind (a parametric kind: the record of each row, one card for them all,
/// a table on its pages; its extendable families' base designs too, as generators with a page and
/// no card) and drops every other circuit build; returns the notes.
pub fn select_circuits(builds: &mut Vec<Build>) -> Vec<String> {
    let chosen = chosen(builds);
    let all = kinds_of(builds);
    // a pick the archive does not hold falls back to the rule, loudly (nothing silently dropped)
    let mut missing: Vec<String> = Vec::new();
    for (path, name) in SIZE_PICKS.iter().map(|(p, _, n)| (*p, *n)) {
        let (study, kind) = path.split_once('/').unwrap_or((path, ""));
        let have = all.get(&(study.to_string(), kind.to_string())).is_some_and(|v| v.iter().any(|&i| builds[i].name == name));
        if !have {
            let used = chosen.get(&(study.to_string(), kind.to_string())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("pick {path}: {name} is not in the archive (yet); showing {used} by the rule"));
        }
    }
    // a rule nothing meets: the ranking's pick, loudly
    for (path, words, test) in RULES {
        let (study, kind) = path.split_once('/').unwrap_or((path, ""));
        if let Some(v) = all.get(&(study.to_string(), kind.to_string()))
            && !v.iter().any(|&i| test(&builds[i]))
        {
            let used = chosen.get(&(study.to_string(), kind.to_string())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("rule {path}: no design {words}; showing {used}, the best of the others"));
        }
    }
    // a crop with no full-size farm yet: its best module, noted (the full-size farms come crop by
    // crop: w86)
    for ((study, kind), v) in &all {
        if kind.starts_with("harvester ") && !v.iter().any(|&i| full_size(&builds[i])) {
            let used = chosen.get(&(study.clone(), kind.clone())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("rule {study}/{kind}: no full-size farm (64 plants or more) yet; showing {used}, the best module"));
        }
    }
    // why each kept design is the one (critique: "cards don't say why they're the best"): the
    // coordinator's reason for a pick, else smallest (or best) of the designs tested for its kind,
    // or for its size
    let mut claims: BTreeMap<usize, String> = BTreeMap::new();
    for ((study, kind), keep) in &chosen {
        let cands = all.get(&(study.clone(), kind.clone())).cloned().unwrap_or_default();
        let tested = |n: usize| format!("of {n} tested");
        for &i in keep {
            let row = row_of(&builds[i]).map(|r| r.0);
            let pool: Vec<usize> = if parametric(kind) { cands.iter().copied().filter(|&j| row_of(&builds[j]).map(|r| r.0) == row).collect() } else { cands.clone() };
            // (the kinds ranked by their box first, [`by_box`]: "smallest" is their box's)
            // (what "smallest" counts for its kind: [`rank_size`], [`small_words`])
            let size = |j: usize| rank_size(&builds[j]);
            let smallest = pool.iter().all(|&j| size(j) >= size(i));
            let (small_word, counts) = small_words(&builds[i]);
            let path = format!("{study}/{kind}");
            let rule = rule_for(&path).filter(|(_, _, test)| test(&builds[i]));
            // ("the smallest", "the biggest harvest per block,": one "the")
            let the_small = if small_word.starts_with("the ") { small_word.to_string() } else { format!("the {small_word}") };
            let reason = match (path.as_str(), builds[i].name.as_str()) {
                // (the best of those that do what builders ask, said so: "the smallest box of 13
                // tested that tiles side by side"; smallest of all, it is said plainly below)
                _ if rule.is_some() && !smallest => format!("{the_small} {}{counts} that {}", tested(pool.len()), rule.map(|r| r.1).unwrap_or_default()),
                // (one design left in the archive says nothing of the others tested before it:
                // critique, "the only one tested" on a duper picked over seven; a pick says why)
                _ if pool.len() == 1 => lone_reason(&builds[i]),
                _ if parametric(kind) && smallest => format!("{small_word} for its size {}{counts}", tested(pool.len())),
                _ if parametric(kind) => format!("best for its size {}", tested(pool.len())),
                _ if smallest => format!("{small_word} {}{counts}", tested(pool.len())),
                _ => format!("best {}", tested(pool.len())),
            };
            // the kind's one-line mention beside its pick (the BUD: the latched classic, the
            // observer)
            let reason = match mention(study, kind) {
                Some(m) if !reason.is_empty() => format!("{reason}; {m}"),
                Some(m) => m.to_string(),
                None => reason,
            };
            claims.insert(i, reason);
        }
    }
    let mut kept: BTreeSet<usize> = chosen.values().flatten().copied().collect();
    // (no generators: the extendable families are gone, vjeux 00:33Z)
    let generators: BTreeSet<usize> = BTreeSet::new();
    kept.extend(generators.iter().copied());
    // a pick that stands higher keeps the kind's ground design beside it, a pick with an on/off
    // switch the kind's best design without one: a page, no card, linked from the pick's (vjeux
    // 11:49Z; the coordinator 14:08Z)
    let grounds = ground_variants(builds, &chosen);
    kept.extend(grounds.iter().map(|(v, _)| *v));
    let switchless = deviceless_variants(builds, &chosen);
    kept.extend(switchless.iter().map(|(v, ..)| *v));
    // a piston extender's size whose pick breaks when flipped back early (or is not known safe):
    // that size's best flip-safe design, a page linked from the pick's (the coordinator)
    let flip_safe = flip_safe_variants(builds, &chosen);
    kept.extend(flip_safe.iter().map(|(v, _)| *v));
    // a piston bridge's gap whose pick has one walkway: that gap's best design with the other,
    // a page linked from the pick's and back (pext)
    let walkways = walkway_variants(builds, &chosen);
    kept.extend(walkways.iter().map(|(v, _)| *v));
    // an auto-smelter's size whose pick has plain furnaces: that size's best blast-furnace
    // design, a page linked from the pick's and back (the coordinator 2026-10-22)
    let blasts = blast_variants(builds, &chosen);
    kept.extend(blasts.iter().map(|(v, _)| *v));
    // a crop whose pick is a full-size farm: its best small module, a page linked from the pick's
    // and back (the coordinator via w86 2026-10-22)
    let modules = module_variants(builds, &chosen);
    kept.extend(modules.iter().map(|(v, _)| *v));
    // a crop whose pick sweeps its field with a flying machine: its best full-size farm without
    // one, linked both ways (the coordinator 2026-10-22)
    let still_farms = machine_variants(builds, &chosen);
    kept.extend(still_farms.iter().map(|(v, _)| *v));
    // a full-size farm's pick: its crop's farm of the same kind on about half the field, linked
    // both ways (the reviewer via the coordinator 2026-10-22)
    let half_fields = half_field_variants(builds, &chosen);
    kept.extend(half_fields.iter().map(|(v, _)| *v));
    // an automatic door whose pick closes on someone standing in the doorway: its kind's best
    // door that stays open for them, linked both ways (the coordinator 2026-10-22)
    let doorways = doorway_variants(builds, &chosen);
    kept.extend(doorways.iter().map(|(v, _)| *v));
    // a minecart line's stations: a page each, linked from the line's (sorter 2026-10-22)
    let stations = line_stations(builds, &chosen);
    kept.extend(stations.iter().map(|(s, _, _)| *s));
    // a minecart stop whose pick is the fastest: its smallest design beside it (the coordinator
    // 2026-10-22)
    let small_stations = smaller_station_variants(builds, &chosen);
    kept.extend(small_stations.iter().map(|(v, _)| *v));
    // an iron farm's one-module pick: its tier's scaled build, linked both ways (vjeux via the
    // decisions inbox 2026-11-08)
    let scaled = scaled_variants(builds, &chosen);
    kept.extend(scaled.iter().map(|(v, _)| *v));
    let before = builds.iter().filter(|b| b.circuit.is_some()).count();
    // the kinds' groups: the records of a parametric kind form one group, its smallest design the
    // card (flying machines: the sideways one first), one entry per group
    let mut group_of: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for ((study, kind), keep) in &chosen {
        if parametric(kind) && keep.len() > 1 {
            group_of.insert(format!("kind:{study}:{kind}"), keep.clone());
        }
    }
    let mut notes = vec![format!("one design per kind: {} kinds, {} designs kept of {before} circuit designs (the rest stay in the repository, unpublished)", chosen.len(), kept.len())];
    notes.extend(missing);
    for ((study, kind), keep) in &chosen {
        notes.push(format!("kind {study}/{kind}: {}", keep.iter().map(|&i| builds[i].name.clone()).collect::<Vec<_>>().join(", ")));
    }
    // the folds of the old lists no longer apply: every kept design is its kind's
    let groups: Vec<(String, Vec<usize>)> = group_of.into_iter().collect();
    for i in kept.iter().copied() {
        let b = &mut builds[i];
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "layout" | "faster" | "mirrored" | "tunable" | "kind-twin" | "census" | "twin" | "size" | "generator"));
        if generators.contains(&i) {
            b.flags.push("size".into());
            b.flags.push("generator".into());
        }
        if let Some(c) = b.circuit.as_mut() {
            c.host = None;
            c.layout_of = None;
            c.lead.clear();
            c.claim = claims.get(&i).cloned().unwrap_or_default();
            c.tunable.clear();
            c.span.clear();
            c.span_blocks.clear();
            c.card_member = false;
            c.ground = None;
            c.no_switch = None;
            c.flip_safe = None;
            c.alt_walkway = None;
            c.blast_twin = None;
            c.farm_twin = None;
            c.machine_twin = None;
            c.doorway_twin = None;
            c.half_twin = None;
            c.part_of = None;
            c.station_twin = None;
        }
    }
    let ids: Vec<String> = builds.iter().map(|b| b.id.clone()).collect();
    for &(v, pick) in &grounds {
        let up = stand_height(&builds[pick]);
        builds[v].flags.push("size".into());
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = format!("sits right on the ground (the pick of its kind stands {up} blocks up)");
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.ground = Some(ids[v].clone());
        }
    }
    for &(v, pick, device) in &switchless {
        let fewer = builds[pick].blocks.saturating_sub(builds[v].blocks);
        let ground = grounds.iter().any(|&(g, _)| g == v);
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = if ground {
                // (one build both: the 22-block clocked duper beside the lever engine)
                c.claim.replacen("sits right on the ground", &format!("sits right on the ground, with no {device}"), 1)
            } else if fewer > 0 {
                format!("no {device}, {fewer} block{} smaller than the pick of its kind", if fewer == 1 { "" } else { "s" })
            } else {
                format!("no {device}")
            };
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.no_switch = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &flip_safe {
        let more = builds[v].blocks.saturating_sub(builds[pick].blocks);
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = if more > 0 { format!("safe to flip back any time, {more} block{} bigger than the pick of its size", if more == 1 { "" } else { "s" }) } else { "safe to flip back any time".to_string() };
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.flip_safe = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &walkways {
        let more = builds[v].blocks as i64 - builds[pick].blocks as i64;
        let word = builds[v].circuit.as_ref().map(site_model::html::walkway_word).unwrap_or_default();
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            let n = more.unsigned_abs();
            c.claim = match more {
                0 => format!("a {word} walkway"),
                m if m > 0 => format!("a {word} walkway, {n} block{} more than the pick of its gap", if n == 1 { "" } else { "s" }),
                _ => format!("a {word} walkway, {n} block{} fewer than the pick of its gap", if n == 1 { "" } else { "s" }),
            };
            c.alt_walkway = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.alt_walkway = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &blasts {
        let more = builds[v].blocks as i64 - builds[pick].blocks as i64;
        let n = more.unsigned_abs();
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            let blocks = match more {
                0 => "as many blocks as the pick of its size".to_string(),
                m if m > 0 => format!("{n} block{} more than the pick of its size", if n == 1 { "" } else { "s" }),
                _ => format!("{n} block{} fewer than the pick of its size", if n == 1 { "" } else { "s" }),
            };
            c.claim = format!("blast furnaces: twice as fast, ores and raw metals only; {blocks}");
            c.blast_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.blast_twin = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &modules {
        let full = builds[pick].circuit.as_ref().map(|c| c.crops).unwrap_or(0);
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            let plant = site_model::html::plant_word(c);
            let n = c.crops.max(1);
            c.claim = format!("the best small farm, {n} {plant}{} (the pick of its crop is a full-size farm of {full} {plant}s)", if n == 1 { "" } else { "s" });
            c.farm_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.farm_twin = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &scaled {
        let modules = metric(&builds[v], "modules").to_string();
        let rate = metric(&builds[v], "iron per hour").split_whitespace().next().unwrap_or("").to_string();
        let one = metric(&builds[pick], "iron per hour").split_whitespace().next().unwrap_or("").to_string();
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = format!("the scaled build: {modules} copies of the pick of its tier, {rate} iron an hour against one copy's {one}");
            c.farm_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.farm_twin = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &half_fields {
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        let (n, of) = (builds[v].circuit.as_ref().map(|c| c.crops).unwrap_or(0), builds[pick].circuit.as_ref().map(|c| c.crops).unwrap_or(0));
        let plant = builds[v].circuit.as_ref().map(site_model::html::plant_word).unwrap_or("plant");
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = format!("half the field of the pick of its crop ({n} {plant}s against {of})");
            c.half_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.half_twin = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &still_farms {
        let bar = builds[v].caption.starts_with("The obvious ");
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            let no_slime = site_model::html::bill_slime(c).is_none_or(|n| n < 9);
            let without = format!("no flying machine{}", if no_slime { ", no slime blocks" } else { "" });
            // (the obvious design, the bar the pick is measured against: its caption says so)
            c.claim = if bar { format!("the obvious design, the bar the pick of its crop is measured against: {without}") } else { format!("{without} (the pick of its crop sweeps its field with one)") };
            c.machine_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.machine_twin = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &doorways {
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = "stays open for someone standing in the doorway (the pick of its kind closes on them)".to_string();
            c.doorway_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.doorway_twin = Some(ids[v].clone());
        }
    }
    for &(s, line, pick) in &stations {
        if let Some(c) = builds[s].circuit.as_mut() {
            c.part_of = Some(ids[line].clone());
        }
        if let Some(p) = pick {
            if !builds[s].flags.iter().any(|f| f == "size") {
                builds[s].flags.push("size".into());
            }
            let claim = station_claim(&builds[s], &builds[p]);
            if let Some(c) = builds[s].circuit.as_mut() {
                c.claim = claim;
                c.station_twin = Some(ids[p].clone());
            }
            if let Some(c) = builds[p].circuit.as_mut() {
                c.station_twin = Some(ids[s].clone());
            }
        }
    }
    for &(v, pick) in &small_stations {
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        let claim = smaller_station_claim(&builds[v], &builds[pick]);
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = claim;
            c.station_twin = Some(ids[pick].clone());
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.station_twin = Some(ids[v].clone());
        }
    }
    // a parametric kind's card: its smallest design, unless that build already pictures another
    // kind's card (critique: the toggle switch and the counter showed one build, so did the
    // rising edge and the signal shortener): then its next smallest
    let id_of = |b: &Build| b.circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, b.name)).unwrap_or_default();
    let mut pictured: BTreeSet<String> = BTreeSet::new();
    for ((_, kind), keep) in &chosen {
        if !(parametric(kind) && keep.len() > 1) {
            for &i in keep {
                pictured.insert(id_of(&builds[i]));
                pictured.extend(builds[i].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default());
            }
        }
    }
    for (g, members) in &groups {
        let mut order: Vec<usize> = members.clone();
        // (the size builders know first, where it is not the smallest: CARD_ROWS)
        let card_row = |j: usize| -> bool {
            let k = builds[j].circuit.as_ref().map(|c| base(&c.kind).to_string()).unwrap_or_default();
            CARD_ROWS.iter().any(|(kind, row)| *kind == k && row_of(&builds[j]).map(|r| r.0) == Some(*row))
        };
        order.sort_by_key(|&j| (!card_row(j), builds[j].blocks, row_of(&builds[j]).map(|r| r.0).unwrap_or(i64::MAX), builds[j].name.clone()));
        let twin = |j: usize| pictured.contains(&id_of(&builds[j])) || builds[j].circuit.as_ref().is_some_and(|c| c.same_build.iter().any(|s| pictured.contains(s)));
        let rep = order.iter().copied().find(|&j| !twin(j)).unwrap_or(order[0]);
        pictured.insert(id_of(&builds[rep]));
        pictured.extend(builds[rep].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default());
        // the card speaks for the kind: the smallest design for each size (critique: "the best
        // of 42 designs tested" did not say best at what)
        let tested = g.strip_prefix("kind:").and_then(|k| k.split_once(':')).and_then(|(s, k)| all.get(&(s.to_string(), k.to_string()))).map(|v| v.len()).unwrap_or(0);
        let size_word = match builds[rep].circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default().as_str() {
            "piston-extender" => "reach",
            "piston-bridge" => "gap",
            "counter" => "count",
            "divider" => "factor",
            "burst" => "number of pulses",
            "delay" => "delay",
            "on-delay" => "hold time",
            "pulse-train" => "beat",
            "flying-machine" => "direction and load",
            "shift" => "length",
            "randomizer" => "number of outputs",
            "item-elevator" => "height",
            "elevator" => "height",
            "binary-counter" | "comparator" => "number of bits",
            "decoder" | "mux" => "number of select bits",
            "smelter" => "number of furnaces",
            "lock" => "number of dials",
            "bedrock-breaker" => "number of bedrock side by side",
            _ => "length",
        };
        // (the smallest box for the kinds ranked by it: [`by_box`]; a smelter timed on a double
        // chest, the quickest: [`small_words`])
        let small = match small_words(&builds[rep]).0 {
            w if w.starts_with("the quickest") => w,
            _ if by_box(&builds[rep]) => "the smallest box",
            _ => "the smallest",
        };
        if let Some(c) = builds[rep].circuit.as_mut() {
            c.claim = format!("{small} for each {size_word}, of {tested} designs tested");
        }
        let values: BTreeSet<i64> = members.iter().filter_map(|&j| builds[j].circuit.as_ref().and_then(|c| c.value)).collect();
        let fly = builds[rep].circuit.as_ref().is_some_and(|c| c.kind.starts_with("flying-machine"));
        let axis = builds[rep].circuit.as_ref().map(|c| c.axis.clone()).unwrap_or_default();
        let (letter, unit) = match axis.split_once(" = ") {
            Some((p, v)) => (p.split_whitespace().last().unwrap_or("").to_string(), if v.split_whitespace().nth(1) == Some("gt") { " gt" } else { "" }),
            // (an axis in words: an item elevator's "5 blocks up", a randomizer's "1 of 4")
            None if axis.ends_with(" blocks up") => (String::new(), " blocks up"),
            // (a player elevator's "height 5")
            None if axis.starts_with("height ") && builds[rep].circuit.as_ref().is_some_and(|c| c.kind.starts_with("elevator")) => (String::new(), " blocks up"),
            None if builds[rep].circuit.as_ref().is_some_and(|c| c.kind.starts_with("randomizer")) => (String::new(), " outputs"),
            None if axis.ends_with("furnace") || axis.ends_with("furnaces") => (String::new(), " furnaces"),
            None => (String::new(), ""),
        };
        let span = match (values.first(), values.last()) {
            (Some(lo), Some(hi)) if values.len() > 1 && !fly => format!("{letter} {lo}-{hi}{unit}, {} values", values.len()).trim().to_string(),
            _ => format!("{} versions", members.len()),
        };
        let lo_b = members.iter().map(|&j| builds[j].blocks).min().unwrap_or(0);
        let hi_b = members.iter().map(|&j| builds[j].blocks).max().unwrap_or(0);
        for &j in members {
            let label = row_of(&builds[j]).map(|r| r.1).unwrap_or_default();
            let b = &mut builds[j];
            if let Some(c) = b.circuit.as_mut() {
                c.tunable = g.clone();
                c.group_title = crate::explain::kind_title(&c.kind);
                c.group_note.clear();
                c.group_differs = if fly { "the direction and the load".into() } else { "the size".into() };
                c.variant_label = label;
                if j == rep {
                    c.span = span.clone();
                    c.span_blocks = if hi_b > lo_b { format!("{lo_b} to {hi_b} blocks") } else { String::new() };
                }
            }
            if j == rep {
                b.flags.push("tunable".into());
            }
        }
    }
    // what a kept design says about the same build archived as another kind: only kinds kept
    let ids: BTreeMap<String, String> = kept.iter().map(|&i| (builds[i].circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, builds[i].name)).unwrap_or_default(), builds[i].group.clone())).collect();
    for i in kept.iter().copied() {
        let also: Vec<String> = builds[i].circuit.as_ref().map(|c| c.same_build.iter().filter_map(|s| ids.get(s).cloned()).filter(|g| *g != builds[i].group).collect()).unwrap_or_default();
        if let Some(c) = builds[i].circuit.as_mut() {
            c.also = also;
        }
    }
    let mut k = 0;
    builds.retain(|b| {
        let keep = b.circuit.is_none() || kept.contains(&k);
        k += 1;
        keep
    });
    notes
}

/// Doors (the coordinator 21:55Z): one card, the overall smallest solid door (the "solid" board's
/// leader); every board's record keeps its page (no card); every other door is not published.
/// One card per door size (the coordinator 2026-10-21: the 2x2 doors' own boards, not mixed with
/// the 3x3 records): each size's card holds that size's records.
pub fn select_doors(builds: &mut Vec<Build>) -> Vec<String> {
    let leads = |b: &Build| b.door.as_ref().map(|d| d.leads.clone()).unwrap_or_default();
    let before = builds.iter().filter(|b| b.door.is_some()).count();
    builds.retain(|b| b.door.is_none() || !leads(b).is_empty());
    // a record of "X, not rotation-safe" that the record of X (which works facing any way) beats
    // or ties on size is the best at nothing (critique: the one-way sculk door, 40 blocks, under
    // the any-way one, 37)
    let size = |b: &Build| (b.volume.unwrap_or(u32::MAX), b.blocks);
    let side = site_model::door_side;
    let dominated: BTreeSet<String> = builds
        .iter()
        .filter(|b| {
            let l = leads(b);
            !l.is_empty()
                && l.iter().all(|x| {
                    x.strip_suffix(", not rotation-safe").is_some_and(|base| builds.iter().any(|o| o.id != b.id && side(o) == side(b) && leads(o).iter().any(|y| y == base) && size(o) <= size(b)))
                })
        })
        .map(|b| b.id.clone())
        .collect();
    builds.retain(|b| !dominated.contains(&b.id));
    let sides: BTreeSet<u32> = builds.iter().filter(|b| b.door.is_some()).map(side).collect();
    let mut mains: BTreeMap<u32, usize> = BTreeMap::new();
    let mut records: BTreeMap<u32, usize> = BTreeMap::new();
    for s in &sides {
        let of_side = |b: &Build| b.door.is_some() && side(b) == *s;
        // the one card: the best fully seamless door, the seamless board's record (vjeux 23:44Z:
        // "Can you show a seamless piston door. This is what people usually consider to be piston
        // doors"); without one, the smallest stone door
        let main = builds
            .iter()
            .position(|b| of_side(b) && leads(b).iter().any(|l| l == "seamless"))
            .or_else(|| builds.iter().position(|b| of_side(b) && leads(b).iter().any(|l| l == "solid")))
            .or_else(|| builds.iter().position(of_side));
        if let Some(m) = main {
            mains.insert(*s, m);
        }
        // what the one card holds: how many records sit behind it
        records.insert(*s, builds.iter().filter(|b| of_side(b)).count());
    }
    let mut kept = 0;
    for (i, b) in builds.iter_mut().enumerate() {
        if b.door.is_none() {
            continue;
        }
        let s = side(b);
        let main = mains.get(&s) == Some(&i);
        let d = b.door.as_mut().unwrap();
        d.host = None;
        d.variants.clear();
        d.records = if main { records.get(&s).copied().unwrap_or(0) } else { 0 };
        kept += 1;
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "twin" | "size"));
        if !main {
            b.flags.push("size".into());
        }
    }
    let per_side: Vec<String> = records.iter().rev().map(|(s, n)| format!("{s}x{s}: {n}")).collect();
    vec![format!("door cards: {} ({}): {kept} board records kept of {before} doors ({} best at nothing dropped; the rest stay in the repository, unpublished)", mains.len(), per_side.join(", "), dominated.len())]
}

/// Clocks (vjeux 21:47Z: "one card per period is the per-size table of a single clock kind
/// page"): per period the best clock (robust first, then fewest blocks) keeps its page, in the
/// table of the one clock card; the extendable families of the kept clocks stay; every other
/// clock is not published. The card: the smallest of them.
pub fn select_clocks(builds: &mut Vec<Build>, cards: &mut Vec<site_model::Card>, members: &mut Vec<site_model::Member>) -> Vec<String> {
    // every clock is a card under its period (vjeux 01:24Z: "show all of them with a card grouped
    // by timing"); a folded twin (the same mechanism with the same numbers: generate's
    // fold_twins) is not
    let before = builds.iter().filter(|b| b.clock.is_some()).count();
    builds.retain(|b| b.clock.is_none() || !b.flags.iter().any(|f| f == "twin"));
    let _ = cards;
    let mut periods: BTreeSet<u32> = builds.iter().filter_map(|b| b.clock.as_ref().map(|c| c.every())).collect();
    periods.extend(members.iter().filter(|m| m.axis.is_empty()).map(|m| m.period));
    let mut kept = 0;
    for b in builds.iter_mut() {
        let Some(c) = b.clock.as_mut() else { continue };
        kept += 1;
        c.host = None;
        // (an archived family member is an extended clock too: its card says so)
        let extended = c.instance && c.n.is_some();
        c.instance = false;
        // every clock is a card of its own (vjeux 01:24Z: "show all of them with a card grouped
        // by timing"): no clock stands for the others
        c.kind_range.clear();
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "twin" | "census" | "size" | "generator"));
        if extended && !b.flags.iter().any(|f| f == "extended") {
            b.flags.push("extended".into());
        }
    }
    vec![format!("clocks: {kept} of {before} (folded twins aside) and {} verified family members, every one a card under its period ({} periods)", members.iter().filter(|m| m.axis.is_empty()).count(), periods.len())]
}

/// A verified clock family member as a clock of its own (vjeux 01:24Z: "members get real pages
/// again, just not family pages"): its base's facts, its own period, blocks, box and design.
pub fn member_build(m: &site_model::Member, base: &Build) -> Build {
    let name = m.design.rsplit('/').next().unwrap_or("").trim_end_matches(".txt").to_string();
    let mut b = base.clone();
    b.id = format!("clocks/{name}");
    b.name = name.clone();
    b.page = format!("clocks/{name}.html");
    b.design = m.design.clone();
    b.blocks = m.blocks;
    b.dims = m.dims.clone();
    b.volume = m.volume;
    b.picture = m.picture.clone();
    b.trace = None;
    b.flags.retain(|f| !matches!(f.as_str(), "extendable" | "census" | "size" | "variant" | "twin"));
    b.flags.push("extended".into());
    // (the base's own words say how it works; the page says what the extension adds, from the
    // family's rule: critique, "A longer version of clk-item-dropring-1d (n = 15)" was all it said)
    b.caption = base.caption.clone();
    if m.status != site_model::STATUS_JUDGED {
        b.flags.push("simulated".into());
    }
    if let Some(c) = b.clock.as_mut() {
        c.period = m.period;
        // its own numbers, from its own simulation; none when it was not simulated (no claim:
        // never its base's, critique: an extended hopper clock of 148 ticks said "on for 18
        // ticks", its base's 18 of 20)
        match &m.run {
            Some(r) => {
                c.warmup = r.warmup;
                c.on = r.on;
                c.edges = r.edges;
                c.pulses = r.pulses.clone();
                c.pulse_every = r.pulse_every;
            }
            None => {
                c.on = 0;
                c.edges = 0;
                c.pulses.clear();
                c.pulse_every = None;
            }
        }
        // (what the base's chunk reload did is not this clock's: not checked)
        c.reload.clear();
        c.kind_range.clear();
        c.host = None;
        c.variants.clear();
        c.instance = false;
        c.family = Some(m.family.clone());
        c.n = Some(m.n);
        c.aliases.clear();
    }
    b
}

/// Every published circuit design's file in the repository, one per line: each kind's pick, and
/// every row of a parametric kind's table (`site kinds --paths`: the list a sweep over the
/// published designs takes, e.g. the exact game traces of every pick).
pub fn paths(builds: &[Build], repo: &std::path::Path) -> String {
    let chosen = chosen(builds);
    let mut s = String::new();
    for ((study, kind), keep) in &chosen {
        let take: Vec<usize> = if parametric(kind) { keep.clone() } else { keep.first().copied().into_iter().collect() };
        for i in take {
            let b = &builds[i];
            let archived = repo.join(format!("designs/circuits/{study}/archive/{}.txt", b.name));
            if archived.exists() {
                s.push_str(&format!("{}\n", archived.strip_prefix(repo).unwrap_or(&archived).display()));
            } else {
                s.push_str(&format!("{}\n", b.design));
            }
        }
    }
    s
}

/// The plan, one line per kind: the chosen design (a parametric kind: its rows) and the other
/// candidates.
pub fn report(builds: &[Build]) -> String {
    let chosen = chosen(builds);
    let all = kinds_of(builds);
    // (what the selection keeps beside the picks: variants, a line's stations; the report named
    // them "unpublished", cart-unloader-t3 beside ul-gate among them)
    let mut sel = builds.to_vec();
    let _ = select_circuits(&mut sel);
    let shown: BTreeSet<String> = sel.iter().map(|b| b.id.clone()).collect();
    let mut s = String::new();
    for ((study, kind), keep) in &chosen {
        let fact = |b: &Build| -> String { format!("{} {}b{}", b.name, b.blocks, if robust(b) { "" } else { " NOT-ROBUST" }) };
        let rows: Vec<String> = keep.iter().map(|&i| format!("{}={}", row_of(&builds[i]).map(|r| r.1).unwrap_or_default(), fact(&builds[i]))).collect();
        // the others, named (which designs a rule passed over, which it keeps beside the pick)
        let (beside, others): (Vec<usize>, Vec<usize>) = all.get(&(study.clone(), kind.clone())).into_iter().flatten().copied().filter(|i| !keep.contains(i)).partition(|&i| shown.contains(&builds[i].id));
        let names = |v: &[usize]| v.iter().map(|&i| builds[i].name.clone()).collect::<Vec<_>>().join(" ");
        let mut dropped = if others.is_empty() { "0 other designs unpublished".to_string() } else { format!("{} other designs unpublished: {}", others.len(), names(&others)) };
        if !beside.is_empty() {
            dropped = format!("{} kept beside it: {}; {dropped}", beside.len(), names(&beside));
        }
        if parametric(kind) {
            s.push_str(&format!("{study}\t{kind}\tTABLE {}\t({dropped})\n", rows.join(", ")));
        } else {
            s.push_str(&format!("{study}\t{kind}\tPICK {}\t({dropped})\n", keep.first().map(|&i| fact(&builds[i])).unwrap_or_default()));
        }
    }
    s
}
