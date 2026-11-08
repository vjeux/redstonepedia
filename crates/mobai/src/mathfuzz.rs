//! The math probe: redsim's ports of the math the mob AI uses (`jmath::sin`/`cos`: HotSpot's
//! `Math.sin`/`cos`; `mth::atan2`, `mth::sin`/`cos`: `Mth`'s tables) against the game JVM's own
//! answers (the bridge's `math` op), bit for bit, on random inputs.

use anyhow::{Result, anyhow};
use mctas::sim::{Worker, WorkerConfig};
use redsim::entity::{jmath, mth};
use redsim::rng::LegacyRandom;
use serde_json::{Value, json};

pub fn run(version: &str, n: usize, seed: i64, extra: &[String]) -> Result<()> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let mut r = LegacyRandom::new(seed);
    let mut xs: Vec<f64> = Vec::with_capacity(n);
    for s in extra {
        let v: f64 = if let Some(h) = s.strip_prefix("0x") { f64::from_bits(u64::from_str_radix(h, 16)?) } else { s.parse()? };
        xs.push(v);
    }
    while xs.len() < n {
        let k = xs.len() % 4;
        let v = match k {
            // RandomLookAround's angle: 2π x nextDouble
            0 => std::f64::consts::PI * 2.0 * r.next_double(),
            // a coordinate difference (relX after adding and taking off a position)
            1 => (r.next_double() - 0.5) * 64.0,
            // small offsets
            2 => (r.next_double() - 0.5) * 1e-3,
            // wide angles
            _ => (r.next_double() - 0.5) * 2000.0,
        };
        xs.push(v);
    }
    let mut diffs = [0usize; 6];
    let names = ["Math.sin", "Math.cos", "Math.sqrt", "Mth.sin", "Mth.cos", "Mth.atan2"];
    let mut shown = 0;
    for chunk in xs.chunks(2000) {
        let input: Vec<String> = chunk.iter().map(|v| (v.to_bits() as i64).to_string()).collect();
        let res = w.call(json!({"op": "math", "x": input}))?;
        let out = res.get("out").and_then(Value::as_array).ok_or_else(|| anyhow!("math: no out"))?;
        for (k, row) in out.iter().enumerate() {
            let x = chunk[k];
            let y = chunk[(k + 1) % chunk.len()];
            let row = row.as_array().ok_or_else(|| anyhow!("math: a row"))?;
            let g = |c: usize| -> i64 { row[c].as_str().unwrap_or("0").parse().unwrap_or(0) };
            let ours: [i64; 6] = [
                jmath::sin(x).to_bits() as i64,
                jmath::cos(x).to_bits() as i64,
                x.abs().sqrt().to_bits() as i64,
                mth::sin(x as f32 as f64).to_bits() as i32 as i64,
                mth::cos(x as f32 as f64).to_bits() as i32 as i64,
                mth::atan2(x, y).to_bits() as i64,
            ];
            for c in 0..6 {
                if ours[c] != g(c) {
                    diffs[c] += 1;
                    if shown < 12 {
                        shown += 1;
                        let (gv, ov) = if c == 3 || c == 4 {
                            (f32::from_bits(g(c) as u32) as f64, f32::from_bits(ours[c] as u32) as f64)
                        } else {
                            (f64::from_bits(g(c) as u64), f64::from_bits(ours[c] as u64))
                        };
                        println!("{}({x:e} [{:#x}]{}): game {gv:e} | redsim {ov:e}", names[c], x.to_bits(), if c == 5 { format!(", {y:e}") } else { String::new() });
                    }
                }
            }
        }
    }
    for c in 0..6 {
        println!("{}: {} of {} differ", names[c], diffs[c], xs.len());
    }
    Ok(())
}
