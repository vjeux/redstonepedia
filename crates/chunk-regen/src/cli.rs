//! `mctas chunk ...`: the chunk-regeneration study's commands
//! (designs/chunk-regen/NOTES.md).

use crate::exp::{Case, Payload, run_case};
use crate::{Control, apply_control, diff_manifests, inspect, manifest_text, snap, write_manifest};
use anyhow::{Result, bail};
use clap::Subcommand;
use mctas::vanilla::chunkfile::Stream;
use std::path::PathBuf;

fn parse_chunk(s: &str) -> Result<(i32, i32), String> {
    let mut it = s.split(',');
    let x = it.next().and_then(|v| v.trim().parse().ok()).ok_or_else(|| format!("expected cx,cz, got `{s}`"))?;
    let z = it.next().and_then(|v| v.trim().parse().ok()).ok_or_else(|| format!("expected cx,cz, got `{s}`"))?;
    Ok((x, z))
}

fn parse_streams(names: &[String]) -> Result<Vec<Stream>> {
    if names.is_empty() {
        return Ok(Stream::all().to_vec());
    }
    let mut out = Vec::new();
    for n in names {
        for part in n.split(',') {
            out.push(Stream::parse(part.trim())?);
        }
    }
    Ok(out)
}

#[derive(Subcommand)]
pub enum ChunkCmd {
    /// Inspect specific chunks of a saved world (terrain + entity + POI streams):
    /// presence (absent/internal/external/...), stored and decompressed sizes, sha256 of
    /// the decompressed tag, Status and content counts. Absent is printed, not hidden.
    Inspect {
        /// Server run dir, level folder, or dimension folder.
        world: PathBuf,
        /// Chunk coordinates `cx,cz` (repeatable; default the study target 4,4 + neighbours).
        #[arg(long, value_parser = parse_chunk)]
        chunk: Vec<(i32, i32)>,
        /// Streams to show: region, entities, poi (comma-separated or repeated).
        #[arg(long)]
        stream: Vec<String>,
    },
    /// A manifest of every stored chunk of a world (one TSV line per chunk per stream).
    Snap {
        world: PathBuf,
        #[arg(long)]
        stream: Vec<String>,
        /// Write here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Diff two manifests from `chunk snap` / `chunk exp`.
    Diff { before: PathBuf, after: PathBuf },
    /// An offline positive control on a COPY of a disposable world: zero a header entry,
    /// drop an external `.mcc`, or garble a stored stream, so the next load must show
    /// the study harness what failure looks like. Refuses without `--on-copy`.
    Control {
        world: PathBuf,
        #[arg(long, value_parser = parse_chunk, default_value = "4,4")]
        chunk: (i32, i32),
        /// header-zero | drop-external | garbage
        #[arg(long)]
        mode: String,
        /// region | entities | poi
        #[arg(long, default_value = "region")]
        stream: String,
        /// Required: this world is a disposable copy.
        #[arg(long)]
        on_copy: bool,
    },
    /// Run one disposable-world experiment end to end on the official server
    /// (setup with commands, clean stop, optional offline control, reload in a fresh
    /// process, classify). Manifests land in `--out`.
    Exp {
        /// baseline | books | books-under | signs | poi | items |
        /// control-header | control-garbage | control-dropmcc
        name: String,
        /// Region compression like a dedicated server's property.
        #[arg(long, default_value = "deflate")]
        compression: String,
        /// sync-chunk-writes=true (the dedicated default is on).
        #[arg(long)]
        sync: bool,
        /// A normal generated world instead of the void superflat.
        #[arg(long)]
        normal_world: bool,
        /// Books/items/sign layers/poi layers override (meaning per experiment).
        #[arg(long)]
        count: Option<usize>,
        #[arg(long, default_value = "target/chunk-regen")]
        out_root: PathBuf,
    },
}

pub fn run(version: &str, command: ChunkCmd) -> Result<()> {
    match command {
        ChunkCmd::Inspect { world, chunk, stream } => {
            let chunks = if chunk.is_empty() { vec![(4, 4), (3, 4), (5, 4), (4, 3), (4, 5)] } else { chunk };
            let entries = inspect(&world, &parse_streams(&stream)?, &chunks);
            print!("{}", manifest_text(&entries));
        }
        ChunkCmd::Snap { world, stream, out } => {
            let entries = snap(&world, &parse_streams(&stream)?)?;
            match out {
                Some(path) => {
                    write_manifest(&path, &entries)?;
                    println!("{} chunks -> {}", entries.len(), path.display());
                }
                None => print!("{}", manifest_text(&entries)),
            }
        }
        ChunkCmd::Diff { before, after } => {
            let b = std::fs::read_to_string(&before)?;
            let a = std::fs::read_to_string(&after)?;
            print!("{}", diff_manifests(&b, &a)?);
        }
        ChunkCmd::Control { world, chunk, mode, stream, on_copy } => {
            if !on_copy {
                bail!("refusing to mutate a world without --on-copy (pass a disposable copy)");
            }
            let msg = apply_control(&world, Control::parse(&mode)?, Stream::parse(&stream)?, chunk.0, chunk.1)?;
            println!("{msg}");
        }
        ChunkCmd::Exp { name, compression, sync, normal_world, count, out_root } => {
            let case = build_case(&name, compression, sync, normal_world, count, &out_root)?;
            println!("{}", run_case(version, &case)?);
        }
    }
    Ok(())
}

fn build_case(name: &str, compression: String, sync: bool, normal_world: bool, count: Option<usize>, out_root: &PathBuf) -> Result<Case> {
    let base = |payload: Payload, control: Option<(Control, Stream)>| Case {
        name: name.to_string(),
        payload,
        control,
        compression: compression.clone(),
        sync,
        normal_world,
        out_dir: out_root.join(name),
    };
    Ok(match name {
        "baseline" => base(Payload::None, None),
        "books" => base(Payload::Books { books: count.unwrap_or(1400), page_len: 900 }, None),
        "books-under" => base(Payload::Books { books: count.unwrap_or(120), page_len: 900 }, None),
        "signs" => base(Payload::Signs { layers: count.unwrap_or(3) }, None),
        "poi" => base(Payload::Poi { layers: count.unwrap_or(8) }, None),
        "items" => base(Payload::Items { items: count.unwrap_or(900), page_len: 800 }, None),
        "control-header" => base(Payload::None, Some((Control::HeaderZero, Stream::Region))),
        "control-garbage" => base(Payload::None, Some((Control::Garbage, Stream::Region))),
        "control-dropmcc" => base(Payload::Books { books: count.unwrap_or(1400), page_len: 900 }, Some((Control::DropExternal, Stream::Region))),
        "control-dropmcc-entities" => {
            base(Payload::Items { items: count.unwrap_or(1400), page_len: 800 }, Some((Control::DropExternal, Stream::Entities)))
        }
        "control-entities" => base(Payload::None, Some((Control::HeaderZero, Stream::Entities))),
        "control-poi" => base(Payload::None, Some((Control::HeaderZero, Stream::Poi))),
        other => bail!("unknown experiment `{other}` (baseline|books|books-under|signs|poi|items|control-*)"),
    })
}
