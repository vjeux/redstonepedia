//! Bridge snapshot probes for shape automata redsim does not model (walls, stairs,
//! pointed dripstone, chests' pairing, snow layers, chorus, vines, pale moss carpet,
//! iron bars, nether portals). The bridge cannot count updater calls, so the observable
//! is the *net state change*: build the network with update-mode placement (the game
//! settles its own shapes), snapshot the region, fire one trigger setBlock, step one
//! tick and then to idle, snapshot again, and diff. A branching automaton would show
//! changed-cell counts growing superlinearly in the size parameter (and, with the
//! per-flip counted-call costs read from the source, an implied counted total);
//! a one-ring or one-column effect stays constant/linear.

use anyhow::Result;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker, WorkerConfig};
use std::collections::HashMap;
use std::time::Instant;

type Cells = Vec<(Pos, String)>;

fn p(x: i32, y: i32, z: i32) -> Pos {
    [x, y, z]
}

/// (blocks, trigger pos, trigger state, bounds lo, bounds hi)
fn generate(family: &str, n: i32, y0: i32) -> Result<(Cells, Pos, String, Pos, Pos)> {
    let mut v: Cells = Vec::new();
    let put = |v: &mut Cells, x: i32, y: i32, z: i32, s: &str| v.push((p(x, y, z), s.to_string()));
    Ok(match family {
        "wall" => {
            // A wall column of height n; trigger adds a wall on top.
            put(&mut v, 0, y0, 0, "minecraft:stone");
            for y in 1..=n {
                put(&mut v, 0, y0 + y, 0, "minecraft:cobblestone_wall");
            }
            (v, p(0, y0 + n + 1, 0), "minecraft:cobblestone_wall".into(), p(-2, y0, -2), p(2, y0 + n + 2, 2))
        }
        "wallfield" => {
            // n x n walls, two high; trigger removes the center top wall.
            for x in 0..n {
                for z in 0..n {
                    put(&mut v, x, y0, z, "minecraft:stone");
                    put(&mut v, x, y0 + 1, z, "minecraft:cobblestone_wall");
                    put(&mut v, x, y0 + 2, z, "minecraft:cobblestone_wall");
                }
            }
            (v, p(n / 2, y0 + 2, n / 2), "minecraft:air".into(), p(-1, y0, -1), p(n, y0 + 3, n))
        }
        "wallrun" => {
            // A 5-long wall run, n high: settled sides are TALL (except the top layer)
            // and UP=false throughout. One extra wall on top of the middle column has
            // UP=true; if `shouldRaisePost`'s above-has-post rule walks, every wall
            // under it flips UP in the same cascade.
            for x in 0..5 {
                put(&mut v, x, y0, 0, "minecraft:stone");
                for y in 1..=n {
                    put(&mut v, x, y0 + y, 0, "minecraft:cobblestone_wall");
                }
            }
            (v, p(2, y0 + n + 1, 0), "minecraft:cobblestone_wall".into(), p(-1, y0, -1), p(5, y0 + n + 2, 1))
        }
        "stair" => {
            // Field with the center stair missing; the trigger places it (its own shape
            // and up to four neighbours' corner shapes are the whole effect if the shape
            // function is the static one read in the source).
            let facings = ["east", "south", "west", "north"];
            for x in 0..n {
                for z in 0..n {
                    put(&mut v, x, y0, z, "minecraft:stone");
                    if !(x == n / 2 && z == n / 2) {
                        put(&mut v, x, y0 + 1, z, &format!("minecraft:oak_stairs[facing={}]", facings[((x + z) % 4) as usize]));
                    }
                }
            }
            (v, p(n / 2, y0 + 1, n / 2), "minecraft:oak_stairs[facing=east]".into(), p(-1, y0, -1), p(n, y0 + 2, n))
        }
        "drip" => {
            // A hanging column; trigger extends the tip by one.
            put(&mut v, 0, y0 + n + 1, 0, "minecraft:stone");
            for k in 0..n {
                put(&mut v, 0, y0 + n - k, 0, "minecraft:pointed_dripstone[vertical_direction=down]");
            }
            (v, p(0, y0, 0), "minecraft:pointed_dripstone[vertical_direction=down]".into(), p(-2, y0 - 1, -2), p(2, y0 + n + 2, 2))
        }
        "chest" => {
            // Paired as survival placement leaves them (setblock alone never pairs:
            // LEFT's partner lies clockwise from its facing, i.e. east of it here).
            for x in 0..n {
                put(&mut v, x, y0, 0, "minecraft:stone");
                let t = if x % 2 == 0 { "left" } else { "right" };
                put(&mut v, x, y0 + 1, 0, &format!("minecraft:chest[facing=north,type={t}]"));
            }
            (v, p(1, y0 + 1, 0), "minecraft:air".into(), p(-1, y0, -1), p(n, y0 + 2, 1))
        }
        "snow" => {
            // A tower of full snow layers; trigger removes the lowest.
            put(&mut v, 0, y0, 0, "minecraft:stone");
            for y in 1..=n {
                put(&mut v, 0, y0 + y, 0, "minecraft:snow[layers=8]");
            }
            (v, p(0, y0 + 1, 0), "minecraft:air".into(), p(-2, y0, -2), p(2, y0 + n + 1, 2))
        }
        "chorus" => {
            // A plus of chorus plants over end stone, arms length n/2; trigger breaks
            // the center base plant (the rest must lose support on scheduled ticks).
            put(&mut v, 0, y0, 0, "minecraft:end_stone");
            for k in 1..=n {
                put(&mut v, 0, y0 + k, 0, "minecraft:chorus_plant");
            }
            for k in 1..=n / 2 {
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    put(&mut v, dx * k, y0 + n, dz * k, "minecraft:chorus_plant");
                }
            }
            (v, p(0, y0 + 1, 0), "minecraft:air".into(), p(-n / 2 - 1, y0, -n / 2 - 1), p(n / 2 + 1, y0 + n + 1, n / 2 + 1))
        }
        "vine" => {
            // A stone wall n wide, n/2 high, vines on its south face; break a center stone.
            for x in 0..n {
                for y in 1..=n / 2 {
                    put(&mut v, x, y0 + y, 0, "minecraft:stone");
                    put(&mut v, x, y0 + y, 1, "minecraft:vine[north=true]");
                }
                put(&mut v, x, y0, 0, "minecraft:stone");
            }
            (v, p(n / 2, y0 + n / 2, 0), "minecraft:air".into(), p(-1, y0, -1), p(n, y0 + n / 2 + 1, 2))
        }
        "pane" => {
            for x in 0..n {
                for z in 0..n {
                    put(&mut v, x, y0, z, "minecraft:stone");
                    put(&mut v, x, y0 + 1, z, "minecraft:iron_bars");
                }
            }
            (v, p(n / 2, y0 + 1, n / 2), "minecraft:air".into(), p(-1, y0, -1), p(n, y0 + 2, n))
        }
        "portal" => {
            // Obsidian frame n wide (interior n-2), 5 high; trigger breaks a corner.
            anyhow::ensure!((4..=23).contains(&n), "portal width must be 4..=23");
            for x in 0..n {
                put(&mut v, x, y0, 0, "minecraft:obsidian");
                put(&mut v, x, y0 + 4, 0, "minecraft:obsidian");
            }
            for y in 1..4 {
                put(&mut v, 0, y0 + y, 0, "minecraft:obsidian");
                put(&mut v, n - 1, y0 + y, 0, "minecraft:obsidian");
                for x in 1..n - 1 {
                    put(&mut v, x, y0 + y, 0, "minecraft:nether_portal[axis=x]");
                }
            }
            (v, p(n / 2, y0, 0), "minecraft:air".into(), p(-1, y0, -1), p(n, y0 + 5, 1))
        }
        "moss" => {
            // Pale moss carpet on a floor with a back wall to climb; trigger removes one
            // wall stone behind the carpets.
            for x in 0..n {
                for z in 0..4 {
                    put(&mut v, x, y0, z, "minecraft:stone");
                    put(&mut v, x, y0 + 1, z, "minecraft:pale_moss_carpet");
                }
                put(&mut v, x, y0 + 1, -1, "minecraft:stone");
                put(&mut v, x, y0 + 2, -1, "minecraft:stone");
            }
            (v, p(n / 2, y0 + 1, -1), "minecraft:air".into(), p(-1, y0, -2), p(n, y0 + 3, 4))
        }
        other => anyhow::bail!("unknown bridge family {other}"),
    })
}

fn snapshot(w: &mut Worker, sb: u32, lo: Pos, hi: Pos) -> Result<HashMap<Pos, String>> {
    let r = w.get_region(sb, lo, hi)?;
    Ok(r.blocks().into_iter().collect())
}

pub struct BProbeReport {
    pub family: String,
    pub n: i32,
    pub changed_1tick: usize,
    pub changed_idle: usize,
    pub trigger_ms: u128,
}

pub fn run_bprobe(version: &str, families: &[String], sizes: &[i32]) -> Result<Vec<BProbeReport>> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let y0 = 64;
    let mut out = Vec::new();
    for fam in families {
        for &n in sizes {
            let (blocks, tpos, tstate, lo, hi) = generate(fam, n, y0)?;
            w.clear(sb)?;
            w.set_blocks(sb, &blocks, PlaceMode::Update)?;
            w.step_until_idle(sb, 400)?;
            let before = snapshot(&mut w, sb, lo, hi)?;
            if std::env::var_os("SUPPRESS_DUMP").is_some() {
                eprintln!("bprobe {fam} n={n} trigger {tpos:?}={:?} neighbors:", before.get(&tpos));
                for d in [[0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]] {
                    let q = [tpos[0] + d[0], tpos[1] + d[1], tpos[2] + d[2]];
                    eprintln!("  {q:?} = {:?}", before.get(&q));
                }
            }
            let t0 = Instant::now();
            w.set_blocks(sb, &[(tpos, tstate.clone())], PlaceMode::Update)?;
            let trigger_ms = t0.elapsed().as_millis();
            let s1 = snapshot(&mut w, sb, lo, hi)?;
            w.step(sb, 1)?;
            let s2 = snapshot(&mut w, sb, lo, hi)?;
            w.step_until_idle(sb, 400)?;
            let s3 = snapshot(&mut w, sb, lo, hi)?;
            let diff = |a: &HashMap<Pos, String>, b: &HashMap<Pos, String>| {
                let mut keys: std::collections::HashSet<&Pos> = a.keys().collect();
                keys.extend(b.keys());
                keys.iter().filter(|k| a.get(**k) != b.get(**k)).count()
            };
            let _ = s1;
            out.push(BProbeReport { family: fam.clone(), n, changed_1tick: diff(&before, &s2), changed_idle: diff(&before, &s3), trigger_ms });
        }
    }
    Ok(out)
}
