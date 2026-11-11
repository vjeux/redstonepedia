//! `mctas mobfarm r1-combine` (farmopt Nov 11 10:47, ironspec's S3 + (c′) + R1 file): a whole
//! loop (S3's bench file, C, its frame the loop frame moved by its queue line's `shift`) with its
//! old departure, return run and register head taken out, a stand departure part laid in their
//! place ((c′), ending at R1's take-off) and R1's tail taken from a queue-loop design that has it
//! (`--tail-esc 5 --esc-adm --adm-ledge --adm-xq 4 --xq-z 25 --tail-posts ...`, the base's
//! register by `--slots`/`--row-a`). The header follows: the declared states (the base's outside
//! the removed boxes, the departure's, the tail's), R1's flights, the departure's border and
//! handoff lines, and the queue line (the base's options with R1's tail options and the tail
//! file's slots and row A). The approach (row B's end to the stand) is the base's, untouched, so a
//! base with another approach combines the same way.

use mctas::design::{Design, EntitySpec};

type Pos = [i32; 3];
type Bx = (Pos, Pos);

fn inside(b: &Bx, c: &Pos) -> bool {
    (0..3).all(|k| b.0[k] <= c[k] && c[k] <= b.1[k])
}

/// Every `x,y,z` triple of integers in `s` moved by `d` (the rest of the text kept).
fn shift_triples(s: &str, d: Pos) -> String {
    let mut out = String::new();
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        let v: Vec<&str> = tok.split(',').collect();
        let n: Vec<Option<i32>> = v.iter().map(|t| t.parse::<i32>().ok()).collect();
        if v.len() == 3 && n.iter().all(|x| x.is_some()) {
            out.push_str(&format!("{},{},{}", n[0].unwrap() + d[0], n[1].unwrap() + d[1], n[2].unwrap() + d[2]));
        } else {
            out.push_str(tok);
        }
        tok.clear();
    };
    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == ',' || (ch == '-' && (tok.is_empty() || tok.ends_with(','))) {
            tok.push(ch);
        } else {
            flush(&mut tok, &mut out);
            out.push(ch);
        }
    }
    flush(&mut tok, &mut out);
    out
}

/// The `declared:` entries of `d`: (cell, the entry's text).
fn declared(d: &Design) -> Vec<(Pos, String)> {
    d.meta
        .get("declared")
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .filter_map(|t| {
                    let (c, rest) = t.split_once(' ')?;
                    let v: Vec<i32> = c.split(',').filter_map(|x| x.parse().ok()).collect();
                    (v.len() == 3).then(|| ([v[0], v[1], v[2]], rest.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A queue line's `key=value` tokens.
fn queue_tokens(s: &str) -> Vec<(String, String)> {
    s.split_whitespace().filter_map(|t| t.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect()
}

pub struct Combined {
    pub design: Design,
    pub lines: Vec<String>,
}

/// The combine. `remove`: the base's boxes to take out, in the loop frame; `tail_box`: R1's
/// cells in the tail file (the loop frame), the tail posts' columns added from its queue line.
pub fn combine(base: &Design, departure: &Design, tail: &Design, remove: &[Bx], tail_box: Bx, name: &str) -> Result<Combined, String> {
    let mut lines = Vec::new();
    let bq = base.meta.get("queue").ok_or("the base has no queue: line")?;
    let tq = tail.meta.get("queue").ok_or("the tail file has no queue: line")?;
    let shift = queue_tokens(bq).into_iter().find(|(k, _)| k == "shift").map(|(_, v)| {
        let n: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        [n.first().copied().unwrap_or(0), 0, n.get(1).copied().unwrap_or(0)]
    });
    let d = shift.unwrap_or([0, 0, 0]);
    let mv = |c: &Pos| [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
    let in_removed = |c: &Pos| remove.iter().any(|b| inside(&(mv(&b.0), mv(&b.1)), c));
    // (R1's cells in the tail file: its box and each tail post's column, glass to leaves)
    let posts: Vec<Pos> = queue_tokens(tq)
        .into_iter()
        .find(|(k, _)| k == "tail_posts")
        .map(|(_, v)| v.split(';').filter_map(|p| {
            let n: Vec<i32> = p.split(',').filter_map(|x| x.parse().ok()).collect();
            (n.len() == 3).then(|| [n[0], n[1], n[2]])
        }).collect())
        .unwrap_or_default();
    let in_tail = |c: &Pos| inside(&tail_box, c) || posts.iter().any(|p| c[0] == p[0] && c[2] == p[2] && (p[1] - 1..=p[1] + 3).contains(&c[1]));
    let in_tail_e = |p: &[f64; 3]| {
        let c = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        in_tail(&c)
    };
    let mut out = base.clone();
    out.name = name.to_string();
    let before = out.blocks.len();
    out.blocks.retain(|c, _| !in_removed(c));
    out.block_entities.retain(|c, _| !in_removed(c));
    let gone_e: Vec<EntitySpec> = out.entities.iter().filter(|e| in_removed(&[e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32])).cloned().collect();
    out.entities.retain(|e| !in_removed(&[e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
    lines.push(format!("the base: {} blocks taken out of {} boxes, {} entities ({})", before - out.blocks.len(), remove.len(), gone_e.len(), gone_e.iter().map(|e| format!("{} at {:.2},{:.2},{:.2}", e.kind.trim_start_matches("minecraft:"), e.pos[0] - d[0] as f64, e.pos[1], e.pos[2] - d[2] as f64)).collect::<Vec<_>>().join(", ")));
    let mut clashes = Vec::new();
    let mut lay = |out: &mut Design, from: &Design, keep: &dyn Fn(&Pos) -> bool, what: &str, lines: &mut Vec<String>| {
        let mut n = 0;
        for (c, s) in &from.blocks {
            if !keep(c) {
                continue;
            }
            let at = mv(c);
            match out.blocks.get(&at) {
                Some(t) if t != s => clashes.push(format!("{what} {at:?}: the base's {t}, the part's {s}")),
                _ => {
                    out.blocks.insert(at, s.clone());
                    n += 1;
                }
            }
        }
        for (c, s) in &from.block_entities {
            if keep(c) {
                out.block_entities.insert(mv(c), s.clone());
            }
        }
        lines.push(format!("{what}: {n} blocks laid"));
    };
    lay(&mut out, departure, &|_| true, "the departure", &mut lines);
    lay(&mut out, tail, &in_tail, "R1's tail", &mut lines);
    if !clashes.is_empty() {
        return Err(format!("{} cells both hold with different blocks: {}", clashes.len(), clashes.iter().take(12).cloned().collect::<Vec<_>>().join("; ")));
    }
    for e in departure.entities.iter().chain(tail.entities.iter().filter(|e| in_tail_e(&e.pos))) {
        let mut e = e.clone();
        e.pos = [e.pos[0] + d[0] as f64, e.pos[1], e.pos[2] + d[2] as f64];
        out.entities.push(e);
    }
    let carts = out.entities.iter().filter(|e| e.kind.ends_with("minecart")).count();
    lines.push(format!("carts: {carts}"));
    // (the declared states: the base's outside the removed boxes, the departure's, the tail's)
    let mut dec: Vec<(Pos, String)> = declared(base).into_iter().filter(|(c, _)| !in_removed(c)).collect();
    for (c, s) in declared(departure).into_iter().map(|(c, s)| (mv(&c), s)).chain(declared(tail).into_iter().filter(|(c, _)| in_tail(c)).map(|(c, s)| (mv(&c), s))) {
        match dec.iter().find(|(x, _)| *x == c) {
            Some((_, t)) if *t != s => return Err(format!("declared {c:?}: the base's {t}, the part's {s}")),
            Some(_) => {}
            None => dec.push((c, s)),
        }
    }
    lines.push(format!("declared: {} entries", dec.len()));
    out.meta.insert("declared".into(), dec.iter().map(|(c, s)| format!("{},{},{} {s}", c[0], c[1], c[2])).collect::<Vec<_>>().join("; "));
    // (R1's flights, the departure's border and handoff, moved to the base's frame)
    if let Some(f) = tail.meta.get("flights") {
        out.meta.insert("flights".into(), shift_triples(f, d));
    }
    for k in ["border", "handoff"] {
        if let Some(v) = departure.meta.get(k) {
            let mut v = shift_triples(v, d);
            // ("stand-x A..B": an x range)
            if let Some(i) = v.find("stand-x ") {
                let rest = &v[i + 8..];
                let end = rest.find(|ch: char| !(ch.is_ascii_digit() || ch == '-' || ch == '.')).unwrap_or(rest.len());
                if let Some((a, b)) = rest[..end].split_once("..")
                    && let (Ok(a), Ok(b)) = (a.parse::<i32>(), b.parse::<i32>())
                {
                    v = format!("{}stand-x {}..{}{}", &v[..i], a + d[0], b + d[0], &rest[end..]);
                }
            }
            out.meta.insert(k.into(), v);
        }
    }
    // (the queue line: the base's options; the tail file's register and R1's tail options)
    let take = ["slots", "row_a", "tail_esc", "esc_adm", "adm_ledge", "adm_xq", "xq_gap", "xq_drop", "xq_z", "tail_post_x", "tail_posts"];
    let drop = ["tail_desc", "tail_deep", "tail_fifo", "tail_plain", "tail_brake"];
    let tt = queue_tokens(tq);
    let mut q: Vec<(String, String)> = Vec::new();
    for t in bq.split_whitespace() {
        match t.split_once('=') {
            Some((k, _)) if drop.contains(&k) || take.contains(&k) => {}
            Some((k, v)) => q.push((k.to_string(), v.to_string())),
            None => q.push((t.to_string(), String::new())),
        }
    }
    for k in take {
        if let Some((_, v)) = tt.iter().find(|(x, _)| x == k) {
            q.push((k.to_string(), v.clone()));
        }
    }
    let line = q.iter().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k}={v}") }).collect::<Vec<_>>().join(" ");
    lines.push(format!("queue: {line}"));
    out.meta.insert("queue".into(), line);
    Ok(Combined { design: out, lines })
}
