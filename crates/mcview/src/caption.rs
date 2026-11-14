//! Text on a rendered picture: a labelled render in one command (`mctas web framecost --title
//! --legend --labels`, docs/webrender.md). A title band above the frame, a legend band below it,
//! and labels in boxes at pixel positions of the frame, black on white like a figure's callouts,
//! in DejaVu Sans (the system's; `--font` for another TTF, its `-Bold` sibling for the title).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One label: its box's top-left corner in the frame's pixels and its lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub x: i32,
    pub y: i32,
    pub lines: Vec<String>,
}

/// What goes on each picture: the title's lines (a band above), the legend's (a band below), the
/// labels (on the frame) and the labels' text size in pixels.
#[derive(Clone, Debug, Default)]
pub struct Caption {
    pub title: Vec<String>,
    pub legend: Vec<String>,
    pub labels: Vec<Label>,
    pub size: f32,
}

impl Caption {
    /// Nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.legend.is_empty() && self.labels.is_empty()
    }
}

/// The two faces: the labels' and the legend's, and the title's (bold when the font has one).
pub struct Fonts {
    regular: fontdue::Font,
    bold: fontdue::Font,
}

/// Where the system keeps DejaVu Sans (Fedora's and Debian's paths).
const SYSTEM_FONTS: [&str; 2] = ["/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"];

impl Fonts {
    /// The font at `path` (None: the system's DejaVu Sans) and its bold sibling (`X-Bold.ttf`
    /// beside `X.ttf`; the regular face again when there is none).
    pub fn load(path: Option<&Path>) -> Result<Fonts, String> {
        let regular_path: PathBuf = match path {
            Some(p) => p.to_path_buf(),
            None => SYSTEM_FONTS.iter().map(PathBuf::from).find(|p| p.exists()).ok_or_else(|| format!("no DejaVu Sans at {} (install dejavu-sans-fonts, or pass --font <ttf>)", SYSTEM_FONTS.join(" or ")))?,
        };
        let read = |p: &Path| -> Result<fontdue::Font, String> {
            let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).map_err(|e| format!("{}: {e}", p.display()))
        };
        let regular = read(&regular_path)?;
        let bold_path = regular_path.with_file_name(format!("{}-Bold.ttf", regular_path.file_stem().and_then(|s| s.to_str()).unwrap_or("")));
        let bold = if bold_path.exists() { read(&bold_path)? } else { read(&regular_path)? };
        Ok(Fonts { regular, bold })
    }
}

/// A labels file: one label a line, `x y text` (its box's top-left corner in the frame's pixels,
/// then its text; `\n` in the text breaks a line); blank lines and `#` comments skipped.
pub fn parse_labels(text: &str) -> Result<Vec<Label>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.splitn(3, char::is_whitespace);
        let (Some(x), Some(y), Some(rest)) = (it.next(), it.next(), it.next()) else { return Err(format!("labels line {}: `x y text`", n + 1)) };
        let x: i32 = x.parse().map_err(|_| format!("labels line {}: x {x:?} is not a whole number", n + 1))?;
        let y: i32 = y.parse().map_err(|_| format!("labels line {}: y {y:?} is not a whole number", n + 1))?;
        out.push(Label { x, y, lines: split_lines(rest.trim()) });
    }
    Ok(out)
}

/// A text's lines: broken at each `\n` (the two characters, as a flag or a labels file writes a
/// line break) and at each real newline.
pub fn split_lines(s: &str) -> Vec<String> {
    s.replace("\\n", "\n").lines().map(str::to_string).collect()
}

/// Glyphs rasterized once a face and size (an orbit's 144 frames draw the same title).
struct Glyphs<'a> {
    font: &'a fontdue::Font,
    px: f32,
    cache: HashMap<char, (fontdue::Metrics, Vec<u8>)>,
}

impl<'a> Glyphs<'a> {
    fn new(font: &'a fontdue::Font, px: f32) -> Self {
        Glyphs { font, px, cache: HashMap::new() }
    }

    fn glyph(&mut self, c: char) -> &(fontdue::Metrics, Vec<u8>) {
        let (font, px) = (self.font, self.px);
        self.cache.entry(c).or_insert_with(|| font.rasterize(c, px))
    }

    fn width(&mut self, s: &str) -> f32 {
        s.chars().map(|c| self.glyph(c).0.advance_width).sum()
    }

    fn line_height(&self) -> f32 {
        (self.px * 1.3).ceil()
    }

    fn ascent(&self) -> f32 {
        self.font.horizontal_line_metrics(self.px).map_or(self.px * 0.8, |m| m.ascent)
    }
}

/// An RGBA picture being drawn on.
struct Canvas {
    rgba: Vec<u8>,
    w: i32,
    h: i32,
}

impl Canvas {
    /// `color` over pixel (x, y) at opacity `a` (0..1); outside the picture: nothing.
    fn blend(&mut self, x: i32, y: i32, color: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || a <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        for k in 0..3 {
            let v = self.rgba[i + k] as f32 * (1.0 - a) + color[k] as f32 * a;
            self.rgba[i + k] = v.round().clamp(0.0, 255.0) as u8;
        }
        self.rgba[i + 3] = 255;
    }

    /// A box of `lines` with its top-left corner at (x, y): white at 80% behind black text, the
    /// box `pad` pixels around the text, moved left (or right) to stay inside the picture.
    fn text_box(&mut self, g: &mut Glyphs, x: i32, y: i32, lines: &[String], pad: i32) {
        if lines.is_empty() {
            return;
        }
        let lh = g.line_height();
        let w = lines.iter().map(|l| g.width(l)).fold(0.0f32, f32::max).ceil() as i32;
        let h = (lh * lines.len() as f32).ceil() as i32;
        let x = x.min(self.w - w - pad).max(pad);
        for yy in y - pad..y + h + pad {
            for xx in x - pad..x + w + pad {
                self.blend(xx, yy, [255, 255, 255], 0.8);
            }
        }
        let ascent = g.ascent();
        for (k, line) in lines.iter().enumerate() {
            let baseline = y as f32 + k as f32 * lh + ascent;
            let mut pen = x as f32;
            for c in line.chars() {
                let (m, bitmap) = g.glyph(c).clone();
                // (fontdue: `ymin` is the glyph's bottom edge above the baseline)
                let top = (baseline - (m.height as f32 + m.ymin as f32)).round() as i32;
                let left = (pen + m.xmin as f32).round() as i32;
                for gy in 0..m.height {
                    for gx in 0..m.width {
                        let cov = bitmap[gy * m.width + gx] as f32 / 255.0;
                        self.blend(left + gx as i32, top + gy as i32, [0, 0, 0], cov);
                    }
                }
                pen += m.advance_width;
            }
        }
    }
}

/// The text size that fits `lines` in `max_w` pixels: `px`, or smaller in proportion when the
/// widest line would overflow (a band's title or legend across a narrow picture), never below 8.
pub fn fit_px(fonts: &Fonts, bold: bool, px: f32, lines: &[String], max_w: f32) -> f32 {
    let mut g = Glyphs::new(if bold { &fonts.bold } else { &fonts.regular }, px);
    let w = lines.iter().map(|l| g.width(l)).fold(0.0f32, f32::max);
    if w > max_w && w > 0.0 { (px * max_w / w).floor().max(8.0) } else { px }
}

/// The picture with its caption: the title band above (if any), the legend band below (if any),
/// the labels on the frame (their positions the frame's, below the title band). Returns the new
/// picture and its size.
pub fn apply(rgba: &[u8], w: u32, h: u32, cap: &Caption, fonts: &Fonts) -> (Vec<u8>, u32, u32) {
    let size = if cap.size > 0.0 { cap.size } else { 24.0 };
    let room = w as f32 - 48.0;
    let mut title = Glyphs::new(&fonts.bold, fit_px(fonts, true, (size * 1.08).round(), &cap.title, room));
    let mut label = Glyphs::new(&fonts.regular, size);
    let mut legend = Glyphs::new(&fonts.regular, fit_px(fonts, false, (size * 0.8).round(), &cap.legend, room));
    let band = |g: &Glyphs, n: usize| if n == 0 { 0 } else { (g.line_height() * n as f32).ceil() as i32 + 28 };
    let top = band(&title, cap.title.len());
    let bottom = band(&legend, cap.legend.len());
    let (w, h) = (w as i32, h as i32);
    let mut c = Canvas { rgba: Vec::with_capacity((w * (h + top + bottom) * 4) as usize), w, h: h + top + bottom };
    let fill = [0xD8u8, 0xE2, 0xF0, 255];
    c.rgba.extend(std::iter::repeat_n(fill, (w * top) as usize).flatten());
    c.rgba.extend_from_slice(&rgba[..(w * h * 4) as usize]);
    c.rgba.extend(std::iter::repeat_n(fill, (w * bottom) as usize).flatten());
    c.text_box(&mut title, 16, 14, &cap.title, 8);
    for l in &cap.labels {
        c.text_box(&mut label, l.x, l.y + top, &l.lines, 6);
    }
    c.text_box(&mut legend, 14, top + h + 14, &cap.legend, 6);
    (c.rgba, w as u32, c.h as u32)
}

/// Pictures side by side, left to right, `gap` pixels of the band colour between them, all as tall
/// as the tallest (a shorter one's foot filled): a comparison of two designs in one picture
/// (`web framecost a b --side-by-side`).
pub fn hstack(pictures: &[(Vec<u8>, u32, u32)], gap: u32) -> (Vec<u8>, u32, u32) {
    let h = pictures.iter().map(|p| p.2).max().unwrap_or(0);
    let w = pictures.iter().map(|p| p.1).sum::<u32>() + gap * pictures.len().saturating_sub(1) as u32;
    let mut out: Vec<u8> = std::iter::repeat_n([0xD8u8, 0xE2, 0xF0, 255], (w * h) as usize).flatten().collect();
    let mut x0 = 0u32;
    for (rgba, pw, ph) in pictures {
        for y in 0..*ph {
            let src = &rgba[(y * pw * 4) as usize..((y + 1) * pw * 4) as usize];
            let at = ((y * w + x0) * 4) as usize;
            out[at..at + src.len()].copy_from_slice(src);
        }
        x0 += pw + gap;
    }
    (out, w, h)
}

/// [`apply`] to each `frame_*.png` in `dir` (a `--png-dir`'s film), written back in place: how
/// many pictures it captioned.
pub fn apply_to_dir(dir: &Path, cap: &Caption, fonts: &Fonts) -> Result<usize, String> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("frame_") && n.ends_with(".png")))
        .collect();
    names.sort();
    for p in &names {
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let (rgba, w, h) = crate::atlas::vanilla::decode_png(&bytes)?;
        let (out, w, h) = apply(&rgba, w, h, cap, fonts);
        crate::render::headless::write_png(p, w, h, &out)?;
    }
    Ok(names.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A labels file: `x y text` lines, `\n` breaking a label's lines, comments and blank lines
    /// skipped, a malformed line refused with its number.
    #[test]
    fn labels_parse_with_their_line_breaks() {
        let l = parse_labels("# the module\n\n120 340 lanes ×3 (S-bend)\n-5 7 split row:\\nlevers emulated\n").unwrap();
        assert_eq!(l, vec![Label { x: 120, y: 340, lines: vec!["lanes ×3 (S-bend)".into()] }, Label { x: -5, y: 7, lines: vec!["split row:".into(), "levers emulated".into()] }]);
        assert!(parse_labels("12 lanes").unwrap_err().contains("line 1"));
        assert!(parse_labels("a 3 lanes").unwrap_err().contains("x"));
    }

    /// A caption adds its title band above and its legend band below, keeps the frame's pixels
    /// between them, and draws a label's dark text on a light box where the label sits (the
    /// system's DejaVu Sans: the render boxes have it, `dejavu-sans-fonts`).
    #[test]
    fn a_caption_frames_the_picture_and_draws_its_labels() {
        let fonts = Fonts::load(None).expect("DejaVu Sans for the caption test");
        let (w, h) = (300u32, 200u32);
        let frame: Vec<u8> = std::iter::repeat_n([40u8, 60, 90, 255], (w * h) as usize).flatten().collect();
        let cap = Caption { title: vec!["F1 + Q2".into()], legend: vec!["not laid".into(), "emulated".into()], labels: vec![Label { x: 50, y: 80, lines: vec!["holds".into()] }], size: 24.0 };
        let (out, ow, oh) = apply(&frame, w, h, &cap, &fonts);
        assert_eq!(ow, w);
        assert!(oh > h + 60, "both bands added: {oh}");
        let px = |x: u32, y: u32| -> [u8; 3] {
            let i = ((y * ow + x) * 4) as usize;
            [out[i], out[i + 1], out[i + 2]]
        };
        // the title band's height: the frame starts where the frame's colour does
        let top = (0..oh).find(|&y| px(w - 1, y) == [40, 60, 90]).unwrap();
        assert!(top > 20, "a title band: {top}");
        assert_eq!(px(w - 1, top + h - 1), [40, 60, 90], "the frame's last row kept");
        assert_ne!(px(w - 1, top + h), [40, 60, 90], "the legend band below it");
        // the label: a light box with dark text inside, where the label sits
        let region: Vec<[u8; 3]> = (top + 80..top + 110).flat_map(|y| (50..110).map(move |x| (x, y))).map(|(x, y)| px(x, y)).collect();
        assert!(region.iter().any(|c| c[0] > 180), "the label's box is light");
        assert!(region.iter().any(|c| c[0] < 40 && c[2] < 40), "the label's text is dark");
        // nowhere else on the frame is touched
        assert_eq!(px(250, top + 180), [40, 60, 90]);
    }

    /// A title or legend wider than the picture is set smaller to fit it (never below 8 px), one
    /// that fits keeps its size; a label at the right edge moves left to stay inside.
    #[test]
    fn a_long_line_is_set_to_fit_and_a_label_stays_inside() {
        let fonts = Fonts::load(None).expect("DejaVu Sans for the caption test");
        let long = vec!["f1q2-module-record7f13b.txt, sha256 ed5ec73b…e688eb98 (artifact 1461484826186330; not committed yet)".to_string()];
        let px = fit_px(&fonts, true, 26.0, &long, 600.0);
        assert!(px < 26.0 && px >= 8.0, "{px}");
        let mut g = Glyphs::new(&fonts.bold, px);
        assert!(g.width(&long[0]) <= 600.0 + 1.0, "fits: {}", g.width(&long[0]));
        assert_eq!(fit_px(&fonts, true, 26.0, &["F1 + Q2".to_string()], 600.0), 26.0);
        // a label placed past the right edge: its box drawn inside, the frame's last column light
        let (w, h) = (200u32, 100u32);
        let frame: Vec<u8> = std::iter::repeat_n([10u8, 10, 10, 255], (w * h) as usize).flatten().collect();
        let cap = Caption { labels: vec![Label { x: 190, y: 40, lines: vec!["bubble lift".into()] }], size: 20.0, ..Default::default() };
        let (out, ow, _) = apply(&frame, w, h, &cap, &fonts);
        let col_light = (40..60).any(|y| out[((y * ow + (ow - 8)) * 4) as usize] > 150);
        let left_light = (40..60).any(|y| out[((y * ow + 100) * 4) as usize] > 150);
        assert!(col_light && left_light, "the box moved left inside the picture");
    }

    /// Two pictures side by side: their pixels kept at their offsets, the gap in the band colour,
    /// a shorter picture's foot filled.
    #[test]
    fn pictures_stand_side_by_side() {
        let a: Vec<u8> = std::iter::repeat_n([255u8, 0, 0, 255], 4 * 3).flatten().collect();
        let b: Vec<u8> = std::iter::repeat_n([0u8, 0, 255, 255], 2 * 2).flatten().collect();
        let (out, w, h) = hstack(&[(a, 4, 3), (b, 2, 2)], 1);
        assert_eq!((w, h), (7, 3));
        let px = |x: u32, y: u32| out[((y * w + x) * 4) as usize..((y * w + x) * 4 + 3) as usize].to_vec();
        assert_eq!(px(0, 0), [255, 0, 0]);
        assert_eq!(px(3, 2), [255, 0, 0]);
        assert_eq!(px(4, 0), [0xD8, 0xE2, 0xF0], "the gap");
        assert_eq!(px(5, 1), [0, 0, 255]);
        assert_eq!(px(6, 2), [0xD8, 0xE2, 0xF0], "the shorter picture's foot");
    }
}
