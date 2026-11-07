//! The cut of `mctas bedrock yt`: the video, the vertical Short and the thumbnail, from the
//! recorded segments, their timeline (wall-clock events) and the game audio (OpenAL Soft's wave
//! writer, one file per client session, synced through the file's size sampled at each segment's
//! start and end).
//!
//! Every clip is rendered on its own (cut, speed, overlays drawn with the game's font, the game
//! audio of the same wall-clock span), then all clips are joined and encoded once: VP9 + Opus.

use crate::portal::Pos;
use crate::yt::{self, Seg, Timeline, YtOptions, YtPlan, add};
use anyhow::{Context, Result, bail};
use mctas::mcfont::{Font, Image};
use std::path::{Path, PathBuf};

const FPS: u32 = 30;
const YELLOW: [u8; 3] = [255, 222, 40];
const WHITE: [u8; 3] = [255, 255, 255];
const GREEN: [u8; 3] = [90, 255, 90];
const RED: [u8; 3] = [255, 80, 70];
const AQUA: [u8; 3] = [110, 230, 255];
const GRAY: [u8; 3] = [200, 200, 200];

fn ff(args: &[String]) -> Result<()> {
    let out = std::process::Command::new(mctas::video::ffmpeg()?).args(["-loglevel", "error", "-y"]).args(args).output()?;
    if !out.status.success() {
        bail!("ffmpeg {:?} failed: {}", args.iter().take(12).collect::<Vec<_>>(), String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

fn probe_duration(path: &Path) -> Result<f64> {
    let out = std::process::Command::new(mctas::video::ffmpeg()?.with_file_name("ffprobe"))
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()?;
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().with_context(|| format!("duration of {}", path.display()))
}

// ---- drawing ---------------------------------------------------------------------------------

/// Text with a black outline one font pixel wide (readable on any background).
fn outlined(font: &Font, img: &mut Image, x: i64, y: i64, text: &str, scale: u32, color: [u8; 3]) {
    let o = scale as i64;
    for (dx, dy) in [(-o, -o), (0, -o), (o, -o), (-o, 0), (o, 0), (-o, o), (0, o), (o, o), (o + o / 2, o + o / 2)] {
        font.draw(img, x + dx, y + dy, text, scale, [0, 0, 0], false);
    }
    font.draw(img, x, y, text, scale, color, false);
}

fn outlined_centered(font: &Font, img: &mut Image, cx: i64, y: i64, text: &str, scale: u32, color: [u8; 3]) {
    let w = font.width(text, scale) as i64;
    outlined(font, img, cx - w / 2, y, text, scale, color);
}

/// A bottom caption: up to two wrapped lines on a dark bar; `color` for the first line.
fn caption(font: &Font, w: u32, h: u32, text: &str, color: [u8; 3]) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let scale = if w > h { 4 } else { 6 };
    let mut lines = Vec::new();
    for part in text.split('\n') {
        lines.extend(font.wrap(part, scale, w - 80));
    }
    let lh = 9 * scale as i64 + 6;
    let total = lh * lines.len() as i64;
    // Above the hotbar (the HUD's bottom 70 px) in landscape.
    let y0 = if w > h { h as i64 - 92 - total } else { h as i64 - 300 - total };
    let maxw = lines.iter().map(|l| font.width(l, scale)).max().unwrap_or(0) as i64;
    img.fill_rect(w as i64 / 2 - maxw / 2 - 18, y0 - 12, maxw + 36, total + 16, [0, 0, 0, 165]);
    for (i, l) in lines.iter().enumerate() {
        outlined_centered(font, &mut img, w as i64 / 2, y0 + i as i64 * lh, l, scale, if i == 0 { color } else { WHITE });
    }
    img
}

/// The step badge (top left): "3/11" big, the block name and how it faces.
fn badge(font: &Font, w: u32, h: u32, n: &str, title: &str, sub: &str) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let (bx, by) = (24i64, 22i64);
    let nw = font.width(n, 9) as i64;
    let tw = font.width(title, 5).max(font.width(sub, 3)) as i64;
    img.fill_rect(bx - 12, by - 10, nw + tw + 60, 104, [0, 0, 0, 160]);
    outlined(font, &mut img, bx, by, n, 9, YELLOW);
    outlined(font, &mut img, bx + nw + 28, by + 4, title, 5, WHITE);
    outlined(font, &mut img, bx + nw + 28, by + 58, sub, 3, AQUA);
    img
}

/// Big centered lines (the hook, titles): (text, scale, color), from `y0` down.
fn big(font: &Font, w: u32, h: u32, y0: i64, lines: &[(&str, u32, [u8; 3])]) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let mut y = y0;
    for (t, s, c) in lines {
        outlined_centered(font, &mut img, w as i64 / 2, y, t, *s, *c);
        y += 9 * *s as i64 + 10;
    }
    img
}

/// A full-frame card over a darkened background (drawn by ffmpeg underneath): a title and lines.
fn card(font: &Font, w: u32, h: u32, title: &str, lines: &[(&str, [u8; 3])], icons: &[(Image, usize)]) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 120]);
    outlined_centered(font, &mut img, w as i64 / 2, 46, title, 7, YELLOW);
    let mut y = 150i64;
    for (i, (t, c)) in lines.iter().enumerate() {
        let x = 160i64;
        if let Some((icon, _)) = icons.iter().find(|(_, k)| *k == i) {
            img.draw_resized(icon, x - 70, y - 6, 48, 48);
        }
        outlined(font, &mut img, x, y, t, 4, *c);
        y += 9 * 4 + 22;
    }
    img
}

/// The rhythm bar: six ticks of the 0.3 s cycle, the armed one green, the current one lit.
fn beat_bar(font: &Font, w: u32, h: u32, phase: usize) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let (bw, gap) = (58i64, 10i64);
    let total = 6 * bw + 5 * gap;
    let x0 = w as i64 / 2 - total / 2;
    let y0 = 120i64;
    img.fill_rect(x0 - 16, y0 - 46, total + 32, bw + 62, [0, 0, 0, 140]);
    outlined_centered(font, &mut img, w as i64 / 2, y0 - 38, "MACHINE RHYTHM (1 box = 1 tick)", 3, GRAY);
    for i in 0..6usize {
        let x = x0 + i as i64 * (bw + gap);
        let armed = i == 0;
        let lit = i == phase;
        let c = match (armed, lit) {
            (true, true) => [80, 255, 80, 255],
            (true, false) => [40, 120, 40, 230],
            (false, true) => [255, 255, 255, 255],
            (false, false) => [70, 70, 70, 220],
        };
        img.fill_rect(x, y0, bw, bw, c);
        if armed {
            outlined_centered(font, &mut img, x + bw / 2, y0 + bw / 2 - 12, "GO", 3, if lit { WHITE } else { GRAY });
        }
    }
    img
}

/// Project a world point through a first-person camera (eye, yaw, pitch, vertical fov) to pixels.
pub fn project(eye: [f64; 3], yaw: f64, pitch: f64, vfov: f64, w: f64, h: f64, p: [f64; 3]) -> Option<(f64, f64)> {
    let (y, pi) = (yaw.to_radians(), pitch.to_radians());
    let f = [-y.sin() * pi.cos(), -pi.sin(), y.cos() * pi.cos()];
    let r = [-y.cos(), 0.0, -y.sin()];
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (xc, yc, zc) = (dot(d, r), dot(d, u), dot(d, f));
    if zc < 0.2 {
        return None;
    }
    let k = (h / 2.0) / (vfov.to_radians() / 2.0).tan();
    Some((w / 2.0 + xc / zc * k, h / 2.0 - yc / zc * k))
}

/// Numbered markers on the machine's blocks seen from a recorded pose.
fn labels(font: &Font, w: u32, h: u32, pose: &str, t: Pos) -> Option<Image> {
    let get = |k: &str| pose.split(&format!("{k}=")).nth(1).and_then(|r| r.split(' ').next()).map(str::to_string);
    let eye: Vec<f64> = get("eye")?.split(',').filter_map(|v| v.parse().ok()).collect();
    let yaw: f64 = get("yaw")?.parse().ok()?;
    let pitch: f64 = get("pitch")?.parse().ok()?;
    let fov: f64 = get("fov")?.parse().ok()?;
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let blocks = [yt::A, yt::TD, yt::D1, yt::D2, yt::D3, yt::SP, yt::RB, yt::O, yt::X, yt::O2, yt::L];
    let mut placed: Vec<(i64, i64)> = Vec::new();
    for (i, d) in blocks.iter().enumerate() {
        let c = add(t, *d);
        let dust = matches!(i, 2..=4);
        let pt = [c[0] as f64 + 0.5, c[1] as f64 + if dust { 0.1 } else { 0.5 }, c[2] as f64 + 0.5];
        if let Some((sx, sy)) = project([eye[0], eye[1], eye[2]], yaw, pitch, fov, w as f64, h as f64, pt) {
            let label = format!("{}", i + 1);
            let lw = font.width(&label, 4) as i64;
            let (mut x, y) = (sx as i64, sy as i64);
            // Two blocks on one line of sight: put the later label beside the earlier one.
            while placed.iter().any(|(px, py)| (px - x).abs() < 40 && (py - y).abs() < 40) {
                x += 44;
            }
            placed.push((x, y));
            img.fill_rect(x - lw / 2 - 8, y - 22, lw + 16, 44, [0, 0, 0, 190]);
            img.fill_rect(x - lw / 2 - 8, y - 22, lw + 16, 3, [255, 222, 40, 255]);
            outlined_centered(font, &mut img, x, y - 16, &label, 4, YELLOW);
        }
    }
    Some(img)
}

// ---- audio -----------------------------------------------------------------------------------

struct Wav {
    path: PathBuf,
    data_off: u64,
    byte_rate: f64,
    block: u64,
    fmt: String,
    channels: u32,
    rate: u32,
    /// Wall time of the first data byte (median over the segments' size samples).
    w0: f64,
}

fn read_wav(path: &Path, samples: &[(f64, u64)]) -> Result<Wav> {
    use std::io::Read;
    let mut head = vec![0u8; 256];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    head.truncate(n);
    if &head[..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        bail!("{} is not a WAV file", path.display());
    }
    let (mut i, mut fmt_tag, mut channels, mut rate, mut bits, mut block, mut data_off) = (12usize, 0u16, 0u32, 0u32, 0u16, 0u64, 0u64);
    while i + 8 <= head.len() {
        let id = &head[i..i + 4];
        let size = u32::from_le_bytes(head[i + 4..i + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            fmt_tag = u16::from_le_bytes(head[i + 8..i + 10].try_into().unwrap());
            channels = u16::from_le_bytes(head[i + 10..i + 12].try_into().unwrap()) as u32;
            rate = u32::from_le_bytes(head[i + 12..i + 16].try_into().unwrap());
            block = u16::from_le_bytes(head[i + 20..i + 22].try_into().unwrap()) as u64;
            bits = u16::from_le_bytes(head[i + 22..i + 24].try_into().unwrap());
            if fmt_tag == 0xFFFE && size >= 40 {
                fmt_tag = u16::from_le_bytes(head[i + 32..i + 34].try_into().unwrap());
            }
        }
        if id == b"data" {
            data_off = (i + 8) as u64;
            break;
        }
        i += 8 + size;
    }
    if data_off == 0 || channels == 0 {
        bail!("{}: no fmt/data chunk", path.display());
    }
    let fmt = match (fmt_tag, bits) {
        (3, 32) => "f32le",
        (1, 16) => "s16le",
        (1, 8) => "u8",
        (1, 32) => "s32le",
        other => bail!("{}: unsupported sample format {other:?}", path.display()),
    };
    let byte_rate = rate as f64 * block as f64;
    let mut w0s: Vec<f64> = samples.iter().filter(|(_, b)| *b > data_off).map(|(t, b)| t - (*b - data_off) as f64 / byte_rate).collect();
    w0s.sort_by(|a, b| a.total_cmp(b));
    let w0 = *w0s.get(w0s.len() / 2).context("no audio size samples")?;
    Ok(Wav { path: path.to_path_buf(), data_off, byte_rate, block, fmt: fmt.to_string(), channels, rate, w0 })
}

/// The game audio between two wall times, as a raw file for ffmpeg (None if outside the file).
fn audio_span(wav: &Wav, a: f64, b: f64, out: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let len = std::fs::metadata(&wav.path)?.len();
    // OpenAL Soft mixes ahead of time: the sound of an instant lands a little later in the file
    // (`--phase audiosync`: the presses' sounds peak within -50..+10 ms with this lead).
    let lead = 0.03;
    let off = |t: f64| -> u64 {
        let bytes = ((t + lead - wav.w0) * wav.byte_rate).max(0.0) as u64;
        wav.data_off + bytes / wav.block * wav.block
    };
    let (s, e) = (off(a), off(b).min(len));
    if e <= s + wav.block * 100 {
        return Ok(false);
    }
    let mut f = std::fs::File::open(&wav.path)?;
    f.seek(SeekFrom::Start(s))?;
    let mut buf = vec![0u8; (e - s) as usize];
    f.read_exact(&mut buf)?;
    std::fs::write(out, buf)?;
    Ok(true)
}

// ---- clips -----------------------------------------------------------------------------------

#[derive(Clone)]
struct Ov {
    t0: f64,
    t1: f64,
    png: PathBuf,
    /// An ffmpeg enable expression instead of the t0..t1 window (the rhythm bar).
    expr: Option<String>,
}

#[derive(Clone)]
enum Src {
    Seg(usize),
    /// A card: its png over a darkened, blurred segment (index, at video time).
    Card(PathBuf, usize, f64),
}

#[derive(Clone)]
struct Clip {
    src: Src,
    t_in: f64,
    t_out: f64,
    speed: f64,
    ovs: Vec<Ov>,
    audio: bool,
    chapter: Option<String>,
}

struct Cut {
    font: Font,
    tl: Timeline,
    out: PathBuf,
    work: PathBuf,
    w: u32,
    h: u32,
    wavs: Vec<(String, Wav)>,
    n_png: usize,
}

impl Cut {
    fn new(version: &str, o: &YtOptions, w: u32, h: u32, work: &str) -> Result<Cut> {
        let tl: Timeline = serde_json::from_str(&std::fs::read_to_string(o.out.join("timeline.json"))?)?;
        let work = o.out.join(work);
        std::fs::create_dir_all(work.join("png"))?;
        std::fs::create_dir_all(work.join("clips"))?;
        let mut wavs = Vec::new();
        let mut names: Vec<String> = tl.segments.iter().map(|s| s.wav.clone()).filter(|n| !n.is_empty()).collect();
        names.dedup();
        for n in names {
            let samples: Vec<(f64, u64)> = tl.segments.iter().filter(|s| s.wav == n).flat_map(|s| [(s.wav_t_start, s.wav_start), (s.end, s.wav_end)]).filter(|(t, _)| *t > 0.0).collect();
            match read_wav(&o.out.join("audio").join(&n), &samples) {
                Ok(wv) => wavs.push((n, wv)),
                Err(e) => eprintln!("compose: no audio from {n}: {e:#}"),
            }
        }
        Ok(Cut { font: Font::minecraft(version)?, tl, out: o.out.clone(), work, w, h, wavs, n_png: 0 })
    }

    fn seg(&self, name: &str) -> Result<usize> {
        self.tl.segments.iter().position(|s| s.name == name).with_context(|| format!("no segment {name}"))
    }

    fn segs_with(&self, prefix: &str) -> Vec<usize> {
        self.tl.segments.iter().enumerate().filter(|(_, s)| s.name.starts_with(prefix)).map(|(i, _)| i).collect()
    }

    fn s(&self, i: usize) -> &Seg {
        &self.tl.segments[i]
    }

    fn dur(&self, i: usize) -> f64 {
        let s = self.s(i);
        (s.end - s.start).max(0.1)
    }

    /// Video time (within segment i) of the first event containing `needle` (after `after`).
    fn ev(&self, i: usize, needle: &str) -> Option<f64> {
        let s = self.s(i);
        s.events.iter().find(|(_, e)| e.contains(needle)).map(|(t, _)| t - s.start)
    }

    fn evs(&self, i: usize, needle: &str) -> Vec<(f64, String)> {
        let s = self.s(i);
        s.events.iter().filter(|(_, e)| e.contains(needle)).map(|(t, e)| (t - s.start, e.clone())).collect()
    }

    fn png(&mut self, img: &Image) -> Result<PathBuf> {
        self.n_png += 1;
        let p = self.work.join("png").join(format!("o{:04}.png", self.n_png));
        img.save_png(&p)?;
        Ok(p)
    }

    fn cap(&mut self, t0: f64, t1: f64, text: &str) -> Result<Ov> {
        let img = caption(&self.font, self.w, self.h, text, YELLOW);
        Ok(Ov { t0, t1, png: self.png(&img)?, expr: None })
    }

    fn cap_c(&mut self, t0: f64, t1: f64, text: &str, color: [u8; 3]) -> Result<Ov> {
        let img = caption(&self.font, self.w, self.h, text, color);
        Ok(Ov { t0, t1, png: self.png(&img)?, expr: None })
    }

    fn img(&mut self, t0: f64, t1: f64, img: &Image) -> Result<Ov> {
        Ok(Ov { t0, t1, png: self.png(img)?, expr: None })
    }

    /// The rhythm bar for a clip of segment `i` cut at `t_in` with `speed`, beats anchored at the
    /// wall time `cue` (an armed tick seen by the client).
    fn beats(&mut self, i: usize, t_in: f64, speed: f64, cue: f64) -> Result<Vec<Ov>> {
        let off = self.s(i).start + t_in - cue;
        let mut v = Vec::new();
        for ph in 0..6 {
            let img = beat_bar(&self.font, self.w, self.h, ph);
            let png = self.png(&img)?;
            // Wall time of output time t: off + t * speed (relative to the cue).
            let expr = format!("eq(mod(floor((({off:.4})+t*{speed:.4})/0.05+600000),6),{ph})");
            v.push(Ov { t0: 0.0, t1: 0.0, png, expr: Some(expr) });
        }
        Ok(v)
    }

    fn wav_for(&self, i: usize) -> Option<&Wav> {
        let n = &self.s(i).wav;
        self.wavs.iter().find(|(k, _)| k == n).map(|(_, w)| w)
    }

    /// Render one clip to `clips/NNN.mkv` (h264 + pcm, constant frame rate).
    fn render(&self, k: usize, c: &Clip) -> Result<(PathBuf, f64)> {
        let out = self.work.join("clips").join(format!("{k:03}.mkv"));
        let (w, h) = (self.w, self.h);
        let mut args: Vec<String> = Vec::new();
        let secs;
        let mut graph = Vec::new();
        let seg_i = match &c.src {
            Src::Seg(i) => *i,
            Src::Card(_, i, _) => *i,
        };
        let seg_path = self.out.join("segments").join(&self.s(seg_i).file);
        match &c.src {
            Src::Seg(_) => {
                secs = (c.t_out - c.t_in) / c.speed;
                args.extend(["-ss".into(), format!("{:.3}", c.t_in), "-t".into(), format!("{:.3}", c.t_out - c.t_in), "-i".into(), seg_path.display().to_string()]);
                graph.push(format!("[0:v]setpts=(PTS-STARTPTS)/{:.4},fps={FPS},scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},eq=gamma=1.12:saturation=1.2:contrast=1.04,setsar=1[v0]", c.speed));
            }
            Src::Card(png, _, at) => {
                secs = c.t_out - c.t_in;
                // The background: one frame of the segment, blurred and darkened, as a still.
                let bg = self.work.join("clips").join(format!("{k:03}-bg.png"));
                ff(&["-ss".into(), format!("{at:.3}"), "-i".into(), seg_path.display().to_string(), "-frames:v".into(), "1".into(),
                    "-vf".into(), format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},boxblur=12:2,eq=brightness=-0.18"), bg.display().to_string()])?;
                args.extend(["-loop".into(), "1".into(), "-framerate".into(), FPS.to_string(), "-t".into(), format!("{secs:.3}"), "-i".into(), bg.display().to_string()]);
                args.extend(["-loop".into(), "1".into(), "-framerate".into(), FPS.to_string(), "-t".into(), format!("{secs:.3}"), "-i".into(), png.display().to_string()]);
                graph.push(format!(
                    "[0:v]setsar=1,format=rgba[bg];[bg][1:v]overlay=0:0:shortest=1,fade=t=in:st=0:d=0.25,fade=t=out:st={:.3}:d=0.25[v0]",
                    (secs - 0.25).max(0.0)
                ));
            }
        }
        let base = if matches!(c.src, Src::Card(..)) { 2 } else { 1 };
        let mut last = "v0".to_string();
        for (j, ov) in c.ovs.iter().enumerate() {
            // A still overlay: 2 frames a second are plenty (the overlay filter repeats the last
            // one), and 15x fewer PNG decodes than at the video's rate.
            args.extend(["-loop".into(), "1".into(), "-framerate".into(), "2".into(), "-t".into(), format!("{:.3}", secs + 1.0), "-i".into(), ov.png.display().to_string()]);
            let en = match &ov.expr {
                Some(e) => e.clone(),
                None => format!("between(t,{:.3},{:.3})", ov.t0, ov.t1),
            };
            let next = format!("v{}", j + 1);
            graph.push(format!("[{last}][{}:v]overlay=0:0:enable='{en}':shortest=1[{next}]", base + j));
            last = next;
        }
        graph.push(format!("[{last}]format=yuv420p,trim=duration={secs:.3}[vout]"));
        // Audio: the game's, for 1x-2x clips; silence otherwise.
        let a_idx = base + c.ovs.len();
        let mut have_audio = false;
        if c.audio && matches!(c.src, Src::Seg(_)) && c.speed <= 2.0 {
            if let Some(wav) = self.wav_for(seg_i) {
                let s = self.s(seg_i);
                let raw = self.work.join("clips").join(format!("{k:03}.raw"));
                if audio_span(wav, s.start + c.t_in, s.start + c.t_out, &raw)? {
                    args.extend(["-f".into(), wav.fmt.clone(), "-ar".into(), wav.rate.to_string(), "-ac".into(), wav.channels.to_string(), "-i".into(), raw.display().to_string()]);
                    let tempo = if (c.speed - 1.0).abs() < 1e-3 { "anull".to_string() } else { format!("atempo={:.4}", c.speed) };
                    graph.push(format!("[{a_idx}:a]aresample=48000,aformat=channel_layouts=stereo,{tempo},apad,atrim=duration={secs:.3}[aout]"));
                    have_audio = true;
                }
            }
        }
        if !have_audio {
            args.extend(["-f".into(), "lavfi".into(), "-t".into(), format!("{secs:.3}"), "-i".into(), "anullsrc=r=48000:cl=stereo".into()]);
            graph.push(format!("[{a_idx}:a]atrim=duration={secs:.3}[aout]"));
        }
        args.extend(["-filter_complex".into(), graph.join(";"), "-map".into(), "[vout]".into(), "-map".into(), "[aout]".into()]);
        args.extend(["-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "14".into(), "-r".into(), FPS.to_string()]);
        args.extend(["-c:a".into(), "pcm_s16le".into(), "-ar".into(), "48000".into(), out.display().to_string()]);
        ff(&args)?;
        Ok((out, secs))
    }

    /// Render all clips, join them, encode the WebM; returns chapter times.
    fn finish(&self, clips: &[Clip], file: &str) -> Result<(PathBuf, Vec<(f64, String)>, f64)> {
        let mut list = String::new();
        let mut chapters = Vec::new();
        let mut t = 0.0;
        for (k, c) in clips.iter().enumerate() {
            if let Some(ch) = &c.chapter {
                chapters.push((t, ch.clone()));
            }
            let (p, _) = self.render(k, c).with_context(|| format!("clip {k}"))?;
            let d = probe_duration(&p)?;
            t += d;
            list.push_str(&format!("file '{}'\n", p.display()));
        }
        let lst = self.work.join("clips.txt");
        std::fs::write(&lst, list)?;
        let joined = self.work.join("joined.mkv");
        ff(&["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), lst.display().to_string(), "-c".into(), "copy".into(), joined.display().to_string()])?;
        let out = self.out.join(file);
        ff(&[
            "-i".into(), joined.display().to_string(),
            "-vf".into(), "scale=out_range=tv,format=yuv420p".into(),
            "-c:v".into(), "libvpx-vp9".into(), "-b:v".into(), "0".into(), "-crf".into(), "28".into(), "-row-mt".into(), "1".into(),
            "-deadline".into(), "good".into(), "-cpu-used".into(), "3".into(), "-color_range".into(), "tv".into(),
            "-af".into(), "loudnorm=I=-16:TP=-1.5:LRA=11".into(),
            "-c:a".into(), "libopus".into(), "-b:a".into(), "128k".into(), "-ar".into(), "48000".into(),
            out.display().to_string(),
        ])?;
        let d = probe_duration(&out)?;
        Ok((out, chapters, d))
    }

    /// Stills every `step` seconds (phone size) + one per chapter, for review.
    fn stills(&self, video: &Path, dir: &str, step: f64, total: f64, scale: &str) -> Result<()> {
        let d = self.out.join(dir);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d)?;
        let n = (total / step).ceil() as usize;
        for i in 0..n {
            let t = i as f64 * step + step / 2.0;
            ff(&["-ss".into(), format!("{t:.2}"), "-i".into(), video.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), format!("scale={scale}"), "-q:v".into(), "4".into(), d.join(format!("s{i:03}-{t:06.1}.jpg")).display().to_string()])?;
        }
        Ok(())
    }
}

fn seg_clip(i: usize, t_in: f64, t_out: f64, speed: f64) -> Clip {
    Clip { src: Src::Seg(i), t_in, t_out, speed, ovs: Vec::new(), audio: true, chapter: None }
}

/// The hit try segment, its press time (video) and the wall time of its armed tick (the arm's
/// rise seen by the client + 2 ticks); the first miss try (with its repair) if any.
fn tries(c: &Cut) -> Result<(usize, f64, f64, Option<(usize, f64, f64)>)> {
    let mut hit = None;
    let mut miss = None;
    for i in c.segs_with("try-") {
        let s = c.s(i);
        let Some((_, e)) = s.events.iter().find(|(_, e)| e.starts_with("press try")) else { continue };
        let num = |key: &str| -> Option<f64> { e.split(key).nth(1)?.split_whitespace().next()?.parse::<f64>().ok() };
        let pressed = num("pressed at ").map(|ms| ms / 1000.0 - s.start).or_else(|| c.ev(i, "press try")).unwrap_or(0.0);
        let armed = num("rise at ").map(|ms| ms / 1000.0 + 0.10).unwrap_or(s.start + pressed);
        if c.ev(i, "HIT").is_some() {
            hit = Some((i, pressed, armed));
        } else if miss.is_none() && c.ev(i, "repair").is_some() && !e.contains("offset -") {
            miss = Some((i, pressed, c.ev(i, "repair").unwrap()));
        }
    }
    let (i, p, cue) = hit.context("no hit in the tries")?;
    Ok((i, p, cue, miss))
}

fn steps() -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static str)> {
    // (segment, badge, title, subtitle, tip)
    vec![
        ("b01-piston", "1/11", "PISTON", "facing WEST, on the target bedrock", "Stand 3 blocks west of the target and look east"),
        ("b02-trapdoor", "2/11", "OAK TRAPDOOR", "bottom half, above where you will lie", "Temporary block, trapdoor on top, break the block"),
        ("b03-dust", "3-5/11", "REDSTONE DUST x3", "diagonal to the piston, then 2 more", "The first dust nudges the piston, it never powers it"),
        ("b06-sticky", "6/11", "STICKY PISTON", "facing SOUTH", "Place it on a temporary block, looking north"),
        ("b07-rblock", "7/11", "REDSTONE BLOCK", "on the sticky piston's face", "It sits on the second dust"),
        ("b08-observer", "8/11", "OBSERVER", "facing EAST, on the first dust", "Sneak to place it on the dust"),
        ("b09-stone", "9/11", "STONE", "on the observer's back", "Diagonally above the piston: it powers it (QC)"),
        ("b10-observer2", "10/11", "OBSERVER", "facing SOUTH, on the redstone block", "Stand on a block to reach the top"),
        ("b11-lamp", "11/11", "REDSTONE LAMP", "LAST: it starts the clock", "The machine now arms the piston every 0.3 s"),
    ]
}

pub fn compose(version: &str, plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let mut c = Cut::new(version, o, o.width, o.height, "cut")?;
    let t1 = plan.t1;
    let mut clips: Vec<Clip> = Vec::new();
    let (hit_i, hit_p, hit_cue, miss) = tries(&c)?;
    // ---- HOOK ----------------------------------------------------------------------------
    let h1 = big(&c.font, c.w, c.h, 40, &[("11 BLOCKS.", 11, YELLOW)]);
    let h2 = big(&c.font, c.w, c.h, 160, &[("NO LAG. NO TNT.", 7, WHITE)]);
    let h3 = big(&c.font, c.w, c.h, 560, &[("BREAKS NETHER ROOF BEDROCK", 5, WHITE)]);
    let (ov1, ov2, ov3) = (c.img(0.0, 99.0, &h1)?, c.img(0.0, 99.0, &h2)?, c.img(0.0, 99.0, &h3)?);
    // The machine running (lamp blinking, the arm pumping, the beacon beam behind it).
    let o2 = c.seg("overview-2")?;
    let mut k = seg_clip(o2, 0.4, 2.2, 1.0);
    k.ovs = vec![ov1.clone()];
    k.chapter = Some("The result".into());
    clips.push(k);
    let hs_i = c.seg("hole-shot")?;
    let mut k = seg_clip(hs_i, 0.4, 2.2, 1.0);
    k.ovs = vec![ov1.clone(), ov2.clone(), c.img(0.0, 99.0, &big(&c.font, c.w, c.h, 470, &[("A HOLE IN THE NETHER ROOF", 6, GREEN)]))?];
    clips.push(k);
    let dr = c.seg("drop")?;
    let fall = c.ev(dr, "falling").unwrap_or(c.dur(dr) - 8.0);
    let mut k = seg_clip(dr, (fall - 1.0).max(0.0), (fall + 5.0).min(c.dur(dr)), 2.0);
    k.ovs = vec![ov1, ov2, ov3];
    clips.push(k);
    // A title over the finished machine.
    let ov_i = c.seg("overview-1")?;
    let mut k = seg_clip(ov_i, 0.3, 2.4, 1.0);
    k.ovs = vec![c.img(0.0, 99.0, &big(&c.font, c.w, c.h, 470, &[("SURVIVAL BEDROCK BREAKER", 6, YELLOW), ("how to build it, step by step", 4, WHITE)]))?];
    clips.push(k);
    // ---- WHAT YOU NEED ---------------------------------------------------------------------
    let tex = |n: &str| mctas::mcfont::texture(version, n).ok();
    let mut icons = Vec::new();
    for (i, n) in ["item/netherite_pickaxe", "mob_effect/haste", "block/piston_side", "block/oak_trapdoor", "item/redstone", "block/piston_top_sticky", "block/redstone_lamp", "block/netherrack"].iter().enumerate() {
        if let Some(t) = tex(n) {
            icons.push((t, i));
        }
    }
    let kit = card(&c.font, c.w, c.h, "WHAT YOU NEED", &[
        ("Netherite pickaxe, Efficiency V (diamond works too)", WHITE),
        ("Haste II (beacon: see below)", YELLOW),
        ("Pistons: 1 to build + a stack in your OFFHAND", WHITE),
        ("1 oak trapdoor, 1 stone", WHITE),
        ("3 redstone dust, 1 redstone block", WHITE),
        ("1 sticky piston, 2 observers", WHITE),
        ("1 redstone lamp", WHITE),
        ("A few netherrack (temporary, mined again)", GRAY),
    ], &icons);
    let kit_i = c.seg("kit-hotbar")?;
    clips.push(Clip { src: Src::Card(c.png(&kit)?, kit_i, 0.5), t_in: 0.0, t_out: 6.5, speed: 1.0, ovs: vec![], audio: false, chapter: Some("What you need".into()) });
    let mut k = seg_clip(kit_i, 0.2, c.dur(kit_i) - 0.2, 1.4);
    k.ovs = vec![c.cap(0.0, 99.0, "Your hotbar (kit given by command for this video)")?];
    clips.push(k);
    let math = card(&c.font, c.w, c.h, "HASTE II IS REQUIRED", &[
        ("A piston breaks INSTANTLY at mining speed 45+", WHITE),
        ("Netherite + Efficiency V: 35", WHITE),
        ("x 1.4 with Haste II = 49  -> instant", GREEN),
        ("Diamond: 34 x 1.4 = 47.6  -> instant", GREEN),
        ("Haste I: x 1.2 = 42  -> NOT instant", RED),
        ("No instant break = the swap can't fit in one tick", GRAY),
    ], &[]);
    clips.push(Clip { src: Src::Card(c.png(&math)?, kit_i, 1.0), t_in: 0.0, t_out: 6.5, speed: 1.0, ovs: vec![], audio: false, chapter: None });
    let bw = c.seg("beacon-walk")?;
    let mut k = seg_clip(bw, 0.2, c.dur(bw), 2.5);
    k.ovs = vec![c.cap(0.0, 99.0, "Haste II = a beacon on a FULL 4-level pyramid (164 blocks)\n(pyramid placed with /fill for this video)")?];
    clips.push(k);
    let bp = c.seg("beacon-place")?;
    let placed = c.ev(bp, "placed beacon").unwrap_or(1.0);
    let mut k = seg_clip(bp, (placed - 0.8).max(0.0), placed + 0.8, 1.0);
    k.ovs = vec![c.cap(0.0, 99.0, "The beacon on top")?];
    clips.push(k);
    let bg = c.seg("beacon-gui")?;
    let mut k = seg_clip(bg, 0.3, c.dur(bg) - 0.5, 1.3);
    k.ovs = vec![c.cap(0.0, 99.0, "Primary power: Haste. Then the II upgrade. Confirm.")?];
    clips.push(k);
    let bb = c.seg("beacon-beam")?;
    let mut k = seg_clip(bb, 0.0, 0.9, 0.6);
    k.ovs = vec![c.cap(0.0, 99.0, "Works ON the roof: the beam sees the sky. Range 50 blocks.")?];
    clips.push(k);
    // ---- THE BUILD -------------------------------------------------------------------------
    // (No labels on this first view: it looks down the machine's diagonal, labels would pile up.)
    let mut k = seg_clip(ov_i, 0.3, c.dur(ov_i) - 0.2, 1.0);
    k.ovs.push(c.cap(0.0, 99.0, "The whole machine: 11 blocks on top of the roof")?);
    k.chapter = Some("The build: 11 blocks".into());
    clips.push(k);
    for (name, n, title, sub, tip) in steps() {
        let i = c.seg(name)?;
        let evs = c.evs(i, "placed");
        let mined = c.evs(i, "mined");
        let last = evs.iter().chain(mined.iter()).map(|(t, _)| *t).fold(0.0f64, f64::max);
        let end = (last + 1.0).min(c.dur(i));
        let speed = if evs.len() + mined.len() > 2 { 1.5 } else { 1.2 };
        let mut k = seg_clip(i, 0.2, end, speed);
        let b = badge(&c.font, c.w, c.h, n, title, sub);
        k.ovs.push(c.img(0.0, 99.0, &b)?);
        k.ovs.push(c.cap_c(0.0, 99.0, tip, WHITE)?);
        clips.push(k);
    }
    for name in ["overview-2", "overview-3"] {
        let i = c.seg(name)?;
        let pose = c.s(i).events.iter().find(|(_, e)| e.starts_with("pose")).map(|(_, e)| e.clone());
        let mut k = seg_clip(i, 0.3, c.dur(i) - 0.3, 1.0);
        if let Some(l) = pose.as_deref().and_then(|p| labels(&c.font, c.w, c.h, p, t1)) {
            k.ovs.push(c.img(0.0, 99.0, &l)?);
        }
        let text = if name == "overview-2" { "Done. The lamp blinks: the clock runs." } else { "It arms the swap piston (1) every 6 ticks = 0.3 s" };
        k.ovs.push(c.cap(0.0, 99.0, text)?);
        clips.push(k);
    }
    // ---- ONE KEY: ATTACK + USE -------------------------------------------------------------
    let kb = c.seg("keybinds")?;
    let sa = c.ev(kb, "select attack").unwrap_or(3.0);
    let su = c.ev(kb, "select use").unwrap_or(sa + 2.0);
    let ur = c.ev(kb, "use = R").unwrap_or(su + 1.0);
    let mut k = seg_clip(kb, 0.4, (ur + 3.0).min(c.dur(kb) - 0.2), 1.25);
    let s = 1.25;
    k.ovs.push(c.cap(0.0, (sa - 0.4) / s, "Options > Controls > Key Binds")?);
    k.ovs.push(c.cap((sa - 0.4) / s, (su - 0.4) / s, "Attack/Destroy: set it to R")?);
    k.ovs.push(c.cap((su - 0.4) / s, (ur - 0.4) / s, "Use Item/Place Block: ALSO R")?);
    k.ovs.push(c.cap_c((ur - 0.4) / s, 99.0, "Both get flagged as a conflict (yellow bars). Ignore it:\none press of R now does BOTH, in the same tick", GREEN)?);
    k.chapter = Some("One key = Attack + Use".into());
    clips.push(k);
    let oh = c.seg("offhand")?;
    let mut k = seg_clip(oh, 0.2, c.dur(oh) - 0.2, 1.0);
    k.ovs.push(c.cap(0.0, 99.0, "Pistons in your OFFHAND (select them, press F)")?);
    clips.push(k);
    // ---- HOW TO PRESS ------------------------------------------------------------------------
    let ld = c.seg("liedown")?;
    let crawl = c.ev(ld, "crawling").unwrap_or(4.0);
    let mut k = seg_clip(ld, 0.2, (crawl + 1.6).min(c.dur(ld)), 1.3);
    k.ovs.push(c.cap(0.0, (crawl - 0.2) / 1.3, "Open the trapdoor, step in, close it...")?);
    k.ovs.push(c.cap((crawl - 0.2) / 1.3, 99.0, "...you are crawling. Look up at the piston's side.")?);
    k.chapter = Some("How to press".into());
    clips.push(k);
    let n_hit = c.segs_with("try-").iter().position(|&i| i == hit_i).map(|p| p + 1).unwrap_or(1);
    let t_in = (hit_p - 2.4).max(0.0);
    let mut k = seg_clip(hit_i, t_in, hit_p + 1.6, 1.0);
    k.ovs.extend(c.beats(hit_i, t_in, 1.0, hit_cue)?);
    k.ovs.push(c.cap(0.0, 99.0, "Press R in rhythm: just after the arm pops out (GO)")?);
    clips.push(k);
    let t_in = (hit_p - 0.5).max(0.0);
    let mut k = seg_clip(hit_i, t_in, hit_p + 0.6, 0.25);
    k.ovs.extend(c.beats(hit_i, t_in, 0.25, hit_cue)?);
    k.ovs.push(c.cap(0.0, 99.0, "Slow motion: break + place in the same tick.\nA piston facing DOWN goes in. The bedrock under it is gone.")?);
    k.audio = false;
    clips.push(k);
    let hs = c.seg("hole-shot")?;
    let mut k = seg_clip(hs, 0.3, c.dur(hs) - 0.3, 1.0);
    k.ovs.push(c.cap_c(0.0, 99.0, &format!("The result (this was press #{n_hit}: the first {} missed)", n_hit - 1), WHITE)?);
    clips.push(k);
    let mut k = seg_clip(hit_i, hit_p + 0.6, (hit_p + 1.6).min(c.dur(hit_i)), 0.5);
    k.ovs.push(c.cap_c(0.0, 99.0, "1 armed tick every 6: a random press wins 1 time in 6.\nIn rhythm with the arm, you do better.", WHITE)?);
    k.audio = false;
    clips.push(k);
    // ---- WHAT IF YOU MISS --------------------------------------------------------------------
    if let Some((mi, mp, rp)) = miss {
        let mut k = seg_clip(mi, (mp - 1.0).max(0.0), rp - 0.5, 1.0);
        k.ovs.push(c.cap(0.0, 99.0, "Missed? Nothing breaks, but the piston now faces DOWN")?);
        k.chapter = Some("What if you miss".into());
        clips.push(k);
        let mut k = seg_clip(mi, rp - 0.5, (rp + 1.8).min(c.dur(mi)), 1.0);
        k.ovs.push(c.cap(0.0, 99.0, "Same spot: look right (east), press R once.\nThe piston faces west again.")?);
        clips.push(k);
        let mut k = seg_clip(mi, (rp + 1.8).min(c.dur(mi) - 0.5), c.dur(mi), 1.0);
        k.ovs.push(c.cap(0.0, 99.0, "It skips 2 beats (0.7 s), then it is back: press again")?);
        clips.push(k);
    }
    // ---- WHY IT WORKS ------------------------------------------------------------------------
    let diag = diagram_swap(&c.font, version, c.w, c.h)?;
    let ov_bg = c.seg("overview-1")?;
    clips.push(Clip { src: Src::Card(c.png(&diag)?, ov_bg, 1.0), t_in: 0.0, t_out: 7.0, speed: 1.0, ovs: vec![], audio: false, chapter: Some("Why it works".into()) });
    let sc = c.seg("slowmo-cycle")?;
    let steps_ev = c.evs(sc, "step ");
    if let (Some(first), Some(lastst)) = (steps_ev.first(), steps_ev.last()) {
        let t_in = (first.0 - 1.0).max(0.0);
        let mut k = seg_clip(sc, t_in, lastst.0 + 0.8, 1.0);
        for (j, (t, e)) in steps_ev.iter().enumerate() {
            let t0 = t - t_in;
            let t1v = steps_ev.get(j + 1).map(|(n, _)| n - t_in).unwrap_or(t0 + 1.2);
            let label = format!("tick {}", j + 1);
            let head = e.contains("head=true");
            let moving = e.contains("moving=true");
            let ext = e.contains("ext=true");
            let text = if head {
                "ARMED: the arm landed, its retract is QUEUED for the next tick"
            } else if moving && ext {
                "The machine powers the piston: it pushes its arm out"
            } else if moving {
                "The arm pulls back in (the retract)"
            } else {
                "Waiting for the clock (redstone block + observers)"
            };
            let color = if head { GREEN } else { WHITE };
            let mut img = caption(&c.font, c.w, c.h, text, color);
            outlined(&c.font, &mut img, 30, 24, &label, 6, if head { GREEN } else { YELLOW });
            outlined(&c.font, &mut img, 30, 90, "game frozen with /tick, stepped 1 tick at a time", 3, GRAY);
            k.ovs.push(c.img(t0, t1v, &img)?);
        }
        clips.push(k);
    }
    let ml = c.seg("m2-liedown")?;
    let mut k = seg_clip(ml, 0.2, c.dur(ml) - 0.2, 3.0);
    k.ovs.push(c.cap(0.0, 99.0, "Same machine, second copy: now the press, frozen")?);
    clips.push(k);
    let sp = c.seg("slowmo-press")?;
    let armed = c.ev(sp, "ARMED").unwrap_or(2.0);
    let press = c.ev(sp, "press (frozen)").unwrap_or(armed + 1.2);
    let next = c.ev(sp, "next tick").unwrap_or(press + 1.5);
    let mut k = seg_clip(sp, (armed - 1.5).max(0.0), (next + 1.4).min(c.dur(sp)), 1.0);
    let base = (armed - 1.5).max(0.0);
    k.ovs.push(c.cap(0.0, armed - base, "Frozen, stepping to the armed tick...")?);
    k.ovs.push(c.cap_c(armed - base, press - base, "ARMED: the retract is queued", GREEN)?);
    k.ovs.push(c.cap(press - base, next - base, "Press: the piston is replaced by one facing DOWN\n(the queued retract stays queued)")?);
    k.ovs.push(c.cap_c(next - base, 99.0, "Next tick: the retract runs on the NEW piston:\nit pulls the block below it. The bedrock is deleted.", GREEN)?);
    k.audio = false;
    clips.push(k);
    let sr = c.seg("slowmo-reveal")?;
    let pb2 = c.ev(sr, "piston broken").unwrap_or(1.0);
    let mut k = seg_clip(sr, (pb2 - 0.3).max(0.0), c.dur(sr) - 0.3, 1.5);
    k.ovs.push(c.cap_c(0.0, 99.0, "Gone. No lag, no TNT, no timing tricks with the server.", GREEN)?);
    clips.push(k);
    // ---- THE PAYOFF ------------------------------------------------------------------------
    let dk = c.seg("drink")?;
    let mut k = seg_clip(dk, 0.2, c.dur(dk) - 0.2, 2.0);
    k.ovs.push(c.cap(0.0, 99.0, "Now jump in. A Slow Falling potion for the trip down (kit)")?);
    k.chapter = Some("Through the roof".into());
    clips.push(k);
    let mut k = seg_clip(dr, 0.3, c.dur(dr) - 0.3, 2.0);
    k.ovs.push(c.img(0.0, 3.0, &big(&c.font, c.w, c.h, 60, &[("THROUGH THE ROOF", 8, YELLOW)]))?);
    clips.push(k);
    let end = card(&c.font, c.w, c.h, "11 BLOCKS. NO LAG.", &[
        ("Java 26.3, survival, the official server", WHITE),
        ("1 piston, 1 trapdoor, 1 stone, 2 observers,", WHITE),
        ("3 dust, 1 sticky piston, 1 redstone block, 1 lamp", WHITE),
        ("One key = Attack + Use, pistons in the offhand", YELLOW),
        ("Random press: 1 in 6. In rhythm: better.", GRAY),
        ("github.com/vjeux/redstonepedia", AQUA),
        ("", WHITE),
        ("Subscribe for more redstone!", GREEN),
    ], &[]);
    clips.push(Clip { src: Src::Card(c.png(&end)?, hs, 1.0), t_in: 0.0, t_out: 6.0, speed: 1.0, ovs: vec![], audio: false, chapter: Some("Recap".into()) });
    let (out, chapters, d) = c.finish(&clips, "bedrock-roof-11-blocks.webm")?;
    c.stills(&out, "stills", 2.0, d, "640:360")?;
    println!("composed {} ({d:.1}s)", out.display());
    let mut desc = String::new();
    for (t, ch) in &chapters {
        desc.push_str(&format!("{}:{:02} {ch}\n", (*t as u64) / 60, (*t as u64) % 60));
    }
    std::fs::write(o.out.join("chapters.txt"), &desc)?;
    print!("{desc}");
    Ok(())
}

/// An image rotated by a quarter turn clockwise, `n` times.
fn rot90(img: &Image, n: u32) -> Image {
    let mut cur = img.clone();
    for _ in 0..n % 4 {
        let mut out = Image::new(cur.h, cur.w, [0, 0, 0, 0]);
        for y in 0..cur.h {
            for x in 0..cur.w {
                let c = cur.get(x, y);
                let (nx, ny) = (cur.h - 1 - y, x);
                let i = ((ny * out.w + nx) * 4) as usize;
                out.px[i..i + 4].copy_from_slice(&c);
            }
        }
        cur = out;
    }
    cur
}

/// The trick in three side views (x across, y up): the armed piston with its retract queued, the
/// swap to a piston facing down, the queued retract pulling the block under it.
fn diagram_swap(font: &Font, version: &str, w: u32, h: u32) -> Result<Image> {
    let side = mctas::mcfont::texture(version, "block/piston_side")?;
    let top = mctas::mcfont::texture(version, "block/piston_top")?;
    let bedrock = mctas::mcfont::texture(version, "block/bedrock")?;
    let rack = mctas::mcfont::texture(version, "block/netherrack")?;
    let mut img = Image::new(w, h, [0, 0, 0, 150]);
    outlined_centered(font, &mut img, w as i64 / 2, 30, "WHY IT WORKS", 7, YELLOW);
    let s = 128u32; // one block on screen
    let panels = [
        ("1. ARMED", "its retract is queued", "for the next tick", 0),
        ("2. YOUR PRESS", "break + place a piston", "facing DOWN, same tick", 1),
        ("3. NEXT TICK", "the queued retract", "pulls the block below", 2),
    ];
    for (k, (title, l1, l2, stage)) in panels.iter().enumerate() {
        let cx = (w as i64) * (2 * k as i64 + 1) / 6;
        let x0 = cx - s as i64 / 2;
        let y_piston = 190i64;
        let y_bed = y_piston + s as i64;
        // The roof: bedrock under the piston (gone in panel 3), neighbours on both sides.
        for dx in [-1i64, 0, 1] {
            let x = x0 + dx * s as i64;
            if dx == 0 && *stage == 2 {
                img.draw_resized(&rack, x, y_bed + s as i64, s, s / 2);
                img.fill_rect(x, y_bed, s as i64, s as i64, [20, 8, 8, 255]);
                continue;
            }
            img.draw_resized(&bedrock, x, y_bed, s, s);
            img.draw_resized(&rack, x, y_bed + s as i64, s, s / 2);
        }
        // The piston: facing west (face on the left) in panel 1, facing down after the swap.
        let tex = if *stage == 0 { rot90(&side, 3) } else { rot90(&side, 2) };
        img.draw_resized(&tex, x0, y_piston, s, s);
        if *stage == 0 {
            // Its head out to the west.
            img.draw_resized(&rot90(&top, 0), x0 - s as i64, y_piston, s / 4, s);
            img.fill_rect(x0 - s as i64 * 3 / 4, y_piston + s as i64 * 3 / 8, s as i64 * 3 / 4, s as i64 / 4, [150, 120, 80, 255]);
        }
        let arrow_c = if *stage == 2 { GREEN } else { WHITE };
        if *stage == 2 {
            outlined_centered(font, &mut img, cx, y_bed + 44, "GONE", 5, GREEN);
        }
        outlined_centered(font, &mut img, cx, 120, title, 5, if *stage == 1 { YELLOW } else { arrow_c });
        outlined_centered(font, &mut img, cx, y_bed + s as i64 + s as i64 / 2 + 30, l1, 3, WHITE);
        outlined_centered(font, &mut img, cx, y_bed + s as i64 + s as i64 / 2 + 64, l2, 3, WHITE);
    }
    Ok(img)
}

// ---- audio sync check ------------------------------------------------------------------------

/// How well the game audio lines up: for every press (its exact wall time is in the try
/// segments' events), the loudest 10 ms of audio within ±0.6 s, relative to the press (the
/// break and place sounds play at the client tick after the tap: expect 0..+60 ms).
pub fn audiosync(version: &str, o: &YtOptions) -> Result<()> {
    let c = Cut::new(version, o, o.width, o.height, "cut-sync")?;
    for i in c.segs_with("try-") {
        let s = c.s(i);
        let Some((_, e)) = s.events.iter().find(|(_, e)| e.starts_with("press try")) else { continue };
        let Some(ms) = e.split("pressed at ").nth(1).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok()) else { continue };
        let t = ms / 1000.0;
        let Some(wav) = c.wav_for(i) else { continue };
        let raw = c.work.join(format!("sync-{i}.raw"));
        if !audio_span(wav, t - 0.6, t + 0.6, &raw)? || wav.fmt != "f32le" {
            continue;
        }
        let bytes = std::fs::read(&raw)?;
        let samples: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let per = (wav.rate as usize / 100) * wav.channels as usize;
        let (mut best, mut at) = (0.0f32, 0usize);
        for (k, win) in samples.chunks(per).enumerate() {
            let m = win.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            if m > best {
                best = m;
                at = k;
            }
        }
        println!("{}: loudest at {:+.0} ms from the press (peak {best:.3})", s.name, (at as f64 * 10.0) - 600.0);
    }
    Ok(())
}

// ---- review ----------------------------------------------------------------------------------

/// Frames of every recorded segment for checking (640x360 JPEGs in `review/`): at 15%, 50% and
/// 85% of each segment and at each of its events; `--from <name>` limits it to the segments whose
/// name contains it.
pub fn review(o: &YtOptions) -> Result<()> {
    let tl: Timeline = serde_json::from_str(&std::fs::read_to_string(o.out.join("timeline.json"))?)?;
    let dir = o.out.join("review");
    std::fs::create_dir_all(&dir)?;
    for s in &tl.segments {
        if let Some(f) = &o.from {
            if !s.name.contains(f.as_str()) {
                continue;
            }
        }
        let path = o.out.join("segments").join(&s.file);
        let d = probe_duration(&path).unwrap_or(0.0);
        let mut ts: Vec<(f64, String)> = [0.15, 0.5, 0.85].iter().map(|f| (d * f, format!("p{:02}", (f * 100.0) as u32))).collect();
        for (k, (t, e)) in s.events.iter().enumerate() {
            let v = t - s.start + 0.3;
            if v > 0.0 && v < d {
                ts.push((v, format!("e{k}")));
            }
            let _ = e;
        }
        for (t, tag) in ts {
            let out = dir.join(format!("{}-{tag}.jpg", s.file.trim_end_matches(".mkv")));
            ff(&["-ss".into(), format!("{t:.2}"), "-i".into(), path.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "scale=640:360".into(), "-q:v".into(), "4".into(), out.display().to_string()])?;
        }
        println!("{} {:.1}s: {}", s.file, d, s.events.iter().map(|(t, e)| format!("[{:.1}] {}", t - s.start, e.chars().take(60).collect::<String>())).collect::<Vec<_>>().join(" | "));
    }
    Ok(())
}

// ---- preview ---------------------------------------------------------------------------------

/// The overlays drawn over real frames (`preview/`), to check sizes and placement.
pub fn preview(version: &str, plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let mut c = Cut::new(version, o, o.width, o.height, "preview")?;
    let dir = o.out.join("preview");
    std::fs::create_dir_all(&dir)?;
    let frame = |c: &Cut, name: &str, t: f64| -> Result<Image> {
        let i = c.seg(name)?;
        let p = c.work.join(format!("{name}.png"));
        let seg = c.out.join("segments").join(&c.s(i).file);
        ff(&["-ss".into(), format!("{t:.2}"), "-i".into(), seg.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "eq=gamma=1.12:saturation=1.2:contrast=1.04".into(), p.display().to_string()])?;
        Image::load_png(&p)
    };
    let save = |img: &Image, name: &str| -> Result<()> {
        let p = dir.join(format!("{name}.png"));
        img.save_png(&p)?;
        // Phone size too.
        ff(&["-i".into(), p.display().to_string(), "-vf".into(), "scale=640:-2".into(), dir.join(format!("{name}-small.jpg")).display().to_string()])?;
        Ok(())
    };
    let mut f = frame(&c, "b06-sticky", 2.0)?;
    f.draw(&badge(&c.font, c.w, c.h, "6/11", "STICKY PISTON", "facing SOUTH"), 0, 0, 1);
    f.draw(&caption(&c.font, c.w, c.h, "Place it on a temporary block, looking north", WHITE), 0, 0, 1);
    save(&f, "step")?;
    let ov = c.seg("overview-1")?;
    let pose = c.s(ov).events.iter().find(|(_, e)| e.starts_with("pose")).map(|(_, e)| e.clone()).context("pose")?;
    let mut f = frame(&c, "overview-1", 1.0)?;
    if let Some(l) = labels(&c.font, c.w, c.h, &pose, plan.t1) {
        f.draw(&l, 0, 0, 1);
    }
    f.draw(&caption(&c.font, c.w, c.h, "The whole machine: 11 blocks on top of the roof", YELLOW), 0, 0, 1);
    save(&f, "labels")?;
    let mut f = frame(&c, "overview-2", 1.0)?;
    f.draw(&big(&c.font, c.w, c.h, 40, &[("11 BLOCKS.", 11, YELLOW)]), 0, 0, 1);
    f.draw(&big(&c.font, c.w, c.h, 160, &[("NO LAG. NO TNT.", 7, WHITE)]), 0, 0, 1);
    f.draw(&big(&c.font, c.w, c.h, 560, &[("BREAKS NETHER ROOF BEDROCK", 5, WHITE)]), 0, 0, 1);
    save(&f, "hook")?;
    let mut f = frame(&c, "liedown", 6.0)?;
    f.draw(&beat_bar(&c.font, c.w, c.h, 0), 0, 0, 1);
    f.draw(&caption(&c.font, c.w, c.h, "Press R right when the piston's arm pops out (GO)", YELLOW), 0, 0, 1);
    save(&f, "beat")?;
    let math = card(&c.font, c.w, c.h, "HASTE II IS REQUIRED", &[
        ("A piston breaks INSTANTLY at mining speed 45+", WHITE),
        ("Netherite + Efficiency V: 35", WHITE),
        ("x 1.4 with Haste II = 49  -> instant", GREEN),
        ("Diamond: 34 x 1.4 = 47.6  -> instant", GREEN),
        ("Haste I: x 1.2 = 42  -> NOT instant", RED),
        ("No instant break = the swap can't fit in one tick", GRAY),
    ], &[]);
    let mut f = frame(&c, "kit-hotbar", 1.0)?;
    f.draw(&math, 0, 0, 1);
    save(&f, "card")?;
    let mut f = frame(&c, "overview-1", 1.0)?;
    f.draw(&diagram_swap(&c.font, version, c.w, c.h)?, 0, 0, 1);
    save(&f, "diagram")?;
    let _ = c.png(&math)?;
    Ok(())
}

// ---- the Short -------------------------------------------------------------------------------

pub fn short(version: &str, _plan: &YtPlan, o: &YtOptions) -> Result<()> {
    // Clips cut like the video's, rendered at 1280x720, then the vertical frame: a square crop of
    // the middle, 1080 wide, with the text above and below.
    let mut c = Cut::new(version, o, o.width, o.height, "cut-short")?;
    let (hit_i, hit_p, _cue, _) = tries(&c)?;
    let mut parts: Vec<(Clip, Vec<(String, u32, [u8; 3])>, Vec<(String, [u8; 3])>)> = Vec::new();
    let top = |a: &str, s: u32, col: [u8; 3]| vec![(a.to_string(), s, col)];
    let hook = || vec![("11 BLOCKS.".to_string(), 13u32, YELLOW), ("NO LAG. NO TNT.".to_string(), 8u32, WHITE)];
    let o2 = c.seg("overview-2")?;
    parts.push((seg_clip(o2, 0.4, 2.2, 1.0), hook(), vec![("Nether roof bedrock, survival".into(), WHITE)]));
    let hs = c.seg("hole-shot")?;
    parts.push((seg_clip(hs, 0.4, 2.4, 1.0), hook(), vec![("A hole in the roof".into(), GREEN)]));
    for (name, n, title, _sub, _tip) in steps() {
        let i = c.seg(name)?;
        let evs = c.evs(i, "placed");
        let last = evs.iter().map(|(t, _)| *t).fold(0.0f64, f64::max);
        parts.push((seg_clip(i, 0.2, (last + 0.6).min(c.dur(i)), 4.0), top(n, 12, YELLOW), vec![(title.to_string(), WHITE)]));
    }
    let kb = c.seg("keybinds")?;
    let sa = c.ev(kb, "select attack").unwrap_or(3.0);
    parts.push((seg_clip(kb, (sa - 0.5).max(0.0), (sa + 4.5).min(c.dur(kb)), 2.5), top("1 KEY =", 11, YELLOW), vec![("ATTACK + USE".into(), WHITE)]));
    let ld = c.seg("liedown")?;
    let crawl = c.ev(ld, "crawling").unwrap_or(c.dur(ld) - 1.0);
    parts.push((seg_clip(ld, 0.2, (crawl + 0.8).min(c.dur(ld)), 3.0), top("CRAWL UNDER", 10, YELLOW), vec![("the trapdoor".into(), WHITE)]));
    parts.push((seg_clip(hit_i, (hit_p - 1.2).max(0.0), hit_p + 0.8, 1.0), top("PRESS WHEN", 10, YELLOW), vec![("the arm pops out".into(), WHITE)]));
    let sp = c.seg("slowmo-press")?;
    let press = c.ev(sp, "press (frozen)").unwrap_or(2.0);
    let next = c.ev(sp, "next tick").unwrap_or(press + 1.5);
    parts.push((seg_clip(sp, (press - 0.6).max(0.0), next + 1.0, 1.5), top("SLOW MOTION", 10, YELLOW), vec![("its retract deletes the bedrock".into(), GREEN)]));
    let dr = c.seg("drop")?;
    let fall = c.ev(dr, "falling").unwrap_or(c.dur(dr) - 8.0);
    parts.push((seg_clip(dr, (fall - 1.5).max(0.0), (fall + 7.0).min(c.dur(dr)), 2.0), top("THROUGH", 13, YELLOW), vec![("the Nether roof".into(), WHITE), ("Full tutorial on the channel".into(), AQUA)]));
    // Render each part at 1280x720 (no overlays), then the vertical frame.
    let (vw, vh) = (1080u32, 1920u32);
    let mut list = String::new();
    let mut total = 0.0;
    for (k, (clip, tops, bots)) in parts.iter().enumerate() {
        let (p, secs) = c.render(k, clip)?;
        total += secs;
        let mut img = Image::new(vw, vh, [0, 0, 0, 0]);
        // The largest scale (up to the asked one) that fits the frame's width.
        let fit = |t: &str, s: u32| -> u32 { (2..=s).rev().find(|&k| c.font.width(t, k) <= vw - 60).unwrap_or(2) };
        let mut y = 150i64;
        for (t, s, col) in tops {
            let s = fit(t, *s);
            outlined_centered(&c.font, &mut img, vw as i64 / 2, y, t, s, *col);
            y += 9 * s as i64 + 16;
        }
        let mut y = 1560i64;
        for (t, col) in bots {
            let s = fit(t, 7);
            outlined_centered(&c.font, &mut img, vw as i64 / 2, y, t, s, *col);
            y += 9 * s as i64 + 18;
        }
        let png = c.png(&img)?;
        let vout = c.work.join("clips").join(format!("v{k:03}.mkv"));
        ff(&[
            "-i".into(), p.display().to_string(), "-loop".into(), "1".into(), "-framerate".into(), FPS.to_string(), "-i".into(), png.display().to_string(),
            "-filter_complex".into(),
            format!("[0:v]crop=720:720:280:0,scale=1080:1080:flags=lanczos,pad={vw}:{vh}:0:450:color=0x140808,setsar=1[b];[b][1:v]overlay=0:0:shortest=1,format=yuv420p[v]"),
            "-map".into(), "[v]".into(), "-map".into(), "0:a".into(), "-t".into(), format!("{secs:.3}"),
            "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "15".into(), "-r".into(), FPS.to_string(), "-c:a".into(), "pcm_s16le".into(),
            vout.display().to_string(),
        ])?;
        list.push_str(&format!("file '{}'\n", vout.display()));
    }
    if total > 59.0 {
        eprintln!("short: WARN {total:.1}s (over 59 s)");
    }
    let lst = c.work.join("vlist.txt");
    std::fs::write(&lst, list)?;
    let joined = c.work.join("vjoined.mkv");
    ff(&["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), lst.display().to_string(), "-c".into(), "copy".into(), joined.display().to_string()])?;
    let out = o.out.join("bedrock-roof-11-blocks-short.webm");
    ff(&[
        "-i".into(), joined.display().to_string(), "-vf".into(), "scale=out_range=tv,format=yuv420p".into(),
        "-c:v".into(), "libvpx-vp9".into(), "-b:v".into(), "0".into(), "-crf".into(), "30".into(), "-row-mt".into(), "1".into(), "-deadline".into(), "good".into(), "-cpu-used".into(), "3".into(),
        "-color_range".into(), "tv".into(), "-af".into(), "loudnorm=I=-16:TP=-1.5:LRA=11".into(), "-c:a".into(), "libopus".into(), "-b:a".into(), "128k".into(), out.display().to_string(),
    ])?;
    let d = probe_duration(&out)?;
    c.stills(&out, "stills-short", 2.0, d, "360:640")?;
    println!("short {} ({d:.1}s)", out.display());
    Ok(())
}

// ---- the thumbnail ---------------------------------------------------------------------------

pub fn thumb(version: &str, _plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let c = Cut::new(version, o, 1280, 720, "cut-thumb")?;
    let hs = c.seg("hole-shot")?;
    let frame = c.work.join("frame.png");
    let seg = o.out.join("segments").join(&c.s(hs).file);
    ff(&["-ss".into(), "1.2".into(), "-i".into(), seg.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "scale=1280:720,eq=saturation=1.35:contrast=1.12:brightness=0.03".into(), frame.display().to_string()])?;
    let mut img = Image::load_png(&frame)?;
    let f = &c.font;
    // Left: the big words. Right: an arrow to the hole (frame center, a little low).
    let mut shade = Image::new(1280, 720, [0, 0, 0, 0]);
    shade.fill_rect(0, 0, 620, 720, [0, 0, 0, 110]);
    img.draw(&shade, 0, 0, 1);
    outlined(f, &mut img, 40, 60, "11", 26, YELLOW);
    outlined(f, &mut img, 40, 300, "BLOCKS", 13, YELLOW);
    outlined(f, &mut img, 44, 440, "NO LAG", 10, WHITE);
    outlined(f, &mut img, 44, 550, "NO TNT", 10, RED);
    // The arrow: a thick shaft and head pointing down-right at the hole.
    let (hx, hy) = (640i64 + 40, 430i64);
    // A red ring around the hole (the frame looks at the hole's center: the middle of the frame).
    let (cx, cy, rad) = (640.0f64, 362.0f64, 118.0f64);
    for y in (cy - rad - 14.0) as i64..(cy + rad + 14.0) as i64 {
        for x in (cx - rad - 14.0) as i64..(cx + rad + 14.0) as i64 {
            let d = ((x as f64 - cx).powi(2) + ((y as f64 - cy) * 1.25).powi(2)).sqrt();
            if (d - rad).abs() < 10.0 {
                img.fill_rect(x, y, 1, 1, [0, 0, 0, 255]);
            }
            if (d - rad).abs() < 6.0 {
                img.fill_rect(x, y, 1, 1, [255, 60, 50, 255]);
            }
        }
    }
    // The arrow, from the words to the ring.
    let (ax0, ay0, ax1, ay1) = (400i64, 150i64, 528i64, 262i64);
    let n = 128i64;
    for (pad, col) in [(11i64, [0u8, 0, 0, 255]), (7, [255, 222, 40, 255])] {
        for i in 0..=n {
            let (x, y) = (ax0 + (ax1 - ax0) * i / n, ay0 + (ay1 - ay0) * i / n);
            img.fill_rect(x - pad, y - pad, 2 * pad, 2 * pad, col);
        }
        // The head: two short strokes back from the tip.
        for i in 0..=40i64 {
            img.fill_rect(ax1 - i - pad / 2, ay1 - pad / 2, pad, pad, col);
            img.fill_rect(ax1 - pad / 2, ay1 - i - pad / 2, pad, pad, col);
        }
    }
    let _ = (hx, hy);
    let out = o.out.join("bedrock-roof-11-blocks-thumb.png");
    img.save_png(&out)?;
    let jpg = o.out.join("bedrock-roof-11-blocks-thumb.jpg");
    img.save(&jpg)?;
    println!("thumbnail {}", out.display());
    Ok(())
}
