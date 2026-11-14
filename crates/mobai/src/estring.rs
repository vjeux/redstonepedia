//! (tickexact 2026-11-13, ironspec 20:41: (E)'s empty press) `mctas mobai e-string --record <rev 3 record>`: on the
//! official 26.3 server, the record's blocks and fluids (no entities, +80 y as every replay) with (E)'s string laid
//! over the napper climb's exit detector E (7,-3,4): the string at (7,-2,4), hooks (7,-2,3) facing south and
//! (7,-2,5) facing north on glass (7,-2,2) / (7,-2,6), the string last (attached). An EMPTY minecart is summoned in
//! the napper lift's bubble column (6.5, y, 4.5) with no motion; it rises to the column's top, drifts east on the
//! top current onto E and is boosted east up the first booster (8,-3,4) (ascending east, always on). Every tick
//! until the cart is past x 9.5 (or 160 gt): E's powered state, the string's and both hooks', the cart's position
//! and box (frame). Printed per row: E's first powered tick, the string's first powered tick, the difference (the
//! bar: the string's first press >= E + 5), and where the cart was then. Rows: start y -8 and -11, and a control with
//! no cart. (G1's AE pattern) The known answers are asserted (4 asserts); MCTAS_E_STRING_PLANT=<k> inverts assert k.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;
const E: [i32; 3] = [7, -3, 4];
const STRING: [i32; 3] = [7, -2, 4];

fn w(p: [i32; 3]) -> [i32; 3] {
    [p[0], p[1] + DY, p[2]]
}

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1] + DY, p[2]))?;
    Ok(())
}

fn cart_pos(srv: &mut VanillaServer, tag: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1] - DY as f64, v[2]]) } else { None })
}

fn row(version: &str, record: &std::path::Path, start_y: Option<f64>) -> Result<ERow> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(20)?;
    crate::seatclear::place_blocks(&mut srv, record)?;
    set(&mut srv, [7, -2, 2], "minecraft:glass")?;
    set(&mut srv, [7, -2, 6], "minecraft:glass")?;
    set(&mut srv, [7, -2, 3], "minecraft:tripwire_hook[attached=false,facing=south,powered=false]")?;
    set(&mut srv, [7, -2, 5], "minecraft:tripwire_hook[attached=false,facing=north,powered=false]")?;
    set(&mut srv, STRING, "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
    srv.step(20)?;
    let attached = srv.test_block(w(STRING), "minecraft:tripwire[attached=true]")?;
    let label = start_y.map(|y| format!("an EMPTY cart summoned in the bubble column at (6.5, {y}, 4.5)")).unwrap_or_else(|| "no cart (the control)".into());
    println!("  row: {label}; the string attached {attached}");
    if let Some(y) = start_y {
        srv.cmd_ok(&format!("summon minecraft:minecart 6.5 {} 4.5 {{Tags:[\"ecart\"],Motion:[0.0d,0.0d,0.0d]}}", y + DY as f64))?;
    }
    let (mut e_first, mut s_first, mut s_at): (Option<u32>, Option<u32>, Option<[f64; 3]>) = (None, None, None);
    let mut s_on = 0u32;
    for t in 1..=160u32 {
        srv.step(1)?;
        let e = srv.test_block(w(E), "minecraft:detector_rail[powered=true]")?;
        let s = srv.test_block(w(STRING), "minecraft:tripwire[powered=true]")?;
        let p = if start_y.is_some() { cart_pos(&mut srv, "ecart")? } else { None };
        if e {
            e_first.get_or_insert(t);
        }
        if s {
            s_on += 1;
            if s_first.is_none() {
                s_first = Some(t);
                s_at = p;
            }
        }
        println!(
            "    t{t:>3}: E powered {e} | string powered {s} | cart {}",
            p.map(|p| format!("({:.4},{:.4},{:.4}) box y {:.4}..{:.4} (the band -1.9375..-1.8438)", p[0], p[1], p[2], p[1], p[1] + 0.7)).unwrap_or_else(|| "-".into())
        );
        if p.is_some_and(|p| p[0] > 9.5) || (start_y.is_none() && t >= 40) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let verdict = match (e_first, s_first) {
        (Some(e), Some(s)) => format!(
            "E first powered t{e}, the string first powered t{s}: E + {}{} (the cart then at {})",
            s as i64 - e as i64,
            if s as i64 - e as i64 >= 5 { " (>= E + 5: HOLDS)" } else { " (< E + 5: FAILS)" },
            s_at.map(|p| format!("({:.4},{:.4},{:.4}), box top {:.4}", p[0], p[1], p[2], p[1] + 0.7)).unwrap_or_else(|| "?".into())
        ),
        (Some(e), None) => format!("E first powered t{e}; the string never powered"),
        (None, Some(s)) => format!("E never powered; the string first powered t{s}"),
        (None, None) => "neither E nor the string powered".into(),
    };
    println!("    => {verdict}; the string powered on {s_on} ticks");
    Ok(ERow { summary: format!("{label}: {verdict}"), e_first, s_first, s_top: s_at.map(|p| p[1] + 0.7) })
}

/// One row's record: its summary line, E's and the string's first powered ticks, the cart's end-of-tick box top on
/// the string's first powered tick (frame).
struct ERow {
    summary: String,
    e_first: Option<u32>,
    s_first: Option<u32>,
    s_top: Option<f64>,
}

/// (G1's AE pattern) MCTAS_E_STRING_PLANT=<k> inverts assert k's expectation (k = 1..4): that run must fail at
/// assert k by name.
fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_E_STRING_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn e_string(version: &str, record: &std::path::Path) -> Result<()> {
    println!("== e-string on the official {version} server (ironspec 20:41): {} (blocks and fluids, +{DY} y) with (E)'s string over E (7,-3,4)", record.display());
    let mut rows = Vec::new();
    for start in [Some(-8.0), Some(-11.0), None] {
        rows.push(row(version, record, start).with_context(|| format!("row {start:?}"))?);
    }
    println!("  summary: {}", rows.iter().map(|r| r.summary.as_str()).collect::<Vec<_>>().join(" | "));
    // the known answers (the first run, 21:10): both starts press the string first at E + 13 (E at t15 / t20); the
    // press comes from the within-tick sweep (the end-of-tick box top under the band's bottom -1.9375); no cart, no
    // press
    pin(1, rows[0].e_first == Some(15) && rows[0].s_first == Some(28), format!("start -8: E first t15, the string first t28 (E + 13): {:?} / {:?}", rows[0].e_first, rows[0].s_first))?;
    pin(2, rows[1].e_first == Some(20) && rows[1].s_first == Some(33), format!("start -11: E first t20, the string first t33 (E + 13): {:?} / {:?}", rows[1].e_first, rows[1].s_first))?;
    pin(3, rows[..2].iter().all(|r| r.s_top.is_some_and(|t| t < -1.9375)), format!("both first presses come with the end-of-tick box top under the band (the swept check): {:?}", rows.iter().map(|r| r.s_top).collect::<Vec<_>>()))?;
    pin(4, rows[2].e_first.is_none() && rows[2].s_first.is_none(), "the control (no cart): neither E nor the string powered".into())?;
    println!("  every known answer held (asserts 1..4)");
    Ok(())
}
