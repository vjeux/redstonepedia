//! A recorded run: per frame, each input's state, each output's level and every non-air cell.

use crate::{Circuit, InputHow, Pos};
use std::collections::HashMap;

/// One run of a circuit, frames 0..=ticks (the state after t ticks, with the inputs due at t
/// applied).
#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// [input][frame]: whether the input is on, read from its cell (the on-state written there, a
    /// powered lever or button).
    pub inputs: Vec<Vec<bool>>,
    /// [output][frame]: the output's level ([`level`]).
    pub levels: Vec<Vec<u8>>,
    /// The cell states seen ([`Trace::cells`] holds indices into it).
    pub names: Vec<String>,
    /// [frame]: every non-air cell (position, index into `names`; full state: block state +
    /// comparator strength + container contents), sorted by position.
    pub cells: Vec<Vec<(Pos, u32)>>,
    /// The run ended early on purpose: a flying machine reached the far end of its corridor.
    pub ended: Option<String>,
    /// Primed TNT spawned during the run (TNT dupers; the judge removes each one before it can
    /// explode), in spawn order.
    pub primed: Vec<Primed>,
    /// TNT dupers judged with explosions (the default; real game only): how many primed TNT
    /// exploded during the run, and the frame of each explosion.
    pub exploded: u32,
    pub explosions: Vec<u32>,
    /// Burning TNT removed 16+ blocks from the design (harmless to it: `run::OUT_OF_RANGE`).
    pub out_of_range: u32,
    /// Every explosion (explosions on): where it went off and whether it was any use as a TNT
    /// (the TNT judge's rule: none may go off in a fluid or where it would destroy nothing).
    pub blasts: Vec<BlastUse>,
    /// Entity records (the elevator's rider): (frame, every entity's kind and feet position).
    /// redsim records every frame of a run with entities; the real game the end of each phase.
    pub entities: Vec<(u32, Vec<(String, [f64; 3])>)>,
}

/// One explosion of a TNT duper's run: its frame, its center (the real game: the TNT's last
/// sighting, at fuse 1, plus its last move), whether a fluid fills its cell (water, lava, a
/// waterlogged block: it destroys nothing) and the blocks it would destroy on open ground
/// (`redsim::explosion::open_ground`, its weakest rolls).
#[derive(Clone, Debug, Default)]
pub struct BlastUse {
    pub frame: u32,
    pub pos: [f64; 3],
    pub in_fluid: bool,
    pub open_ground: usize,
}

impl BlastUse {
    /// The judge's verdict on it: `None` when it is a useful explosion, else why not.
    pub fn useless(&self) -> Option<String> {
        let at = format!("frame {} at ({:.1}, {:.1}, {:.1})", self.frame, self.pos[0], self.pos[1], self.pos[2]);
        if self.in_fluid {
            Some(format!("a TNT explodes in a fluid ({at}): it destroys nothing, a duper whose TNT explodes in water is useless"))
        } else if self.open_ground == 0 {
            Some(format!("a TNT explodes where it would destroy nothing even on open ground ({at}: fluid all around it)"))
        } else {
            None
        }
    }
}

/// One primed TNT: the frame it first appears in, and where it is and how it moves there (the
/// real game: one tick after it spawned; redsim: its spawn point, no motion).
#[derive(Clone, Debug, Default)]
pub struct Primed {
    pub frame: u32,
    pub pos: [f64; 3],
    pub motion: Option<[f64; 3]>,
    /// The real game: its position and motion at fuse 1, the tick before it would explode (the
    /// judge removes it there); `None` when the run ended first, and on redsim.
    pub end: Option<([f64; 3], [f64; 3])>,
    /// The real game: its motion when its horizontal speed was largest, if that reached 0.4
    /// blocks per gt (a launch: the LAUNCHED direction).
    pub launch: Option<[f64; 3]>,
    /// The real game: the last frame its position changed (at rest since then).
    pub moved: u32,
    pub last_pos: [f64; 3],
    /// The frame `end` was recorded at.
    pub end_frame: u32,
    /// The real game, explosions on: its position and motion when it was removed out of range.
    pub gone: Option<([f64; 3], [f64; 3])>,
}

impl Trace {
    /// Build a trace from recorded cells and the best signal into each output cell, per frame.
    pub fn new(c: &Circuit, cells: Vec<Vec<(Pos, String)>>, signals: Vec<Vec<u8>>) -> Trace {
        let mut names: Vec<String> = Vec::new();
        let mut ids: HashMap<String, u32> = HashMap::new();
        let mut t = Trace { inputs: vec![Vec::with_capacity(cells.len()); c.inputs.len()], levels: vec![Vec::with_capacity(cells.len()); c.outputs.len()], ..Default::default() };
        // the judge's input devices (R7: a driven input's repeater, its support and its switch)
        // are not the circuit's: read for the input's state, then left out of the recorded cells
        let judge: Vec<Pos> = c
            .inputs
            .iter()
            .filter_map(|i| match i.how {
                InputHow::Driven { dir, .. } => {
                    let (r, sup, sw) = crate::driver_cells(i.pos, dir);
                    Some([r, sup, sw])
                }
                _ => None,
            })
            .flatten()
            .collect();
        // the previous frame's cells with their ids: a cell holding the same state as in the
        // previous frame keeps its id without a lookup (frames are sorted by position; a full
        // adder's run hashed every cell's state at every frame, 2026-11-08)
        let mut prev: Vec<(Pos, String, u32)> = Vec::new();
        for (f, frame) in cells.into_iter().enumerate() {
            let get = |p: Pos| frame.binary_search_by_key(&p, |(q, _)| *q).ok().map(|i| frame[i].1.as_str());
            for (k, inp) in c.inputs.iter().enumerate() {
                let s = get(inp.pos);
                let on = match &inp.how {
                    // an active-low input (on-state air: the pulse removes the rest block) reads on
                    // while its cell is empty
                    InputHow::Block { on, .. } if mctas::blockstate::is_air(on) => s.is_none_or(|s| state_matches(s, on)),
                    InputHow::Block { on, .. } => s.is_some_and(|s| state_matches(s, on)),
                    InputHow::Lever | InputHow::Button => s.is_some_and(|s| s.contains("powered=true")),
                    // the judge's repeater powering the input stone
                    InputHow::Driven { dir, .. } => get(crate::driver_cells(inp.pos, *dir).0).is_some_and(|s| s.contains("powered=true")),
                    InputHow::Strength { .. } | InputHow::Frame { .. } => false,
                };
                t.inputs[k].push(on);
            }
            for (k, out) in c.outputs.iter().enumerate() {
                let best = signals.get(f).and_then(|v| v.get(k)).copied().unwrap_or(0);
                t.levels[k].push(level(get(out.pos), best));
            }
            let mut cur: Vec<(Pos, String, u32)> = Vec::with_capacity(frame.len());
            let mut v: Vec<(Pos, u32)> = Vec::with_capacity(frame.len());
            let mut j = 0;
            for (p, s) in frame.into_iter().filter(|(p, _)| !judge.contains(p)) {
                while j < prev.len() && prev[j].0 < p {
                    j += 1;
                }
                let id = if j < prev.len() && prev[j].0 == p && prev[j].1 == s {
                    prev[j].2
                } else {
                    match ids.get(&s) {
                        Some(id) => *id,
                        None => {
                            let id = names.len() as u32;
                            ids.insert(s.clone(), id);
                            names.push(s.clone());
                            id
                        }
                    }
                };
                v.push((p, id));
                cur.push((p, s, id));
            }
            prev = cur;
            t.cells.push(v);
        }
        t.names = names;
        t
    }

    pub fn frames(&self) -> usize {
        self.cells.len()
    }

    /// Whether output `k` is on (level > 0) at frame `t`.
    pub fn on(&self, k: usize, t: usize) -> bool {
        self.levels[k].get(t).is_some_and(|l| *l > 0)
    }

    /// Output `k` on/off over frames `from..to`.
    pub fn out_seq(&self, k: usize) -> Vec<bool> {
        self.levels[k].iter().map(|l| *l > 0).collect()
    }

    /// The state of the cell at `p` in frame `t` (`None`: air).
    pub fn state(&self, t: usize, p: Pos) -> Option<&str> {
        let f = &self.cells[t];
        f.binary_search_by_key(&p, |(q, _)| *q).ok().map(|i| self.names[f[i].1 as usize].as_str())
    }

    /// Frames `from..to` as text, one line per input and output: `#` on, `.` off (an output's level
    /// as a hex digit when it is not 15).
    pub fn strip(&self, c: &Circuit, from: usize, to: usize) -> Vec<String> {
        let to = to.min(self.frames());
        let w = c.inputs.iter().map(|i| i.name.len()).chain(c.outputs.iter().map(|o| o.name.len())).max().unwrap_or(3).max(3);
        let mut out = vec![format!("{:w$}  frames {from}..{}", "", to.saturating_sub(1))];
        for (k, i) in c.inputs.iter().enumerate() {
            out.push(format!("{:w$}  {}", i.name, self.inputs[k][from..to].iter().map(|b| if *b { '#' } else { '.' }).collect::<String>()));
        }
        for (k, o) in c.outputs.iter().enumerate() {
            out.push(format!(
                "{:w$}  {}",
                o.name,
                self.levels[k][from..to].iter().map(|l| match *l { 0 => '.', 15 => '#', l => char::from_digit(l as u32, 16).unwrap_or('?') }).collect::<String>()
            ));
        }
        out
    }
}

/// Whether a recorded state is a lit redstone lamp (the judge's consumer, [`crate::CONSUMER`]).
pub fn lamp_lit(state: Option<&str>) -> bool {
    state.is_some_and(|s| s.starts_with("minecraft:redstone_lamp") && s.contains("lit=true"))
}

/// The level of an output cell holding `state` (`None`: air) with `best` the best signal its
/// neighbors give into it: redstone dust gives its power; a repeater, torch or observer its own
/// output (15 when on); a comparator its strength; any other cell (air, a solid block, a lamp, a
/// piston) the best signal into it (what a lamp there would see).
pub fn level(state: Option<&str>, best: u8) -> u8 {
    let Some(s) = state else {
        return best;
    };
    let id = s.split(['[', '{']).next().unwrap_or(s);
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    let prop = |k: &str| -> Option<&str> {
        let i = s.find(&format!("{k}="))? + k.len() + 1;
        s[i..].split([',', ']']).next()
    };
    match id {
        "redstone_wire" => prop("power").and_then(|p| p.parse().ok()).unwrap_or(0),
        "repeater" | "observer" => {
            if prop("powered") == Some("true") {
                15
            } else {
                0
            }
        }
        "redstone_torch" | "redstone_wall_torch" => {
            if prop("lit") == Some("true") {
                15
            } else {
                0
            }
        }
        "comparator" => match s.find("{out=") {
            Some(i) => s[i + 5..].split('}').next().and_then(|n| n.parse().ok()).unwrap_or(0),
            None => {
                if prop("powered") == Some("true") {
                    15
                } else {
                    0
                }
            }
        },
        _ => best,
    }
}

/// Whether a recorded state is `wanted`: the same block, and every property `wanted` names has the
/// same value (the game reports every property; a design may name only some).
pub fn state_matches(actual: &str, wanted: &str) -> bool {
    let split = |s: &str| -> (String, Vec<(String, String)>) {
        let s = s.split('{').next().unwrap_or(s);
        let (id, props) = match s.split_once('[') {
            Some((id, rest)) => (id, rest.trim_end_matches(']')),
            None => (s, ""),
        };
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        let props = props.split(',').filter(|p| !p.is_empty()).filter_map(|p| p.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        (id, props)
    };
    let (ia, pa) = split(actual);
    let (iw, pw) = split(wanted);
    ia == iw && pw.iter().all(|kv| pa.contains(kv))
}

/// The maximal runs of `true` in `seq`: (start, length).
pub fn runs(seq: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, b) in seq.iter().enumerate() {
        match (b, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i - s));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, seq.len() - s));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(level(None, 7), 7);
        assert_eq!(level(Some("minecraft:redstone_wire[east=side,north=none,power=9,south=none,west=side]"), 0), 9);
        assert_eq!(level(Some("minecraft:repeater[delay=1,facing=north,locked=false,powered=true]"), 0), 15);
        assert_eq!(level(Some("minecraft:redstone_wall_torch[facing=east,lit=false]"), 15), 0);
        assert_eq!(level(Some("minecraft:comparator[facing=north,mode=compare,powered=true]{out=11}"), 0), 11);
        assert_eq!(level(Some("minecraft:redstone_lamp[lit=true]"), 0), 0);
        assert_eq!(level(Some("minecraft:stone"), 13), 13);
    }

    #[test]
    fn matching() {
        assert!(state_matches("minecraft:redstone_block", "minecraft:redstone_block"));
        assert!(state_matches("minecraft:target[power=0]", "minecraft:target"));
        assert!(!state_matches("minecraft:target[power=3]", "minecraft:target[power=0]"));
        assert!(state_matches("minecraft:air", "air"));
        assert!(!state_matches("minecraft:stone", "minecraft:air"));
    }

    #[test]
    fn run_lengths() {
        assert_eq!(runs(&[false, true, true, false, true]), vec![(1, 2), (4, 1)]);
        assert!(runs(&[false, false]).is_empty());
    }
}
