//! `mctas web ...`: the in-browser viewer's commands (docs/webrender.md). Argument parsing and
//! dispatch only (compiled unoptimized); the work is in `crates/mcview`.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::{Path, PathBuf};

mod cdp;
mod qa;
mod recqa;
mod single;

#[derive(Subcommand)]
pub enum WebCmd {
    /// The viewer's entities (redsim, what the browser draws) tick by tick, next to a real-game
    /// trace's (`designs/clocks/traces/<stem>.trace`, `mctas clock misc trace-export`) with the
    /// distance between them; ends with the largest distance over the ticks shown.
    Entities {
        design: PathBuf,
        #[arg(long)]
        trace: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long, default_value_t = 40)]
        to: u32,
    },
    /// Write the game texture the viewer samples (the game's block atlas, with the entity and
    /// item textures packed below it) to a PNG, and list where the entity textures went.
    GameTexture {
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Render designs to PNG offscreen with the browser viewer's renderer (wgpu, Vulkan headless:
    /// lavapipe on a GPU-less box). Clocks at their most active moment (like `clock shot`), doors
    /// at rest; `--tick N` runs N ticks first, `--flips N` flips a door's lever N times.
    Shot {
        designs: Vec<PathBuf>,
        /// Output PNG (one design) or directory (`<stem>.png` each).
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 800)]
        width: u32,
        #[arg(long, default_value_t = 450)]
        height: u32,
        #[arg(long)]
        tick: Option<u32>,
        #[arg(long, default_value_t = 0)]
        flips: u32,
        /// Camera yaw / pitch in degrees (default: the real-game pictures' camera).
        #[arg(long)]
        yaw: Option<f32>,
        #[arg(long)]
        pitch: Option<f32>,
        /// The hand-drawn tiles only (default: the game's textures, the vendored ones in
        /// `web/textures/26.3`, local only).
        #[arg(long)]
        procedural: bool,
        /// As a live viewer of this size frames it (its default camera and outlines), not a
        /// thumbnail.
        #[arg(long)]
        live: bool,
        /// With --live: the device pixel ratio (the viewer's CSS size is the picture's / this;
        /// a phone: --width 654 --height 960 --dpr 2).
        #[arg(long, default_value_t = 1.0)]
        dpr: f32,
        /// Keep only this part of the picture, `x,y,w,h` in its pixels (a detail: a mob model,
        /// a flame), written at that size.
        #[arg(long)]
        crop: Option<String>,
        /// The game's day time (ticks, 0..24000; 6000 noon, 18000 midnight): the picture's sky,
        /// sun, moon and light as a live view shows that moment (`mcview::daylight`; default: the
        /// pictures' noon, no sun drawn).
        #[arg(long)]
        day_time: Option<i64>,
        /// With --day-time: the rain's and the thunder's levels (0..1).
        #[arg(long, default_value_t = 0.0)]
        rain: f32,
        #[arg(long, default_value_t = 0.0)]
        thunder: f32,
        /// Compare each picture with the PNG of the same name in this folder (or this PNG, for one
        /// design): the share of pixels that differ by more than 2 levels and the largest
        /// difference (a renderer change that must keep the pictures).
        #[arg(long)]
        against: Option<PathBuf>,
    },
    /// Check the viewer against the repo's tools: its design parser against `mctas::design` on
    /// every block and port, and its clock period detector (the code the browser runs) against
    /// `mctas clock sim` (state warmup + period, visible warmup + period).
    Check {
        designs: Vec<PathBuf>,
        /// Ticks to look for a repeat.
        #[arg(long, default_value_t = 50_000)]
        horizon: u32,
        /// Only failures and the summary.
        #[arg(long, short)]
        quiet: bool,
        /// Also run the period detector of this built wasm (web/viewer/mcview_bg.wasm, run by an
        /// interpreter) and require the same numbers: the browser simulates exactly like redsim.
        #[arg(long)]
        wasm: Option<PathBuf>,
        /// Hold the kinds the viewer shows as built on purpose (`mcview::circuit::not_replayed_yet`)
        /// to their judge's run anyway: the work that makes them exact.
        #[arg(long)]
        unverified: bool,
    },
    /// Build the browser bundle: `crates/mcview-web` for wasm32 (profile `web`), then wasm-bindgen
    /// (`--target web --out-name mcview`): `<out>/mcview.js` + `<out>/mcview_bg.wasm`. Needs the
    /// wasm32 target (`rustup target add wasm32-unknown-unknown`) and `wasm-bindgen` 0.2.129
    /// (`cargo install wasm-bindgen-cli --version =0.2.129`). Run from the repository root.
    Bundle {
        #[arg(long, short, default_value = "web/viewer")]
        out: PathBuf,
    },
    /// A generated page with its live viewer as ONE self-contained HTML file (the viewer's script,
    /// WebAssembly and textures, the design and terrain files it fetches, the page's style), to
    /// share where only a single file goes (an artifact): the files are served from inside the page
    /// by a `fetch` shim (crates/web-cli/src/single.rs).
    Single {
        /// The page (in a generated site folder, or a page laid out like one).
        page: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Write a standalone demo page: `<out>/index.html` (a grid of thumbnails and live viewers),
    /// the designs under `<out>/designs/`, the bundle copied from `--viewer` into `<out>/viewer/`.
    Demo {
        designs: Vec<PathBuf>,
        #[arg(long, short)]
        out: PathBuf,
        /// The built bundle (mcview.js + mcview_bg.wasm).
        #[arg(long, default_value = "web/viewer")]
        viewer: PathBuf,
        /// How many of the designs get a live viewer (the first ones).
        #[arg(long, default_value_t = 2)]
        live: usize,
        /// The live viewers start playing (`data-autoplay`).
        #[arg(long)]
        autoplay: bool,
    },
    /// Serve a directory over HTTP on localhost (to open a demo page in a browser).
    Serve {
        dir: PathBuf,
        #[arg(long, default_value_t = 8000)]
        port: u16,
    },
    /// Sanity check against the real game: each design's real-game picture
    /// (`<img-dir>/<stem>.jpg`, from `mctas clock shot`) next to the viewer's frame of the same
    /// moment and camera, as `<out>/<stem>.cmp.png`, with the mean RGB difference per pixel and on
    /// a coarse 32x18 grid.
    Compare {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "designs/clocks/img")]
        img_dir: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
        /// The hand-drawn tiles only (default: the game's textures).
        #[arg(long)]
        procedural: bool,
        /// Also write `<stem>.zoom.png`: this region (x,y,w,h of the picture) of the real picture
        /// and of the viewer's, side by side, scaled up 4x (nearest), to compare pixels.
        #[arg(long)]
        zoom: Option<String>,
        /// The ground's reach around the build (default: the viewer's, 56 around a clock), to
        /// measure the pictures' floor edges.
        #[arg(long)]
        floor_margin: Option<f32>,
        /// The designs are still frames (`web snapshot`) pictured as written (`clock shot
        /// --still`): compared at tick 0, no marker.
        #[arg(long)]
        still: bool,
        /// An earlier run's output folder (another viewer build's `-o`): also writes
        /// `<out>/before-after.png`, each design's real picture, that run's view and this one's,
        /// with both differences, and prints both means.
        #[arg(long)]
        before: Option<PathBuf>,
    },
    /// Each design's still frame in the viewer as a plain design of blocks (and its own
    /// entities): `<out>/<stem>.txt`, for `clock shot --still` (the real game) and `web compare
    /// --still` (the viewer) to picture the very same blocks: what differs is the rendering.
    Snapshot {
        designs: Vec<PathBuf>,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Layout-shift check of a page's live viewers in headless Chromium (real time, WebGL2 in
    /// software): opens `<url>?mcview-debug`, brings each viewer on screen in turn (a viewer only
    /// draws and plays on screen) for up to `--seconds`, reads every frame's bounding boxes of the
    /// viewer, its bar, counter and badge, and fails unless they never moved over at least
    /// `--min-ticks` ticks (give the viewers `data-autoplay`).
    Layout {
        url: String,
        #[arg(long, default_value_t = 8)]
        seconds: u64,
        /// Window size in CSS px, `W,H` (390,844 for a phone).
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        #[arg(long, default_value_t = 100)]
        min_ticks: u32,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Click test of a page's live viewers in headless Chromium (real input events through the
    /// DevTools protocol): for every viewer with a lever or button, clicks it (or taps it with
    /// `--touch`) while paused and requires the input to flip, the simulation to start and the
    /// build to react (a door's opening changes, or another block turns on) within `--wait`
    /// seconds.
    Click {
        url: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Tap (touch events, a phone) instead of a mouse click.
        #[arg(long)]
        touch: bool,
        /// Press and HOLD each circuit input in the view instead: it must be on while held, off
        /// once released, and the camera must not move.
        #[arg(long)]
        hold: bool,
        #[arg(long, default_value_t = 8)]
        wait: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Press a live viewer's Record button in headless Chromium and keep the video it downloads
    /// (`<design>.mp4` or `.webm`) in `--out`.
    Record {
        url: String,
        #[arg(long, short)]
        out: PathBuf,
        /// Which live viewer of the page (0: the first).
        #[arg(long, default_value_t = 0)]
        index: usize,
        #[arg(long, default_value = "1280,720")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Seconds to wait for the file.
        #[arg(long, default_value_t = 300)]
        timeout: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// The Record button's files at phone size, in Chromium and WebKitGTK: each page's live viewer
    /// recorded (the file it offers caught), every file played in both browsers (it loads, plays
    /// on, draws a picture) and checked natively: the codec and size, the duration (its frames at
    /// 30/s), every frame decoded and none black, its colours next to what the encoder read (the
    /// range and matrix its pixels use, and that its tags say so). Writes the files, a sheet of
    /// each one's frames and a report.
    Recqa {
        /// Pages of the site (paths in `--dir`, e.g. `doors/e78k-h3-38b.html`).
        pages: Vec<String>,
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
        /// `chromium`, `chrome` (needs `--chrome`), `webkit` (comma-separated).
        #[arg(long, default_value = "chromium,webkit")]
        engines: String,
        #[arg(long, default_value = "390,844")]
        window: String,
        /// Chromium's device scale (WebKit's rig is 2).
        #[arg(long, default_value_t = 3.0)]
        scale: f32,
        /// Seconds to wait for each file.
        #[arg(long, default_value_t = 300)]
        timeout: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
        /// Google's Chrome for Testing headless shell (H.264 in MP4: the file most visitors get),
        /// for `--engines ...,chrome`.
        #[arg(long)]
        chrome: Option<PathBuf>,
    },
    /// Each design's thumbnail `data-still` attribute (`mcview::tools::still_attr`: the still
    /// frame the viewer would search for, found ahead; the site puts it on its thumbnails), one
    /// `<path>\t<attribute>` line per design (`-` when the design does not load in the viewer).
    Still { designs: Vec<PathBuf> },
    /// The live viewer's visual QA at phone size in WebKit (WebKitGTK through the site's rig):
    /// each page's first live viewer on screen, screenshotted `--at` seconds after it is ready,
    /// its tags (no tag over another, the tick badge or the status line, under the bar or cut by
    /// the view's edge), its ticks over `--watch` s (it plays; a demo restarts), the picture's
    /// brightness, every input control pressed once (the build reacts), the console. Writes
    /// `<out>/<page>-<k>.png` (the viewer cropped), `<out>/sheet.png` (every page's viewer and
    /// findings) and `<out>/report.txt`.
    Qa {
        /// Pages of the site (paths in `--dir`, e.g. `circuits/lock/lock3-alt.html`).
        pages: Vec<String>,
        /// The site folder (`mctas site build --out`), served by the rig.
        #[arg(long)]
        dir: PathBuf,
        /// Every kind the site publishes: each circuit kind (up to 4 designs: smallest, middle,
        /// largest), every door, a clock per common mechanism (`--clocks`).
        #[arg(long)]
        kinds: bool,
        #[arg(long, default_value_t = 18)]
        clocks: usize,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value = "390,844")]
        window: String,
        /// Seconds after the viewer is ready to take the screenshots.
        #[arg(long, default_value = "2,8")]
        at: String,
        /// Seconds after ready to read the ticks (a demo's restart must fall in it).
        #[arg(long, default_value_t = 24.0)]
        watch: f64,
        /// Pages in parallel (a WebKit and its X display each).
        #[arg(long, default_value_t = 4)]
        shards: usize,
    },
    /// What a live viewer's frames cost (natively, the browser's renderer on lavapipe: a software
    /// GPU like the phone profile's WebKit): each design's live view at `--width x --height`
    /// device pixels (`--dpr`), `--warm` ticks in, then `--frames` frames of `--ticks` ticks each:
    /// the mesh and the draw (waited for) a frame, and the vertices on the GPU and uploaded.
    Framecost {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 798)]
        width: u32,
        #[arg(long, default_value_t = 960)]
        height: u32,
        #[arg(long, default_value_t = 2.0)]
        dpr: f32,
        #[arg(long, default_value_t = 400)]
        warm: u32,
        #[arg(long, default_value_t = 40)]
        frames: u32,
        #[arg(long, default_value_t = 1)]
        ticks: u32,
        /// Draw only the kept triangles the camera can see (`mcview::occlude`), as the browser does.
        #[arg(long)]
        cull: bool,
        /// With `--cull`: draw each frame whole too and count the pixels that differ (must be 0).
        #[arg(long)]
        check: bool,
        /// Each frame's staged mesh work stops after this many ms, as in the browser (a changed
        /// blocks' mesh is then built over several frames): how many were finished and over how
        /// many frames.
        #[arg(long)]
        budget: Option<f64>,
    },
    /// What a card costs (natively): each design's still (`Session::new` with `Options::still`)
    /// and its picture's mesh, timed; the slowest first. A card over `--budget` ms here is surely
    /// over it in the browser (the wasm build runs a few times slower).
    Cardcost {
        designs: Vec<PathBuf>,
        /// The time a card may take (ms).
        #[arg(long, default_value_t = 40.0)]
        budget: f64,
        /// Lines shown.
        #[arg(long, default_value_t = 12)]
        top: usize,
        /// The hand-drawn tiles only (default: the browser's atlas, the game's textures and block
        /// models, whose meshes are a few times heavier).
        #[arg(long)]
        procedural: bool,
    },
    /// What a page's view costs natively when it loads and when it is played, step by step as the
    /// browser runs them: the still and its camera, the session the judge seeds and the cycle
    /// speed pick (at once, then over frames as `Player::wake` builds them: the longest slice is
    /// what one browser frame would run, a few times slower there), the first picture, and the
    /// slowest of the next `--ticks` frames.
    Wakecost {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 100)]
        ticks: u32,
        #[arg(long)]
        procedural: bool,
        /// Also print every slice of the wake over frames (ms).
        #[arg(long)]
        slices: bool,
        /// At the end, the `N` slowest to their first frame (the load, the still and its camera,
        /// and the first picture's mesh), the slowest first.
        #[arg(long)]
        slowest: Option<usize>,
    },
    /// Why a live viewer's default camera looks where it does (the reviewer's sweeper farm, seen
    /// from its station's end wall on a desktop): every candidate direction the viewer weighs for
    /// a viewer of `--size` CSS px (`scene::live_camera`), its score part by part, the pick
    /// marked. The game's atlas, as the browser's.
    Camera {
        designs: Vec<PathBuf>,
        /// The viewer's CSS size, `W,H` (a desktop page: 1217,480; a phone: 342,480).
        #[arg(long, default_value = "1217,480")]
        size: String,
        /// The hand-drawn tiles only.
        #[arg(long)]
        procedural: bool,
        /// One line per design: its picked direction only (diff two runs to see what a scoring
        /// change moves).
        #[arg(long)]
        picked: bool,
        /// The card's thumbnail camera instead (`scene::thumb_camera`: the still, the thumbnail's
        /// window; `--size` the card's picture).
        #[arg(long)]
        thumb: bool,
    },
    /// No page or card view may block the page (vjeux 2026-10-21, "Freezes my browser"): each
    /// page in headless Chromium loads for `--load` s (cards are stills, a page's view a still
    /// until played), then every live view's play is pressed for `--play` s. Fails when the page
    /// stops answering, when a main-thread task passes `--budget` ms, or when a view is never drawn.
    Stall {
        /// The pages' URLs.
        #[arg(required = true)]
        urls: Vec<String>,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// The CPU slowed this many times.
        #[arg(long, default_value_t = 1.0)]
        cpu: f64,
        #[arg(long, default_value_t = 8.0)]
        load: f64,
        #[arg(long, default_value_t = 4.0)]
        play: f64,
        /// The longest main-thread task allowed (ms).
        #[arg(long, default_value_t = 200.0)]
        budget: f64,
        /// Print every main-thread task over this many ms, with what the viewer logged while it
        /// ran (its `?mcview-debug` steps, timed from the task's start).
        #[arg(long)]
        show: Option<f64>,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// A page on a phone profile in headless Chromium (CSS window size, device scale, touch, an
    /// iPhone user agent, the CPU slowed `--cpu` times): the time to its cards and to its first
    /// screen of 3D views drawn, long tasks and slow frames; then a scroll at `--speed` px/s for
    /// `--scroll` s: its frame times, the views drawn meanwhile (and how many already off screen),
    /// and how long the views on screen when it stops take to be drawn.
    Perf {
        /// The pages' URLs (Chromium), or their paths in the `--dir` site folder (WebKit): with
        /// several, each one's report, then a table of their first frames, the slowest first.
        #[arg(required = true)]
        urls: Vec<String>,
        /// `chromium` (headless, the CPU slowed `--cpu` times) or `webkit` (WebKitGTK through
        /// WebKitWebDriver in an Xvfb display, an iPhone user agent, device scale 2).
        #[arg(long, default_value = "chromium")]
        engine: String,
        /// With `--engine webkit`: the site folder to serve.
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "375,812")]
        window: String,
        #[arg(long, default_value_t = 3.0)]
        scale: f32,
        #[arg(long, default_value_t = 4.0)]
        cpu: f64,
        /// Seconds to watch the page load.
        #[arg(long, default_value_t = 20.0)]
        load: f64,
        #[arg(long, default_value_t = 8.0)]
        scroll: f64,
        #[arg(long, default_value_t = 2500.0)]
        speed: f64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
        /// With `--engine webkit`: the viewer's trace (`?mcview-trace`) beside each load frame
        /// over `--trace-over` ms: what the viewer did in it.
        #[arg(long)]
        trace: bool,
        /// With `--trace`: the frames shown, those over this many ms (0: every frame).
        #[arg(long, default_value_t = 50.0)]
        trace_over: f64,
        /// With `--trace`: the page without `?mcview-debug` (a visitor's frames: no debug log
        /// lines nor layout reads; the trace's own spans only).
        #[arg(long)]
        trace_plain: bool,
    },
    /// A screenshot of a page in headless Chromium after some seconds of real time (playing
    /// viewers and demos advance), e.g. to check a live viewer's overlays.
    Snap {
        url: String,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 4.0)]
        seconds: f64,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Scroll the page down this many CSS px first.
        #[arg(long, default_value_t = 0.0)]
        scroll: f64,
        /// Also evaluate this JavaScript expression in the page at the end and print its value
        /// (diagnostics: a WebGL limit, an element's state).
        #[arg(long)]
        eval: Option<String>,
        /// Evaluate this JavaScript after `--before-at` seconds, then wait `--seconds` more
        /// before the screenshot (an in-place swap: set `data-keep-camera` and `data-src`).
        #[arg(long)]
        before: Option<String>,
        #[arg(long, default_value_t = 4.0)]
        before_at: f64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// A page's first live viewer loses its WebGL context and gets it back
    /// (WEBGL_lose_context): it must draw again.
    Lose {
        url: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// The site's in-place design swap on a page's first live viewer: orbit, set
    /// `data-keep-camera` then `data-src` to `--to`: the new design must show under the same
    /// camera, the viewer marked `mcview-live-src`.
    Swap {
        url: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Mostly-black check: natively, every design given rendered offscreen (its still, small);
    /// with `--url`, every viewer of a page in headless Chromium through the real bundle (the
    /// WebGL path). Fails on any frame with a mean luma under `--min` or mostly near-black.
    Luma {
        designs: Vec<PathBuf>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, default_value_t = 40.0)]
        min: f64,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// A mob farm's rate in the 3D view (vjeux's brief, 2026-11-08: the view's golem count over
    /// fixed days must match the card): the judge's every run (its 8 builds, 4 turns x 2 places,
    /// each 8 seeds of 5 days) played through the viewer's own session, natively; their mean iron
    /// and golems an hour against the card's (the archived `# CIRCUIT:` line), and the view's
    /// first run (the page's) alone. Fails when a build's farm is not the judge's or the means
    /// differ from the card's.
    Mobrate {
        designs: Vec<PathBuf>,
        /// Runs at once.
        #[arg(long, default_value_t = 32)]
        threads: usize,
    },
    /// One of a mob farm's judge runs (its build `--build` of the judge's 8, `--seed`) played both
    /// ways, the judge's runner (`farm::Run`, with the card's hand step) and the viewer's session,
    /// compared every tick (each world's time, mob random and entities: kind, position, motion):
    /// the first tick they part and how, or that they agree to the end.
    Mobdiff {
        design: PathBuf,
        #[arg(long, default_value_t = 0)]
        build: usize,
        #[arg(long)]
        seed: i64,
        #[arg(long, default_value_t = 120000)]
        ticks: u32,
    },
    /// Fit the default camera to the real-game pictures: renders each design under camera tweaks
    /// (zoom, pitch, yaw, height, distance) and prints the best by mean pixel difference.
    Fit {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "designs/clocks/img")]
        img_dir: PathBuf,
        /// Zooms to try (comma-separated).
        #[arg(long, default_value = "1.0,1.1,1.2,1.3")]
        zoom: String,
        #[arg(long, default_value = "-2,0,2")]
        pitch: String,
        #[arg(long, default_value = "-2,0,2")]
        yaw: String,
        #[arg(long, default_value = "0")]
        dy: String,
        #[arg(long, default_value = "1.0")]
        dist: String,
    },
    /// Texture contact sheet: PNGs scaled up (nearest) in a grid, in the given order (row by row),
    /// to pick textures by eye. With `--crop x,y,w,h`: that part of each picture instead, at its
    /// own size (any pictures: a WebKit film's frames, to read many in one).
    Sheet {
        pngs: Vec<PathBuf>,
        #[arg(long, short)]
        out: PathBuf,
        /// How many times bigger (nearest): textures 8 by default, `--crop` parts 1.
        #[arg(long)]
        scale: Option<u32>,
        #[arg(long, default_value_t = 8)]
        cols: u32,
        #[arg(long)]
        crop: Option<String>,
    },
    /// The viewer's own textures as PNGs (`<out>/<tile>.png`), to compare with the game's.
    Tiles {
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Colors of textures: mean and a k-means palette with shares (`--tint rrggbb` multiplies,
    /// for the game's grayscale grass and leaves).
    Palette {
        pngs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        k: usize,
        #[arg(long)]
        tint: Option<String>,
        /// Only this rectangle `x,y,w,h` of each image (PNG or JPEG: a region of a real-game
        /// picture); default: the first 16x16 (a texture's first frame).
        #[arg(long)]
        crop: Option<String>,
    },
    /// LOCAL REFERENCE ONLY (never commit or ship the output: Mojang's assets): the game's block
    /// textures from the client jar of `--version` (downloaded once into the version's cache
    /// directory) into `<out>/<name>.png`; every block texture, or only the given names.
    Reference {
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value = "26.3")]
        version: String,
        names: Vec<String>,
    },
    /// A grid of frames over time, as the browser animates them: a door from its lever flip, a
    /// clock from its still moment, `--step` game ticks apart (0.5 shows moving blocks mid-way).
    Strip {
        design: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 6)]
        cols: u32,
        #[arg(long, default_value_t = 4)]
        rows: u32,
        #[arg(long, default_value_t = 0.5)]
        step: f32,
        #[arg(long, default_value_t = 320)]
        width: u32,
        #[arg(long, default_value_t = 240)]
        height: u32,
        /// The hand-drawn tiles only (default: the game's textures).
        #[arg(long)]
        procedural: bool,
    },
}

/// The tiles: the game's textures over the hand-drawn ones (the vendored atlas in
/// `web/textures/26.3`) and the game's own block models (`--procedural`: the hand-drawn tiles and the
/// viewer's own models only; `MCVIEW_NO_GAME_MODELS=1`: the game's textures on the viewer's
/// models).
fn atlas(procedural: bool) -> mcview::atlas::Atlas {
    let mut a = mcview::atlas::Atlas::build();
    if !procedural {
        match mcview::tools::vanilla_assets() {
            Ok((png, idx, defs, models)) => {
                match a.apply_vanilla_png(&png, &idx) {
                    Ok(n) => eprintln!("the game's textures on {n} tiles"),
                    Err(e) => eprintln!("the game's textures unavailable ({e}): hand-drawn tiles"),
                }
                if std::env::var_os("MCVIEW_NO_GAME_MODELS").is_none() {
                    let r = mcview::tools::decode_png(&png).and_then(|(rgba, w, h)| a.set_game(&rgba, w, h, &idx, &defs, &models));
                    match r {
                        Ok(n) => eprintln!("the game's block models ({n} textures)"),
                        Err(e) => eprintln!("the game's block models unavailable ({e}): the viewer's models"),
                    }
                    // the entity and item textures (boats, minecarts, armor stands, dropped items)
                    let r = mcview::tools::vanilla_entity_assets().and_then(|((ep, ei), (ip, ii))| {
                        let (er, ew, eh) = mcview::tools::decode_png(&ep)?;
                        let (ir, iw, ih) = mcview::tools::decode_png(&ip)?;
                        a.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii))
                    });
                    if let Err(e) = r {
                        eprintln!("the game's entity textures unavailable ({e}): the viewer's stand-ins");
                    }
                    // the particle sprites (an explosion's frames), below them
                    let r = mcview::tools::vanilla_particle_assets().and_then(|s| a.add_game_particles(&s));
                    if let Err(e) = r {
                        eprintln!("the game's particle sprites unavailable ({e}): the viewer's stand-ins");
                    }
                }
            }
            Err(e) => eprintln!("the game's textures unavailable ({e}): hand-drawn tiles"),
        }
    }
    a
}

pub fn run(_version: &str, cmd: WebCmd) -> Result<()> {
    match cmd {
        WebCmd::Shot { designs, out, width, height, tick, flips, yaw, pitch, procedural, live, dpr, crop, day_time, rain, thunder, against } => cmd_shot(&designs, &out, mcview::tools::ShotOptions { width, height, tick, flips, yaw, pitch, picture: false, floor_margin: None, live, dpr, as_written: false, full_frame: false }, procedural, crop.as_deref(), day_time.map(|t| mcview::daylight::at(t, rain, thunder)), against.as_deref()),
        WebCmd::Check { designs, horizon, quiet, wasm, unverified } => cmd_check(&designs, horizon, quiet, wasm.as_deref(), unverified),
        WebCmd::Bundle { out } => cmd_bundle(&out),
        WebCmd::Single { page, out } => single::single_page(&page, &out),
        WebCmd::Demo { designs, out, viewer, live, autoplay } => cmd_demo(&designs, &out, &viewer, live, autoplay),
        WebCmd::Serve { dir, port } => cmd_serve(&dir, port),
        WebCmd::Compare { designs, img_dir, out, procedural, zoom, floor_margin, still, before } => cmd_compare(&designs, &img_dir, &out, procedural, zoom.as_deref(), floor_margin, still, before.as_deref()),
        WebCmd::Snapshot { designs, out } => {
            std::fs::create_dir_all(&out)?;
            let mut n = 0;
            for p in &designs {
                let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
                match mcview::tools::snapshot(&text) {
                    Ok((blocks, entities, what)) => {
                        // at the height the real game pictures it (`clock shot`: the lowest cell at
                        // y = 72), so both pick the same random variant of each block (a stone's
                        // turn, the grass's) from its position
                        let low = blocks.iter().map(|(p, _)| p[1]).chain(entities.iter().map(|e| e.pos[1].floor() as i32)).min().unwrap_or(72);
                        let dy = 72 - low;
                        let d = mctas::design::Design {
                            name: stem.clone(),
                            blocks: blocks.into_iter().map(|(p, s)| ([p[0], p[1] + dy, p[2]], s)).collect(),
                            entities: entities.into_iter().map(|e| mctas::design::EntitySpec { kind: e.kind, pos: [e.pos[0], e.pos[1] + dy as f64, e.pos[2]], nbt: e.nbt }).collect(),
                            ..Default::default()
                        };
                        let file = out.join(format!("{stem}.txt"));
                        std::fs::write(&file, format!("# snapshot of {} in the viewer ({what}): its blocks as drawn there\n{}", p.display(), d.to_text()))?;
                        println!("{stem}: {} blocks, {} entities ({what}) -> {}", d.blocks.len(), d.entities.len(), file.display());
                        n += 1;
                    }
                    Err(e) => println!("{stem}: SKIP {e}"),
                }
            }
            println!("web snapshot: {n} of {} designs", designs.len());
            Ok(())
        }
        WebCmd::Tiles { out } => {
            let atlas = mcview::atlas::Atlas::build();
            std::fs::create_dir_all(&out)?;
            let mut names: Vec<(&str, u16)> = atlas.names.iter().map(|(k, v)| (*k, *v)).collect();
            names.sort();
            for (name, layer) in &names {
                let px = &atlas.mips[0][*layer as usize * 16 * 16 * 4..(*layer as usize + 1) * 16 * 16 * 4];
                mcview::render::headless::write_png(&out.join(format!("{name}.png")), 16, 16, px).map_err(anyhow::Error::msg)?;
            }
            println!("{} tiles in {}", names.len(), out.display());
            Ok(())
        }
        WebCmd::Entities { design, trace, from, to } => {
            let text = std::fs::read_to_string(&design)?;
            let mut s = mcview::Session::new(&text, &mcview::Options::default()).map_err(anyhow::Error::msg)?;
            let replay = match &trace {
                Some(p) => Some(mcview::replay::Replay::parse(&std::fs::read_to_string(p)?).map_err(anyhow::Error::msg)?),
                None => None,
            };
            while s.tick < from {
                s.step();
            }
            let mut worst = 0f32;
            while s.tick <= to {
                // the simulated positions at the end of the tick (the trace's convention; the
                // renderer draws a minecart on a rail at the rail's line instead, like
                // AbstractMinecartRenderer.oldRender, so the drawn pose is not compared)
                let mine: Vec<(String, [f32; 3], f32)> = s
                    .world
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed)
                    .zip(s.entities(1.0))
                    .map(|(e, d)| (d.kind, [e.pos.x as f32, e.pos.y as f32, e.pos.z as f32], e.y_rot))
                    .collect();
                let line: Vec<String> = mine.iter().map(|(k, p, yaw)| format!("{k} ({:.4}, {:.4}, {:.4}) yaw {yaw:.1}", p[0], p[1], p[2])).collect();
                println!("t {}: {}", s.tick, line.join("; "));
                // what the viewer draws besides redsim's entities: falling blocks, primed TNT
                let extra: Vec<String> = s.entities(1.0).iter().filter(|e| e.kind.starts_with("primed:") || e.kind.starts_with("falling:")).map(|e| format!("{} ({:.3}, {:.3}, {:.3}){}", e.kind.split('[').next().unwrap_or(""), e.pos[0], e.pos[1], e.pos[2], if e.fuse > 0.0 { format!(" fuse {:.0}", e.fuse) } else { String::new() })).collect();
                if !extra.is_empty() {
                    println!("  drawn: {}", extra.join("; "));
                }
                if let Some(r) = &replay {
                    let theirs = r.entities_at(s.tick);
                    let mut parts = Vec::new();
                    for (k, e) in theirs.iter().enumerate() {
                        let d = mine.get(k).map(|(_, m, _)| ((m[0] - e.pos[0]).powi(2) + (m[1] - e.pos[1]).powi(2) + (m[2] - e.pos[2]).powi(2)).sqrt());
                        if let Some(d) = d {
                            worst = worst.max(d);
                        }
                        parts.push(format!("{} ({:.4}, {:.4}, {:.4}) yaw {:.1}{}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.yaw, d.map(|d| format!(" [{d:.4}]")).unwrap_or_default()));
                    }
                    println!("  game: {}", parts.join("; "));
                }
                s.step();
            }
            if replay.is_some() {
                println!("largest distance to the game: {worst:.4} blocks over ticks {from}..={to}");
            }
            Ok(())
        }
        WebCmd::GameTexture { out } => {
            let a = atlas(false);
            let g = a.game.as_ref().ok_or_else(|| anyhow::anyhow!("no game assets"))?;
            mcview::render::headless::write_png(&out, g.width, g.height, &g.mips[0]).map_err(anyhow::Error::msg)?;
            let mut ent: Vec<_> = g.tex.iter().filter(|(k, _)| k.starts_with("entity/")).map(|(k, t)| format!("{k} {:?}", t.rect)).collect();
            ent.sort();
            for l in &ent {
                println!("{l}");
            }
            println!("{} ({}x{}, entities packed: {})", out.display(), g.width, g.height, g.entities);
            Ok(())
        }
        WebCmd::Palette { pngs, k, tint, crop } => {
            let tint = match tint {
                Some(h) => {
                    let v = u32::from_str_radix(h.trim_start_matches('#'), 16)?;
                    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
                }
                None => None,
            };
            let crop: Option<Vec<u32>> = crop.map(|c| c.split(',').filter_map(|w| w.trim().parse().ok()).collect());
            for p in &pngs {
                let (px, w, h) = mcview::tools::decode_image(&std::fs::read(p)?).map_err(anyhow::Error::msg)?;
                let (x0, y0, cw, ch) = match crop.as_deref() {
                    Some([x, y, cw, ch]) => (*x, *y, *cw, *ch),
                    _ => (0, 0, w.min(16), h.min(16)),
                };
                let mut sel = Vec::new();
                for y in y0..(y0 + ch).min(h) {
                    for x in x0..(x0 + cw).min(w) {
                        let o = ((y * w + x) * 4) as usize;
                        sel.extend_from_slice(&px[o..o + 4]);
                    }
                }
                let (mean, pal) = mcview::tools::palette(&sel, k, tint);
                let hex = |c: [u8; 3]| format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                let parts: Vec<String> = pal.iter().map(|(c, f)| format!("{} {:.0}%", hex(*c), f * 100.0)).collect();
                println!("{:<32} mean {}  {}", p.file_stem().unwrap_or_default().to_string_lossy(), hex(mean), parts.join("  "));
            }
            Ok(())
        }
        WebCmd::Fit { designs, img_dir, zoom, pitch, yaw, dy, dist } => {
            let nums = |s: &str| -> Vec<f32> { s.split(',').filter_map(|w| w.trim().parse().ok()).collect() };
            let mut tweaks = Vec::new();
            for &z in &nums(&zoom) {
                for &p in &nums(&pitch) {
                    for &y in &nums(&yaw) {
                        for &d in &nums(&dy) {
                            for &ds in &nums(&dist) {
                                tweaks.push(mcview::tools::CamTweak { zoom: z, dpitch: p, dyaw: y, dy: d, dist: ds });
                            }
                        }
                    }
                }
            }
            let mut cases = Vec::new();
            for d in &designs {
                let stem = d.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let jpg = img_dir.join(format!("{stem}.jpg"));
                match std::fs::read(&jpg) {
                    Ok(b) => cases.push((std::fs::read_to_string(d)?, b)),
                    Err(_) => eprintln!("{stem}: no picture {}", jpg.display()),
                }
            }
            let atlas = atlas(false);
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let r = mcview::tools::fit(&mut off, &atlas, &cases, 400, 225, &tweaks).map_err(anyhow::Error::msg)?;
            for (d, t) in r.iter().take(12) {
                println!("{d:6.2}  zoom {:.3} pitch {:+.1} yaw {:+.1} dy {:+.2} dist {:.3}", t.zoom, t.dpitch, t.dyaw, t.dy, t.dist);
            }
            Ok(())
        }
        WebCmd::Lose { url, window, browser } => crate::cdp::cmd_lose(&url, &window, &browser),
        WebCmd::Swap { url, to, window, browser } => crate::cdp::cmd_swap(&url, &to, &window, &browser),
        WebCmd::Mobrate { designs, threads } => cmd_mobrate(&designs, threads),
        WebCmd::Mobdiff { design, build, seed, ticks } => cmd_mobdiff(&design, build, seed, ticks),
        WebCmd::Luma { designs, url, min, window, browser } => match url {
            Some(u) => crate::cdp::cmd_luma(&u, &window, min, &browser),
            None => cmd_luma_native(&designs, min),
        },
        WebCmd::Still { designs } => {
            for p in designs {
                let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
                println!("{}\t{}", p.display(), mcview::tools::still_attr(&text).unwrap_or_else(|| "-".into()));
            }
            Ok(())
        }
        WebCmd::Recqa { pages, dir, out, engines, window, scale, timeout, browser, chrome } => crate::recqa::cmd_recqa(&dir, &pages, &out, &engines, &window, scale, timeout, &browser, chrome.as_deref()),
        WebCmd::Qa { pages, dir, kinds, clocks, out, window, at, watch, shards } => crate::qa::cmd_qa(&dir, &pages, kinds, clocks, &out, &window, &at, watch, shards),
        WebCmd::Cardcost { designs, budget, top, procedural } => cmd_cardcost(&designs, budget, top, procedural),
        WebCmd::Framecost { designs, width, height, dpr, warm, frames, ticks, cull, check, budget } => {
            let atlas = atlas(false);
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let mut bad = 0;
            for d in &designs {
                let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
                match mcview::tools::frame_cost(&mut off, &atlas, &text, width, height, dpr, warm, frames, ticks, cull, check, budget) {
                    Ok(c) => {
                        println!(
                            "{}: {width}x{height}: mesh {:.2} ms, draw {:.2} ms a frame ({} frames of {ticks} ticks; the blocks' mesh rebuilt {} times); vertices: {} on the GPU, {} solid + {} translucent uploaded a frame",
                            d.display(),
                            c.mesh_ms,
                            c.draw_ms,
                            c.frames,
                            c.rebuilds,
                            c.fixed,
                            c.solid,
                            c.translucent
                        );
                        if cull {
                            println!("  culled: {} of {} kept triangles drawn, worked out {} times in {:.1} ms each; carried over {} times (the last {} triangles)", c.kept, c.tested, c.culls, c.cull_ms, c.carried, c.carried_kept);
                        }
                        if let Some(b) = budget {
                            println!("  staged at {b} ms a frame: {} blocks' meshes finished, {} frames of {} working on one", c.built, c.staged_frames, c.frames);
                        }
                        if check {
                            println!("  {}: the most pixels a culled frame differs by from the same frame drawn whole: {}", if c.differing == 0 { "ok" } else { "FAIL" }, c.differing);
                            println!("  {}: the most pixels a frame differs by from it with the sky drawn first: {}", if c.sky_differing == 0 { "ok" } else { "FAIL" }, c.sky_differing);
                            bad += (c.differing > 0 || c.sky_differing > 0) as usize;
                        }
                    }
                    Err(e) => {
                        println!("{}: ERROR {e}", d.display());
                        bad += 1;
                    }
                }
            }
            if bad > 0 {
                anyhow::bail!("{bad} design(s) failed");
            }
            Ok(())
        }
        WebCmd::Wakecost { designs, ticks, procedural, slices, slowest } => cmd_wakecost(&designs, ticks, procedural, slices, slowest),
        WebCmd::Camera { designs, size, procedural, picked, thumb } => cmd_camera(&designs, &size, procedural, picked, thumb),
        WebCmd::Stall { urls, window, scale, cpu, load, play, budget, show, browser } => crate::cdp::cmd_stall(&urls, &window, scale, cpu, load, play, budget, show, &browser),
        WebCmd::Perf { urls, engine, dir, window, scale, cpu, load, scroll, speed, browser, trace, trace_over, trace_plain } => crate::cdp::cmd_perf(&urls, &engine, dir.as_deref(), &window, scale, cpu, load, scroll, speed, &browser, trace.then_some((trace_over, trace_plain))),
        WebCmd::Snap { url, out, seconds, window, scale, scroll, eval, before, before_at, browser } => crate::cdp::cmd_snap(&url, &window, scale, seconds, scroll, eval.as_deref(), before.as_deref().map(|b| (b, before_at)), &out, &browser),
        WebCmd::Click { url, window, scale, touch, hold, wait, browser } => {
            if hold {
                crate::cdp::cmd_hold(&url, &window, scale, touch, &browser)
            } else {
                crate::cdp::cmd_click(&url, &window, scale, touch, wait, &browser)
            }
        }
        WebCmd::Record { url, out, index, window, scale, timeout, browser } => crate::cdp::cmd_record(&url, &out, index, &window, scale, timeout, &browser),
        WebCmd::Layout { url, seconds, window, scale, min_ticks, browser } => crate::cdp::cmd_layout(&url, seconds, &window, scale, min_ticks, &browser),
        WebCmd::Reference { out, version, names } => cmd_reference(&out, &version, &names),
        WebCmd::Sheet { pngs, out, scale, cols, crop: Some(c) } => cmd_sheet(&pngs, &out, Some(&c), cols, scale.unwrap_or(1) as f32),
        WebCmd::Sheet { pngs, out, scale, cols, crop: None } => {
            let scale = scale.unwrap_or(8);
            let (rgba, w, h) = mcview::tools::sheet(&pngs, scale, cols).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({} textures)", out.display(), pngs.len());
            Ok(())
        }
        WebCmd::Strip { design, out, cols, rows, step, width, height, procedural } => {
            let atlas = atlas(procedural);
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let text = std::fs::read_to_string(&design)?;
            let (rgba, w, h) = mcview::tools::strip(&mut off, &atlas, &text, cols, rows, step, width, height).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({cols}x{rows} frames, {step} gt apart)", out.display());
            Ok(())
        }
    }
}

/// Every design's still rendered offscreen (small): fails on a mostly black frame.
fn cmd_luma_native(designs: &[PathBuf], min: f64) -> Result<()> {
    let atlas = atlas(false);
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    let (mut n, mut dark, mut errors) = (0usize, Vec::new(), 0usize);
    let mut worst = (f64::MAX, String::new());
    for d in designs {
        let Ok(text) = std::fs::read_to_string(d) else { continue };
        match mcview::tools::shot(&mut off, &atlas, &text, &mcview::tools::ShotOptions { width: 160, height: 90, ..Default::default() }) {
            Ok((rgba, _)) => {
                n += 1;
                let (mean, black) = crate::cdp::luma_stats(&rgba);
                if mean < worst.0 {
                    worst = (mean, d.display().to_string());
                }
                if mean < min || black > 0.5 {
                    dark.push(format!("{}: mean luma {mean:.0}, {:.0}% near black", d.display(), black * 100.0));
                }
            }
            Err(e) => {
                // a file the viewer cannot read at all (not a design): reported, not a frame
                errors += 1;
                if errors <= 5 {
                    println!("{}: {e}", d.display());
                }
            }
        }
    }
    for l in &dark {
        println!("DARK {l}");
    }
    println!("web luma: {n} designs rendered natively, {} dark, {errors} unreadable (darkest {:.0}: {})", dark.len(), worst.0, worst.1);
    if !dark.is_empty() {
        bail!("{} designs render mostly black", dark.len());
    }
    Ok(())
}

/// One design against its real picture (`web compare`): our frame of the whole window and the
/// floor alone (`mcview::tools::picture_frames`), each through the pictures' own encoder
/// (`clock::shot::encode_picture`, in `frames`: the same crop, resampling and JPEG), then compared.
#[allow(clippy::too_many_arguments)]
fn compare_one(off: &mut mcview::render::headless::Offscreen, atlas: &mcview::atlas::Atlas, text: &str, jpg: &[u8], floor_margin: Option<f32>, still: bool, frames: &Path, stem: &str) -> Result<mcview::tools::Comparison> {
    let (real, w, h) = mcview::tools::decode_jpeg(jpg).map_err(anyhow::Error::msg)?;
    let (frame, floor, note) = mcview::tools::picture_frames(off, atlas, text, floor_margin, still).map_err(anyhow::Error::msg)?;
    std::fs::create_dir_all(frames)?;
    let (fw, fh) = mcview::tools::PICTURE_FRAME;
    let mut encoded = Vec::new();
    for (name, px) in [("view", &frame), ("floor", &floor)] {
        let png = frames.join(format!("{stem}.{name}.png"));
        let out = frames.join(format!("{stem}.{name}.jpg"));
        mcview::render::headless::write_png(&png, fw, fh, px).map_err(anyhow::Error::msg)?;
        clock::shot::encode_picture(&png, &out, w)?;
        let (rgb, ew, eh) = mcview::tools::decode_jpeg(&std::fs::read(&out)?).map_err(anyhow::Error::msg)?;
        if (ew, eh) != (w, h) {
            bail!("our encoded frame is {ew}x{eh}, the picture {w}x{h}");
        }
        let _ = std::fs::remove_file(&png);
        encoded.push(rgb);
    }
    let floor = encoded.pop().unwrap_or_default();
    let ours = encoded.pop().unwrap_or_default();
    Ok(mcview::tools::compare_pictures(&real, &ours, &floor, w, h, note))
}

#[allow(clippy::too_many_arguments)]
fn cmd_compare(designs: &[PathBuf], img_dir: &Path, out: &Path, procedural: bool, zoom: Option<&str>, floor_margin: Option<f32>, still: bool, before: Option<&Path>) -> Result<()> {
    let atlas = atlas(procedural);
    let zoom: Option<Vec<u32>> = zoom.map(|z| z.split(',').filter_map(|v| v.trim().parse().ok()).collect());
    if zoom.as_ref().is_some_and(|z| z.len() != 4) {
        bail!("--zoom wants x,y,w,h");
    }
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    std::fs::create_dir_all(out)?;
    let (mut n, mut sum_px, mut sum_coarse, mut sum_build) = (0, 0.0, 0.0, 0.0);
    let mut sum_edge = (0.0f64, 0.0f64);
    // (stem, pixel diff, coarse diff, build diff, the comparison at half size, its size)
    let mut rows: Vec<(String, f64, f64, f64, Vec<u8>, u32, u32)> = Vec::new();
    // each row's edge offset (`mcview::tools::edge_offset`)
    let mut edges: Vec<(f32, f32)> = Vec::new();
    for d in designs {
        let stem = d.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let jpg = img_dir.join(format!("{stem}.jpg"));
        let Ok(bytes) = std::fs::read(&jpg) else {
            println!("{stem}: no picture at {}", jpg.display());
            continue;
        };
        let text = std::fs::read_to_string(d)?;
        match compare_one(&mut off, &atlas, &text, &bytes, floor_margin, still, &out.join(".frames"), &stem) {
            Ok(c) => {
                let path = out.join(format!("{stem}.cmp.png"));
                mcview::render::headless::write_png(&path, c.width, c.height, &c.rgba).map_err(anyhow::Error::msg)?;
                if let Some(z) = &zoom {
                    // the region of the first two panels (real left, viewer right), 4x nearest
                    let (zx, zy, zw, zh, s) = (z[0], z[1], z[2], z[3], 4u32);
                    let half = c.width / 3;
                    let (ow, oh) = (2 * zw * s + 8, zh * s);
                    let mut img = vec![30u8; (ow * oh * 4) as usize];
                    for (k, x_off) in [(0u32, 0u32), (1, zw * s + 8)] {
                        for y in 0..zh * s {
                            for x in 0..zw * s {
                                let (sx, sy) = ((zx + x / s).min(half - 1) + k * half, (zy + y / s).min(c.height - 1));
                                let si = ((sy * c.width + sx) * 4) as usize;
                                let di = ((y * ow + x_off + x) * 4) as usize;
                                img[di..di + 4].copy_from_slice(&c.rgba[si..si + 4]);
                            }
                        }
                    }
                    mcview::render::headless::write_png(&out.join(format!("{stem}.zoom.png")), ow, oh, &img).map_err(anyhow::Error::msg)?;
                    // the region's mean colour in each picture
                    let mean = |k: u32| -> [f64; 3] {
                        let mut s = [0f64; 3];
                        for y in zy..(zy + zh).min(c.height) {
                            for x in zx..(zx + zw).min(half) {
                                let i = ((y * c.width + x + k * half) * 4) as usize;
                                for ch in 0..3 {
                                    s[ch] += c.rgba[i + ch] as f64;
                                }
                            }
                        }
                        let n = ((zx + zw).min(half).saturating_sub(zx) * (zy + zh).min(c.height).saturating_sub(zy)).max(1) as f64;
                        s.map(|v| v / n)
                    };
                    let (r, m) = (mean(0), mean(1));
                    println!("{stem}: zoom mean colour: real {:.0},{:.0},{:.0}, viewer {:.0},{:.0},{:.0}", r[0], r[1], r[2], m[0], m[1], m[2]);
                }
                println!(
                    "{stem}: build diff {:.1} ({} px), pixel diff {:.1}, coarse diff {:.1}, bottom {:.1}, top {:.1}, edges {:+.3},{:+.3} px ({}) -> {}",
                    c.build_diff, c.build_px, c.pixel_diff, c.coarse_diff, c.bottom_diff, c.top_diff, c.edge_offset.0, c.edge_offset.1, c.note, path.display()
                );
                n += 1;
                sum_px += c.pixel_diff;
                sum_coarse += c.coarse_diff;
                sum_build += c.build_diff;
                sum_edge.0 += c.edge_offset.0 as f64;
                sum_edge.1 += c.edge_offset.1 as f64;
                rows.push((stem.clone(), c.pixel_diff, c.coarse_diff, c.build_diff, mcview::tools::lanczos_resize(&c.rgba, c.width, c.height, c.width / 2, c.height / 2), c.width / 2, c.height / 2));
                edges.push(c.edge_offset);
            }
            Err(e) => println!("{stem}: ERROR {e}"),
        }
    }
    if n > 0 {
        println!(
            "web compare: {n} pictures, mean build diff {:.2}, mean pixel diff {:.2}, mean coarse diff {:.2} (0 = identical, 255 = opposite), mean edge offset {:+.3},{:+.3} px",
            sum_build / n as f64,
            sum_px / n as f64,
            sum_coarse / n as f64,
            sum_edge.0 / n as f64,
            sum_edge.1 / n as f64
        );
        // every comparison at half size, two across, each named with its differences (the real
        // game, the viewer, where they differ), and the numbers as a table
        let (cw, ch) = (rows[0].5, rows[0].6);
        let cols = 2u32;
        let lab = 16u32;
        let nrows = (rows.len() as u32).div_ceil(cols);
        let mut sheet = mctas::mcfont::Image::new(cols * (cw + 8), nrows * (ch + lab + 6), [24, 24, 28, 255]);
        let font = mctas::mcfont::Font::minecraft("26.3").ok();
        let mut tsv = String::from("design\tbuild_diff\tpixel_diff\tcoarse_diff\tedge_dx\tedge_dy\n");
        for (k, (stem, px, coarse, build, rgba, w, h)) in rows.iter().enumerate() {
            let (ox, oy) = ((k as u32 % cols) * (cw + 8), (k as u32 / cols) * (ch + lab + 6));
            sheet.draw(&mctas::mcfont::Image { w: *w, h: *h, px: rgba.clone() }, ox as i64, oy as i64, 1);
            if let Some(f) = &font {
                f.draw(&mut sheet, ox as i64 + 2, (oy + ch + 3) as i64, &format!("{stem}: build {build:.1}, pixel {px:.1}, coarse {coarse:.1}"), 1, [230, 230, 230], true);
            }
            let e = edges[k];
            tsv += &format!("{stem}\t{build:.2}\t{px:.2}\t{coarse:.2}\t{:.3}\t{:.3}\n", e.0, e.1);
        }
        tsv += &format!("mean\t{:.2}\t{:.2}\t{:.2}\t{:.3}\t{:.3}\n", sum_build / n as f64, sum_px / n as f64, sum_coarse / n as f64, sum_edge.0 / n as f64, sum_edge.1 / n as f64);
        mcview::render::headless::write_png(&out.join("sheet.png"), sheet.w, sheet.h, &sheet.px).map_err(anyhow::Error::msg)?;
        std::fs::write(out.join("report.tsv"), tsv)?;
        println!("web compare: {} and {}", out.join("sheet.png").display(), out.join("report.tsv").display());
        if let Some(dir) = before {
            before_after(dir, out, &rows)?;
        }
    }
    Ok(())
}

/// `web compare --before`: each design's real picture, the earlier run's view (the middle third of
/// its `<stem>.cmp.png`) and this run's, three across at half size, two designs a row, named with
/// both build differences (from the earlier `report.tsv`); and both means.
fn before_after(dir: &Path, out: &Path, rows: &[(String, f64, f64, f64, Vec<u8>, u32, u32)]) -> Result<()> {
    let old: std::collections::HashMap<String, f64> = std::fs::read_to_string(dir.join("report.tsv"))
        .with_context(|| format!("reading {}/report.tsv", dir.display()))?
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.parse().ok()?))
        })
        .collect();
    // this run's rows hold their three panels at half size: real, view, difference
    let (pw, ph) = (rows[0].5 / 3, rows[0].6);
    let cols = 2u32;
    let lab = 16u32;
    let cell_w = 3 * pw + 8;
    let n = rows.len() as u32;
    let mut sheet = mctas::mcfont::Image::new(cols * cell_w, n.div_ceil(cols) * (ph + lab + 6), [24, 24, 28, 255]);
    let font = mctas::mcfont::Font::minecraft("26.3").ok();
    let (mut sum_old, mut sum_new, mut both) = (0.0, 0.0, 0usize);
    for (k, (stem, _, _, build, rgba, w, h)) in rows.iter().enumerate() {
        let (ox, oy) = ((k as u32 % cols) * cell_w, (k as u32 / cols) * (ph + lab + 6));
        let panel = |px: &[u8], pw_full: u32, x0: u32, x1: u32, hh: u32| -> mctas::mcfont::Image {
            let mut p = mctas::mcfont::Image::new(x1 - x0, hh, [0, 0, 0, 255]);
            for y in 0..hh {
                let s = ((y * pw_full + x0) * 4) as usize;
                let d = (y * (x1 - x0) * 4) as usize;
                p.px[d..d + ((x1 - x0) * 4) as usize].copy_from_slice(&px[s..s + ((x1 - x0) * 4) as usize]);
            }
            p
        };
        // real, then the earlier view, then this view
        sheet.draw(&panel(rgba, *w, 0, pw, *h), ox as i64, oy as i64, 1);
        let earlier = mctas::mcfont::Image::load_png(&dir.join(format!("{stem}.cmp.png"))).ok().map(|img| {
            let third = img.w / 3;
            let mid = panel(&img.px, img.w, third, 2 * third, img.h);
            mctas::mcfont::Image { w: pw, h: ph, px: mcview::tools::lanczos_resize(&mid.px, mid.w, mid.h, pw, ph) }
        });
        if let Some(e) = &earlier {
            sheet.draw(e, (ox + pw) as i64, oy as i64, 1);
        }
        sheet.draw(&panel(rgba, *w, pw, 2 * pw, *h), (ox + 2 * pw) as i64, oy as i64, 1);
        let was = old.get(stem).copied();
        if let Some(v) = was {
            sum_old += v;
            sum_new += build;
            both += 1;
        }
        if let Some(f) = &font {
            let text = match was {
                Some(v) => format!("{stem}: build diff {v:.1} -> {build:.1} (real, before, after)"),
                None => format!("{stem}: build diff {build:.1} (no earlier run)"),
            };
            f.draw(&mut sheet, ox as i64 + 2, (oy + ph + 3) as i64, &text, 1, [230, 230, 230], true);
        }
    }
    mcview::render::headless::write_png(&out.join("before-after.png"), sheet.w, sheet.h, &sheet.px).map_err(anyhow::Error::msg)?;
    if both > 0 {
        println!("web compare: {both} designs in both runs: mean build diff {:.2} before, {:.2} now -> {}", sum_old / both as f64, sum_new / both as f64, out.join("before-after.png").display());
    }
    Ok(())
}

/// A tool from PATH, else from ~/.cargo/bin.
fn tool(name: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let cargo_bin = home.join(".cargo/bin").join(name);
    if cargo_bin.exists() {
        return cargo_bin;
    }
    PathBuf::from(name)
}

fn cmd_bundle(out: &Path) -> Result<()> {
    let status = std::process::Command::new(tool("cargo"))
        .args(["build", "-p", "mcview-web", "--profile", "web", "--target", "wasm32-unknown-unknown"])
        .status()
        .context("running cargo (is the wasm32 target installed?)")?;
    if !status.success() {
        bail!("cargo build of mcview-web for wasm32 failed");
    }
    let wasm = Path::new("target/wasm32-unknown-unknown/web/mcview_web.wasm");
    if !wasm.exists() {
        bail!("{} missing: run from the repository root", wasm.display());
    }
    std::fs::create_dir_all(out)?;
    let status = std::process::Command::new(tool("wasm-bindgen"))
        .args(["--target", "web", "--out-name", "mcview", "--no-typescript", "--out-dir"])
        .arg(out)
        .arg(wasm)
        .status()
        .context("running wasm-bindgen (cargo install wasm-bindgen-cli --version =0.2.129)")?;
    if !status.success() {
        bail!("wasm-bindgen failed");
    }
    for f in ["mcview.js", "mcview_bg.wasm"] {
        let p = out.join(f);
        println!("{} ({} KB)", p.display(), std::fs::metadata(&p)?.len() / 1024);
    }
    // the sources it was built from (the site build refuses a bundle whose simulator is not its
    // checkout's: `site::web::stale_viewer`)
    let source = site::web::viewer_source_hash(Path::new("."))?;
    std::fs::write(out.join("SOURCE"), format!("{source}\n"))?;
    println!("{} ({source})", out.join("SOURCE").display());
    // the bundle is flat (the site copies the files of web/viewer/, not its folders): wasm-bindgen's
    // snippets move next to mcview.js and its imports follow
    let snippets = out.join("snippets");
    if snippets.is_dir() {
        let mut js = std::fs::read_to_string(out.join("mcview.js"))?;
        let mut stack = vec![snippets.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir)?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let rel = p.strip_prefix(out).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                let flat = format!("mcview-{}", p.file_name().unwrap_or_default().to_string_lossy());
                std::fs::rename(&p, out.join(&flat))?;
                // the module's import statement follows the file; the wasm's import object keeps
                // the snippet's own name as its key (the name the wasm imports it by)
                for q in ['"', '\''] {
                    js = js.replace(&format!("from {q}./{rel}{q}"), &format!("from {q}./{flat}{q}"));
                }
                println!("{} (from {rel})", out.join(&flat).display());
            }
        }
        std::fs::write(out.join("mcview.js"), js)?;
        std::fs::remove_dir_all(&snippets)?;
    }
    // the video recorder's encoder (vendored mediabunny, MPL-2.0: its license beside it), imported
    // by the viewer only when a recording starts; `.js` (every static server sends it as
    // JavaScript, which a module import requires; some send `.mjs` as bytes)
    let vendor = Path::new("crates/mcview-web/vendor/mediabunny");
    std::fs::copy(vendor.join("mediabunny.min.mjs"), out.join("mediabunny.min.js")).context("copying the vendored mediabunny")?;
    std::fs::copy(vendor.join("LICENSE"), out.join("mediabunny.LICENSE.txt"))?;
    println!("{} ({} KB, MPL-2.0: mediabunny.LICENSE.txt)", out.join("mediabunny.min.js").display(), std::fs::metadata(out.join("mediabunny.min.js"))?.len() / 1024);
    Ok(())
}

fn cmd_sheet(pngs: &[PathBuf], out: &Path, crop: Option<&str>, cols: u32, scale: f32) -> Result<()> {
    if pngs.is_empty() {
        bail!("no picture given");
    }
    let crop: Option<[u32; 4]> = match crop {
        None => None,
        Some(s) => {
            let v: Vec<u32> = s.split(',').map(|x| x.trim().parse::<u32>()).collect::<Result<_, _>>().with_context(|| format!("--crop {s}: x,y,w,h"))?;
            let [x, y, w, h] = v[..] else { bail!("--crop {s}: x,y,w,h") };
            Some([x, y, w, h])
        }
    };
    // each picture's part, scaled (nearest), then tiled `cols` a row with a 4 px gap
    let mut tiles: Vec<(Vec<u8>, u32, u32)> = Vec::new();
    for p in pngs {
        let (px, w, h) = mcview::atlas::vanilla::decode_png(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?).map_err(anyhow::Error::msg)?;
        let [cx, cy, cw, ch] = crop.unwrap_or([0, 0, w, h]);
        if cx + cw > w || cy + ch > h {
            bail!("--crop is outside {} ({w}x{h})", p.display());
        }
        let (tw, th) = (((cw as f32 * scale).round() as u32).max(1), ((ch as f32 * scale).round() as u32).max(1));
        let mut t = vec![0u8; (tw * th * 4) as usize];
        for y in 0..th {
            for x in 0..tw {
                let (sx, sy) = (cx + ((x as f32 / scale) as u32).min(cw - 1), cy + ((y as f32 / scale) as u32).min(ch - 1));
                let s = ((sy * w + sx) * 4) as usize;
                let d = ((y * tw + x) * 4) as usize;
                t[d..d + 4].copy_from_slice(&px[s..s + 4]);
            }
        }
        tiles.push((t, tw, th));
    }
    let (tw, th) = (tiles.iter().map(|t| t.1).max().unwrap_or(1), tiles.iter().map(|t| t.2).max().unwrap_or(1));
    let cols = cols.max(1).min(tiles.len() as u32);
    let rows = (tiles.len() as u32).div_ceil(cols);
    let gap = 4;
    let (w, h) = (cols * tw + (cols - 1) * gap, rows * th + (rows - 1) * gap);
    let mut sheet = vec![255u8; (w * h * 4) as usize];
    for (i, (t, iw, ih)) in tiles.iter().enumerate() {
        let (ox, oy) = ((i as u32 % cols) * (tw + gap), (i as u32 / cols) * (th + gap));
        for y in 0..*ih {
            let s = (y * iw * 4) as usize;
            let d = (((oy + y) * w + ox) * 4) as usize;
            sheet[d..d + (iw * 4) as usize].copy_from_slice(&t[s..s + (iw * 4) as usize]);
        }
    }
    mcview::render::headless::write_png(out, w, h, &sheet).map_err(anyhow::Error::msg)?;
    println!("{}: {} pictures, {cols} a row ({w}x{h})", out.display(), tiles.len());
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The client jar of `version`, downloaded (alone, no libraries or assets) when missing.
fn client_jar(version: &str) -> Result<PathBuf> {
    let jar = mctas::paths::version_dir(version).join("client.jar");
    if jar.exists() {
        return Ok(jar);
    }
    let v = mctas::setup::version_json(version)?;
    let c = &v["downloads"]["client"];
    let url = c["url"].as_str().context("no client download in the version json")?;
    mctas::net::download(url, &jar, mctas::net::Checksum::Sha1(c["sha1"].as_str().unwrap_or_default()))?;
    Ok(jar)
}

fn cmd_reference(out: &Path, version: &str, names: &[String]) -> Result<()> {
    let jar = client_jar(version)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?).context("opening the client jar")?;
    std::fs::create_dir_all(out)?;
    let prefix = "assets/minecraft/textures/block/";
    let mut n = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let Some(rest) = f.name().strip_prefix(prefix).map(str::to_string) else { continue };
        let Some(stem) = rest.strip_suffix(".png") else { continue };
        if stem.contains('/') || (!names.is_empty() && !names.iter().any(|w| w == stem)) {
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut bytes)?;
        std::fs::write(out.join(&rest), bytes)?;
        n += 1;
    }
    println!("{n} block textures of {version} in {} (local reference only: never commit them)", out.display());
    Ok(())
}

/// A circuit's judge stimulus and phases as the site emits them (`data-stimulus`, `data-phases`),
/// from the circuit crate; empty for other designs.
fn circuit_attrs(d: &Path) -> String {
    let Ok(c) = circuit::Circuit::load(d) else { return String::new() };
    let Some(kind) = c.kind.clone() else { return String::new() };
    if matches!(kind, circuit::Kind::FlyingMachine) || kind.is_shulker() {
        return String::new();
    }
    let gap = circuit::verify::sizing(&c, 3).unwrap_or(circuit::check::Sizing::default().gap);
    let s = circuit::check::stimulus(&c, &kind, &circuit::check::Sizing { gap });
    if c.inputs.is_empty() && s.events.is_empty() {
        return String::new();
    }
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let phases: Vec<String> = s.phases.iter().map(|(a, b, l)| format!("{a}-{b} {l}")).collect();
    let mut out = String::new();
    if !c.inputs.is_empty() {
        out.push_str(&format!(" data-stimulus=\"{}\"", html_escape(&s.text(&names))));
    }
    let events = events_text(&c, &s);
    if !events.is_empty() {
        out.push_str(&format!(" data-events=\"{}\"", html_escape(&events)));
    }
    // a strength input's values (the hex kinds: the judge's chest fills)
    if !s.levels.is_empty() {
        let levels: Vec<String> = s.levels.iter().filter_map(|(t, i, v)| names.get(*i).map(|n| format!("{t} {n} {v}"))).collect();
        out.push_str(&format!(" data-levels=\"{}\"", html_escape(&levels.join("; "))));
    }
    out.push_str(&format!(" data-phases=\"{}\"", html_escape(&phases.join("; "))));
    out
}

/// A schedule's judge actions at cells (`Schedule::events`: a BUD switch's target and footprint
/// probes) in design coordinates: (frame, cell, op, state).
fn cell_events(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> Vec<(u32, [i32; 3], &'static str, Option<String>)> {
    use circuit::stimulus::{EventAt, TargetOp};
    let probes = if s.events.iter().any(|e| matches!(e.1, EventAt::Probe(_))) { c.probe_cells() } else { Vec::new() };
    let mut v = Vec::new();
    for (t, at, op) in &s.events {
        let pos = match at {
            EventAt::Target(i) => match c.targets.get(*i) {
                Some(tg) => tg.pos,
                None => continue,
            },
            EventAt::Probe(k) => match probes.get(*k) {
                Some(p) => *p,
                None => continue,
            },
            // the vibration test's game events change no block: nothing to show
            EventAt::Input(_) | EventAt::Cell(_) => continue,
        };
        let (name, state) = match op {
            TargetOp::Place(st) => ("place", Some(st.clone())),
            TargetOp::Clear => ("clear", None),
            TargetOp::Shape(st) => ("shape", Some(st.clone())),
            TargetOp::Update => ("update", None),
            TargetOp::GameEvent(_) | TargetOp::Drop(_) => continue,
        };
        v.push((*t, pos, name, state));
    }
    v
}

/// `data-events`: `<frame> <op> <x> <y> <z> [<state>]; ...` (the site's format).
fn events_text(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> String {
    cell_events(c, s)
        .iter()
        .map(|(t, p, op, st)| match st {
            Some(st) => format!("{t} {op} {} {} {} {st}", p[0], p[1], p[2]),
            None => format!("{t} {op} {} {} {}", p[0], p[1], p[2]),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The same events as the viewer replays them (through its `data-events` parser).
fn viewer_events(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> Vec<(u32, [i32; 3], mcview::circuit::EventOp)> {
    mcview::circuit::parse_events(&events_text(c, s)).unwrap_or_default()
}

fn cmd_demo(designs: &[PathBuf], out: &Path, viewer: &Path, live: usize, autoplay: bool) -> Result<()> {
    std::fs::create_dir_all(out.join("designs"))?;
    std::fs::create_dir_all(out.join("viewer"))?;
    // the whole (flat) bundle: mcview.js, its wasm, its snippets, the recorder's encoder
    if !viewer.join("mcview.js").is_file() {
        bail!("no {} (run `mctas web bundle` first)", viewer.join("mcview.js").display());
    }
    for e in std::fs::read_dir(viewer).with_context(|| format!("reading {}", viewer.display()))?.flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), out.join("viewer").join(e.file_name()))?;
        }
    }
    let mut lives = String::new();
    let mut thumbs = String::new();
    for (i, d) in designs.iter().enumerate() {
        let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
        std::fs::copy(d, out.join("designs").join(&name)).with_context(|| format!("copying {}", d.display()))?;
        let src = html_escape(&format!("designs/{name}"));
        let label = html_escape(name.trim_end_matches(".txt"));
        if i < live {
            lives.push_str(&format!("<section><h2>{label}</h2><div class=\"live\" data-mcview=\"full\" data-src=\"{src}\"{}{}><p class=\"fallback\">loading {label}</p></div></section>\n", if autoplay { " data-autoplay" } else { "" }, circuit_attrs(d)));
        }
        thumbs.push_str(&format!("<a class=\"card\" href=\"#\" title=\"{label}\"><div class=\"thumb\" data-mcview=\"thumb\" data-src=\"{src}\"><p class=\"fallback\">{label}</p></div><span>{label}</span></a>\n"));
    }
    let page = format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>REDSTONEPEDIA viewer demo</title>
<style>
body {{ font: 14px system-ui, sans-serif; margin: 24px; background: #1d1f24; color: #e8e8e8; }}
h1 {{ font-size: 20px; }} h2 {{ font-size: 15px; margin: 18px 0 6px; }}
.live {{ width: 100%; height: 480px; background: #2a2d33; border-radius: 6px; overflow: hidden; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, 240px); gap: 12px; }}
.card {{ color: inherit; text-decoration: none; font-size: 12px; }}
.thumb {{ width: 240px; height: 180px; background: #2a2d33; border-radius: 6px; overflow: hidden; }}
.card span {{ display: block; margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.mcview-ready .fallback {{ display: none; }}
.mcview-error {{ outline: 2px solid #c33; }}
.fallback {{ margin: 0; padding: 8px; opacity: 0.6; }}
</style></head><body>
<h1>REDSTONEPEDIA: live contraptions</h1>
{lives}<h2>Thumbnails</h2>
<div class="grid">
{thumbs}</div>
<script type="module">import init from "./viewer/mcview.js"; init();</script>
</body></html>
"#
    );
    std::fs::write(out.join("index.html"), page)?;
    println!("{} ({} designs, {} live)", out.join("index.html").display(), designs.len(), live.min(designs.len()));
    Ok(())
}

/// A minimal static file server (GET only), one connection at a time.
fn cmd_serve(dir: &Path, port: u16) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding port {port}"))?;
    println!("serving {} at http://127.0.0.1:{port}/", dir.display());
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut first = String::new();
        if reader.read_line(&mut first).is_err() {
            continue;
        }
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                break;
            }
        }
        let path = first.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
        let rel = path.trim_start_matches('/');
        let rel = if rel.is_empty() || rel.ends_with('/') { format!("{rel}index.html") } else { rel.to_string() };
        let file = dir.join(&rel);
        let ok = !rel.contains("..") && file.is_file();
        let (status, body) = if ok { ("200 OK", std::fs::read(&file).unwrap_or_default()) } else { ("404 Not Found", b"not found".to_vec()) };
        let ctype = match file.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("js") => "text/javascript",
            Some("wasm") => "application/wasm",
            Some("png") => "image/png",
            Some("json") => "application/json",
            _ => "text/plain; charset=utf-8",
        };
        let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-cache\r\n\r\n", body.len());
        let _ = stream.write_all(&body);
    }
    Ok(())
}

fn cmd_shot(designs: &[PathBuf], out: &Path, o: mcview::tools::ShotOptions, procedural: bool, crop: Option<&str>, env: Option<mcview::daylight::Env>, against: Option<&Path>) -> Result<()> {
    if designs.is_empty() {
        bail!("no design given");
    }
    let crop: Option<[u32; 4]> = match crop {
        None => None,
        Some(s) => {
            let v: Vec<u32> = s.split(',').map(|x| x.trim().parse::<u32>()).collect::<Result<_, _>>().with_context(|| format!("--crop {s}: x,y,w,h"))?;
            let [x, y, w, h] = v[..] else { bail!("--crop {s}: x,y,w,h") };
            if w == 0 || h == 0 || x + w > o.width || y + h > o.height {
                bail!("--crop {s}: outside the {}x{} picture", o.width, o.height);
            }
            Some([x, y, w, h])
        }
    };
    let atlas = atlas(procedural);
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    off.r.env = env;
    let to_dir = designs.len() > 1 || out.is_dir();
    if to_dir {
        std::fs::create_dir_all(out)?;
    }
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        let name = format!("{}.png", d.file_stem().unwrap_or_default().to_string_lossy());
        let path = if to_dir { out.join(&name) } else { out.to_path_buf() };
        let t = std::time::Instant::now();
        match mcview::tools::shot(&mut off, &atlas, &text, &o) {
            Ok((rgba, what)) => {
                let (rgba, w, h) = match crop {
                    Some([x, y, w, h]) => {
                        let mut v = Vec::with_capacity((w * h * 4) as usize);
                        for row in y..y + h {
                            let s = ((row * o.width + x) * 4) as usize;
                            v.extend_from_slice(&rgba[s..s + (w * 4) as usize]);
                        }
                        (v, w, h)
                    }
                    None => (rgba, o.width, o.height),
                };
                mcview::render::headless::write_png(&path, w, h, &rgba).map_err(anyhow::Error::msg)?;
                let cmp = match against {
                    None => String::new(),
                    Some(a) => {
                        let other = if a.is_dir() { a.join(&name) } else { a.to_path_buf() };
                        let bytes = std::fs::read(&other).with_context(|| format!("reading {}", other.display()))?;
                        let (px, ow, oh) = mcview::atlas::vanilla::decode_png(&bytes).map_err(anyhow::Error::msg)?;
                        if (ow, oh) != (w, h) {
                            format!("; against {}: {ow}x{oh}, not {w}x{h}", other.display())
                        } else {
                            let (mut differ, mut most) = (0usize, 0u8);
                            for (p, q) in rgba.chunks(4).zip(px.chunks(4)) {
                                let d = (0..3).map(|k| p[k].abs_diff(q[k])).max().unwrap_or(0);
                                most = most.max(d);
                                if d > 2 {
                                    differ += 1;
                                }
                            }
                            format!("; against {}: {:.3}% of the pixels differ by more than 2 levels, at most {most}", other.display(), differ as f64 * 100.0 / (w * h) as f64)
                        }
                    }
                };
                println!("{} -> {} ({what}, {:.0} ms){cmp}", d.display(), path.display(), t.elapsed().as_secs_f64() * 1000.0);
            }
            Err(e) => println!("{}: ERROR {e}", d.display()),
        }
    }
    Ok(())
}

/// Blocks and ports of both parsers, normalized through redsim's block parser when it knows the
/// state (property order and defaults do not matter), else the raw strings.
fn same_parse(text: &str) -> Result<(), String> {
    let a = mctas::design::Design::parse(text).map_err(|e| format!("mctas: {e:#}"))?;
    let b = mcview::Design::parse(text).map_err(|e| format!("mcview: {e}"))?;
    let norm = |s: &str| redsim::Block::parse(s).map(|x| x.to_string()).unwrap_or_else(|_| s.to_string());
    if a.blocks.len() != b.blocks.len() {
        return Err(format!("{} blocks vs {}", a.blocks.len(), b.blocks.len()));
    }
    for ((pa, sa), (pb, sb)) in a.blocks.iter().zip(b.blocks.iter()) {
        if pa != pb || norm(sa) != norm(sb) {
            return Err(format!("block {pa:?} {sa} vs {pb:?} {sb}"));
        }
    }
    for (k, p) in &a.ports {
        if b.port(k) != Some(*p) {
            return Err(format!("port {k}: {p:?} vs {:?}", b.port(k)));
        }
    }
    if a.entities.len() != b.entities.len() || a.block_entities.len() != b.block_entities.len() {
        return Err("entities / nbt lines differ".into());
    }
    Ok(())
}

fn cmd_check(designs: &[PathBuf], horizon: u32, quiet: bool, wasm: Option<&Path>, unverified: bool) -> Result<()> {
    let mut probe = match wasm {
        Some(p) => Some(mcview::tools::WasmProbe::load(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?).map_err(anyhow::Error::msg)?),
        None => None,
    };
    let (mut ok, mut flyers, mut circuits, mut bad, mut skipped, mut tnts, mut doors) = (0, 0, 0, 0, 0, 0, 0);
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        if let Err(e) = same_parse(&text) {
            bad += 1;
            println!("FAIL {} parse: {e}", d.display());
            continue;
        }
        // a kind the viewer shows as built on purpose (`mcview::circuit::not_replayed_yet`: its
        // judge's run not replayed exactly yet; the coordinator 2026-10-22: shown as built counts
        // as a pass): what it shows is the build, and the viewer must not simulate it
        if !unverified && mcview::Design::parse(&text).ok().and_then(|md| mcview::circuit::not_replayed_yet(&md)).is_some() {
            match mcview::Session::new(&text, &mcview::Options::default()) {
                Ok(s) if !s.simulated() => {
                    skipped += 1;
                    if !quiet {
                        println!("ok   {} (shown as built: {})", d.display(), mcview::circuit::NOT_REPLAYED);
                    }
                }
                _ => {
                    bad += 1;
                    println!("FAIL {} marked as built but the viewer simulates it", d.display());
                }
            }
            continue;
        }
        // piston doors (3x3, 2x2): the door judge's lever cycles in redsim (`door_pair`)
        let door = text.lines().any(|l| l.starts_with("port door:")) && text.lines().any(|l| l.starts_with("port lever:"));
        // mob farms: the judge's farm and run through redsim's own runner (`mob_pair`)
        let mob = !door && text.lines().any(|l| l.starts_with("circuit: mob-farm"));
        let fly = !door && !mob && is_flying_machine(&text);
        // circuits judged under a stimulus: inputs, BUD targets (data-events); shulker designs run
        // with the judge's chests (`shulker_pair`); designs with TNT run with exploding TNT
        // (`tnt_pair`)
        let shulker = !door && !fly && text.lines().any(|l| l.starts_with("circuit: shulker-"));
        // item sorters run with the judge's chests and stream (`sorter_pair`)
        let sorter = !fly && !shulker && text.lines().any(|l| l.starts_with("circuit: sorter"));
        // auto-smelters with the judge's chests, load and coal (`smelter_pair`)
        let smelter = !fly && !shulker && !sorter && text.lines().any(|l| l.starts_with("circuit: smelter"));
        // crop harvesters with the judge's chests and crop, growing by random ticks (`farm_pair`)
        let farm = !fly && !shulker && !sorter && !smelter && text.lines().any(|l| l.starts_with("circuit: harvester"));
        // auto-brewers and auto-composters with their judge's chests (`chest_twin_pair`: their
        // judge's own redsim twin run, frame by frame)
        let chest_twin = !fly && !shulker && !sorter && !smelter && !farm && text.lines().any(|l| l.starts_with("circuit: brewer") || l.starts_with("circuit: composter"));
        let tnt = !door && !fly && !shulker && !sorter && !smelter && !farm && !chest_twin && mcview::design::Design::parse(&text).is_ok_and(|d| mcview::session::is_tnt_design(&d));
        // kinds the viewer shows as built with their judge's chests (minecart stations,
        // auto-crafters, item elevators: their item runs are not replayed yet; `static_pair`)
        let statics = !door && !fly && !shulker && !sorter && !smelter && !farm && !tnt && text.lines().any(|l| l.strip_prefix("circuit: ").is_some_and(|k| mcview::circuit::STATIC_CHEST_KINDS.contains(&k.split_whitespace().next().unwrap_or(""))));
        let circ = !door && !fly && !shulker && !sorter && !smelter && !farm && !tnt && !statics && text.lines().any(|l| l.starts_with("circuit:")) && text.lines().any(|l| l.starts_with("port input") || l.starts_with("port target"));
        let run = if door {
            door_pair(&text)
        } else if mob {
            mob_pair(&text, d, probe.as_mut())
        } else if statics {
            static_pair(&text)
        } else if fly {
            fly_pair(&text, probe.as_mut())
        } else if shulker {
            shulker_pair(&text, probe.as_mut())
        } else if sorter {
            sorter_pair(&text, probe.as_mut())
        } else if smelter {
            smelter_pair(&text, probe.as_mut())
        } else if farm {
            farm_pair(&text, probe.as_mut())
        } else if chest_twin {
            chest_twin_pair(&text)
        } else if tnt {
            tnt_pair(&text, probe.as_mut())
        } else if circ {
            circuit_pair(&text)
        } else {
            clock_pair(&text, horizon, probe.as_mut())
        };
        let line = match run {
            Ok(Some(l)) => l,
            Ok(None) => {
                skipped += 1;
                if !quiet {
                    println!("ok   {} (parse only)", d.display());
                }
                continue;
            }
            Err(e) => {
                bad += 1;
                println!("FAIL {} {e}", d.display());
                continue;
            }
        };
        // what the viewer draws is redsim's state, every tick (circuits: in `circuit_pair`)
        if !circ {
            let busy = tnt || shulker || sorter || smelter || farm || mcview::design::Design::parse(&text).is_ok_and(|d| !d.entities.is_empty());
            if let Err(e) = drawn_pass(&text, if busy { TNT_TICKS } else { DRAWN_TICKS }) {
                bad += 1;
                println!("FAIL {} {e}", d.display());
                continue;
            }
        }
        if door {
            doors += 1;
        } else if fly {
            flyers += 1;
        } else if tnt {
            tnts += 1;
        } else if circ || shulker || sorter || smelter || farm || chest_twin || mob {
            circuits += 1;
        } else {
            ok += 1;
        }
        if !quiet {
            println!("ok   {} {line}", d.display());
        }
    }
    println!(
        "web check: {ok} clocks (same period as clock sim), {flyers} flying machines (same frames and end as circuit fly sim), {circuits} circuits (same frames as circuit sim under the judge's stimulus, or as their judge's redsim twin run: chest machines and farms), {doors} piston doors (same frames and open/close ticks as the door judge's lever cycles in redsim) and {tnts} TNT designs (same blocks and entities as redsim with exploding TNT) agree{}, and draw exactly redsim's blocks and entities every tick, {skipped} parse-only or shown as built, {bad} failures",
        if probe.is_some() { " (clocks, flying machines, shulker, sorter, smelter, farm and TNT designs in the native and the wasm viewer)" } else { "" }
    );
    if bad > 0 {
        bail!("{bad} failures");
    }
    Ok(())
}

/// An auto-brewer or auto-composter held to its judge's own redsim twin of the base run, frame
/// by frame ([`twin_frames_pair`]).
fn chest_twin_pair(text: &str) -> Result<Option<String>, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let frames = match &c.kind {
        Some(circuit::Kind::Brewer(_)) => circuit::brewer::sim_frames(&c, circuit::brewer::Run::Base, 3)?,
        Some(circuit::Kind::Composter(_)) => circuit::composter::sim_frames(&c, circuit::composter::Run::Base, 3)?,
        _ => return Ok(None),
    };
    twin_frames_pair(text, &c, &frames, 3).map(Some)
}

/// A design held to its judge's own redsim twin run, frame by frame (`circuit::{smelter, brewer,
/// composter}::sim_frames`: every non-air cell of the judge's watch box with its block entity's
/// data, every gt): the view's first frame is the twin's frame at the view's start, each step the
/// next, to the twin's end.
fn twin_frames_pair(text: &str, c: &circuit::Circuit, frames: &[Vec<([i32; 3], String)>], margin: i32) -> Result<String, String> {
    let mut s = mcview::Session::new(text, &mcview::Options { unverified: true, ..mcview::Options::default() })?;
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but its judge's twin runs it", s.unsimulated.clone().unwrap_or_default()));
    }
    let (from, to) = circuit::run::watch_box(c, margin);
    let frame = |w: &redsim::World| -> Vec<([i32; 3], String)> {
        let mut v = Vec::new();
        for x in from[0]..=to[0] {
            for y in from[1]..=to[1] {
                for z in from[2]..=to[2] {
                    let q = redsim::BlockPos::new(x, y, z);
                    let b = w.get(q);
                    let st = b.to_string();
                    if !mctas::blockstate::is_air(&st) {
                        v.push(([x, y, z], clock::wave::with_extra(&st, clock::wave::be_extra_redsim(w, q, &b).as_deref())));
                    }
                }
            }
        }
        v.sort();
        v
    };
    // the twin's frame at the view's start (its tick from placement: the settle done, the chests'
    // first fill made; a chest's contents are not in a frame)
    let first = frame(&s.world);
    let start = s.tick as usize;
    if frames.get(start) != Some(&first) {
        let Some(want) = frames.get(start) else { return Err(format!("its judge's twin ends at frame {} before the view's start (tick {start})", frames.len())) };
        let a: Vec<String> = want.iter().filter(|c| !first.contains(c)).take(2).map(|c| format!("{c:?}")).collect();
        let b: Vec<String> = first.iter().filter(|c| !want.contains(c)).take(2).map(|c| format!("{c:?}")).collect();
        return Err(format!("the view's first frame (tick {start}) is not its judge's twin's: the twin {} where the view {}", a.join(", "), b.join(", ")));
    }
    for (k, want) in frames.iter().enumerate().skip(start + 1) {
        s.step();
        let got = frame(&s.world);
        if got != *want {
            let a: Vec<String> = want.iter().filter(|c| !got.contains(c)).take(2).map(|c| format!("{c:?}")).collect();
            let b: Vec<String> = got.iter().filter(|c| !want.contains(c)).take(2).map(|c| format!("{c:?}")).collect();
            return Err(format!("frame {k} of its judge's twin (the view's tick {}): the twin {} where the view {}", s.tick, a.join(", "), b.join(", ")));
        }
    }
    Ok(format!("{} frames agree with its judge's twin run (from its frame {start})", frames.len() - start))
}

/// A design whose `circuit:` header says flying machine.
fn is_flying_machine(text: &str) -> bool {
    mcview::Design::parse(text).is_ok_and(|d| d.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine")))
}

/// A flying machine in `circuit fly sim` and in the viewer (and the wasm when given): the same
/// end tick (the corridor's end, rest, or a break) and the same frames up to it.
fn fly_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use circuit_fly::report::{End, Verdict};
    let m = circuit_fly::machine::Machine::parse(text).map_err(|e| format!("{e:#}"))?;
    let o = circuit_fly::sim::SimOptions::default();
    let r = circuit_fly::sim::simulate(&m, &o);
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    if r.verdict == Verdict::Unmodeled {
        return if s.simulated() { Err(format!("circuit fly sim: unmodeled ({}) but the viewer simulates it", r.why)) } else { Ok(None) };
    }
    let end = match r.end {
        End::Idle(t) | End::Edge(t) | End::Horizon(t) | End::Drop(t, _) | End::Error(t, _) => t,
    };
    let frames = circuit_fly::sim::frames(&m, &o, end)?;
    let ticks = frames.len().saturating_sub(1) as u32;
    let theirs = frames.iter().fold(0xcbf29ce484222325u64, |h, f| mcview::session::frame_hash(h, f));
    // a machine started by its own lever has no end in the viewer before its flip (it rests): its
    // frames up to the judge's end still agree
    let mine_end = match s.end_tick {
        Some(e) => e,
        None if s.starts_on_input() => end,
        None => return Err("the viewer found no corridor end".into()),
    };
    if mine_end != end {
        return Err(format!("circuit fly sim ends at tick {end} ({:?}) but the viewer at {mine_end}", r.end));
    }
    let ours = s.frames_digest(ticks);
    if ours != theirs {
        return Err(format!("frames 0..={ticks} differ from circuit fly sim's"));
    }
    if let Some(p) = probe {
        let wend = p.fly_end(text)?;
        if !s.starts_on_input() && wend != Some(end) {
            return Err(format!("circuit fly sim ends at tick {end} but the wasm viewer at {wend:?}"));
        }
        if p.frames_digest(text, ticks)? != theirs {
            return Err(format!("the wasm viewer's frames 0..={ticks} differ from circuit fly sim's"));
        }
    }
    Ok(Some(format!("{} {}, ends at tick {end}, {} frames agree", r.verdict.word(), r.speed_text(), ticks + 1)))
}

/// `web cardcost`: each design's card (its still and the picture's mesh) timed natively.
fn cmd_camera(designs: &[PathBuf], size: &str, procedural: bool, picked: bool, thumb: bool) -> Result<()> {
    let (w, h) = size.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?))).context("--size W,H")?;
    let atlas = atlas(procedural);
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        // (a card's thumbnail: the still, never simulated, void, as mcview-web's `thumb`)
        let opts = if thumb { mcview::Options { terrain: false, ..mcview::Options::still() } } else { mcview::Options::default() };
        let s = match mcview::Session::new(&text, &opts) {
            Ok(s) => s,
            Err(e) => {
                println!("{}: skipped ({e})", d.display());
                continue;
            }
        };
        let mut look = mcview::scene::viewer_look(&s);
        let aspect = w / h.max(1.0);
        let (pick, window, pts) = if thumb {
            mcview::scene::thumbnail_look(&s, &mut look);
            (mcview::scene::thumb_camera(&atlas, &s, &look, aspect), mcview::scene::THUMB_WINDOW, mcview::scene::still_points(&s, &look))
        } else {
            let (pick, window) = mcview::scene::live_camera(&atlas, &s, &look, w, h);
            (pick, window, mcview::scene::view_points_for(&s, &look, aspect))
        };
        if picked {
            println!("{} yaw {:.0} pitch {:.0}", d.display(), pick.yaw.to_degrees(), pick.pitch.to_degrees());
            continue;
        }
        println!("{} at {w}x{h} CSS px (aspect {aspect:.2}, window {window:?}; display face {:?}):", d.display(), mcview::circuit::display_face(&s.design));
        println!("  {:>5} {:>6}  {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5}  key", "yaw", "pitch", "ports", "parts", "fill", "side", "lean", "unsee", "stack", "hide", "face");
        for c in mcview::scene::camera_candidates(&atlas, &s, &look, aspect, window, &pts) {
            let p = c.parts;
            let mark = if (c.cam.yaw - pick.yaw).abs() < 1e-4 && (c.cam.pitch - pick.pitch).abs() < 1e-4 { "  <- picked" } else { "" };
            println!(
                "  {:>5.0} {:>6.0}  {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>5.2}  {:?}{mark}",
                c.cam.yaw.to_degrees(),
                c.cam.pitch.to_degrees(),
                p.ports,
                p.parts,
                p.fill,
                p.side,
                p.lean,
                p.unseen,
                p.stacked,
                p.hidden,
                c.face_on,
                c.key
            );
        }
    }
    Ok(())
}

/// `web wakecost`: what a page's view costs natively when it loads and when it is played, step by
/// step as the browser runs them (mcview-web `live::load`, `Player::wake`, its first frames):
/// the still it loads as, the session the judge seeds (`Session::new` with `Options::live`), the
/// cycle speed pick (`Session::short_cycle`), the first frame's mesh and triangles, and the
/// slowest of the next `ticks` frames (a tick and its picture each).
fn cmd_wakecost(designs: &[PathBuf], ticks: u32, procedural: bool, show_slices: bool, slowest: Option<usize>) -> Result<()> {
    let atlas = atlas(procedural);
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;
    // (each design's first frame: the load, the still, its camera, the first picture)
    let mut firsts: Vec<(f64, f64, f64, f64, String)> = Vec::new();
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        let t = std::time::Instant::now();
        let mut p = match mcview::player::Player::new_lazy(&text, 1.5, None) {
            Ok(p) => p,
            Err(e) => {
                println!("skip {}: {e}", d.display());
                continue;
            }
        };
        let still = ms(t);
        let tc = std::time::Instant::now();
        p.frame_live(&atlas, 960.0, 640.0);
        let camera = ms(tc);
        let load = ms(t);
        let t = std::time::Instant::now();
        let s = mcview::Session::new(&text, &mcview::Options::live()).map_err(anyhow::Error::msg)?;
        let wake = ms(t);
        let t = std::time::Instant::now();
        // (only for the designs a view looks for one in: `Session::wants_cycle_check`)
        let cycle = if s.wants_cycle_check() { format!("{:?}", s.short_cycle(9, 120)) } else { "not looked for".into() };
        let cyc = ms(t);
        drop(s);
        // the browser's way: the session built over frames, a slice a frame (`Player::wake`)
        let epoch = std::time::Instant::now();
        p.set_clock(std::rc::Rc::new(move || epoch.elapsed().as_secs_f64() * 1000.0));
        p.playing = true;
        p.tps = 20.0;
        let (mut slices, mut longest) = (0u32, 0f64);
        let mut times: Vec<f64> = Vec::new();
        let t = std::time::Instant::now();
        loop {
            let s0 = std::time::Instant::now();
            p.update_with(0.0, &mut || false);
            slices += 1;
            longest = longest.max(ms(s0));
            times.push(ms(s0));
            if !p.is_still() || slices > 100_000 {
                break;
            }
        }
        let woke = ms(t);
        if show_slices {
            println!("  slices (ms): {}", times.iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" "));
        }
        let t = std::time::Instant::now();
        let (a, b) = p.triangles(&atlas);
        let first = ms(t);
        let tris = (a.len() + b.len()) / 3;
        let mut worst = 0f64;
        for _ in 0..ticks {
            let t = std::time::Instant::now();
            p.update(0.05);
            let (a, b) = p.triangles(&atlas);
            std::hint::black_box((a.len(), b.len()));
            worst = worst.max(ms(t));
        }
        println!(
            "{}: load {load:.1} ms (the still {still:.1}, the camera {camera:.1}); on play: the judge's session {wake:.1} ms, the cycle pick {cyc:.1} ms ({cycle}); woken over {slices} frames ({woke:.1} ms in all, the longest {longest:.1} ms); the first picture {first:.1} ms ({tris} triangles); the slowest of {ticks} frames {worst:.1} ms",
            d.display()
        );
        firsts.push((load + first, still, camera, first, d.display().to_string()));
    }
    if let Some(n) = slowest {
        firsts.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("the {} slowest to their first frame (ms: the frame = the still + its camera + the first picture):", n.min(firsts.len()));
        for (f, still, camera, first, d) in firsts.iter().take(n) {
            println!("{f:8.1} {still:8.1} {camera:8.1} {first:8.1}  {d}");
        }
    }
    Ok(())
}

fn cmd_cardcost(designs: &[PathBuf], budget: f64, top: usize, procedural: bool) -> Result<()> {
    // the browser's atlas: the game's textures and block models (the hand-drawn tiles' meshes are
    // a few times lighter: they hid a seqlock card's 286 ms in the browser)
    let atlas = atlas(procedural);
    let mut times: Vec<(f64, f64, String)> = Vec::new();
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        let t0 = std::time::Instant::now();
        // (a card: the still, void like mcview-web's cards)
        let s = match mcview::Session::new(&text, &mcview::Options { terrain: false, ..mcview::Options::still() }) {
            Ok(s) => s,
            Err(e) => {
                println!("skip {}: {e}", d.display());
                continue;
            }
        };
        let t1 = t0.elapsed().as_secs_f64() * 1000.0;
        // as the browser's card draws it (mcview-web `thumb`): the thumbnail look, its camera,
        // the mesh and its triangles
        let mut look = mcview::scene::viewer_look(&s);
        mcview::scene::thumbnail_look(&s, &mut look);
        let cam = mcview::scene::thumb_camera(&atlas, &s, &look, 286.0 / 215.0);
        let mesh = mcview::scene::build(&atlas, &s, 0.0, &look);
        let tris = mcview::scene::triangles(&mesh, &cam);
        std::hint::black_box(&tris);
        let t2 = t0.elapsed().as_secs_f64() * 1000.0;
        times.push((t2, t1, d.display().to_string()));
    }
    times.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (t, s, d) in times.iter().take(top) {
        println!("{t:7.1} ms ({s:.1} the still) {d}");
    }
    let over = times.iter().filter(|x| x.0 > budget).count();
    println!("web cardcost: {} cards, {over} over {budget:.0} ms natively", times.len());
    if over > 0 {
        bail!("{over} cards over {budget:.0} ms");
    }
    Ok(())
}

/// A design of a kind the viewer shows as built with its judge's chests (`mcview::circuit::
/// STATIC_CHEST_KINDS`: minecart stations, auto-crafters, item elevators; their judge's item runs
/// are not replayed yet): a chest stands on every empty `items`/`output` port cell and no lamp is
/// added. Parse only (None) when so; Err otherwise.
fn static_pair(text: &str) -> Result<Option<String>, String> {
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let cells = mcview::circuit::static_judge_chests(&s.design);
    if let Some(p) = cells.iter().find(|p| !matches!(s.world.get(redsim::BlockPos::new(p[0], p[1], p[2])), redsim::Block::Chest { .. })) {
        return Err(format!("no judge's chest at {p:?}: {}", s.world.get(redsim::BlockPos::new(p[0], p[1], p[2]))));
    }
    if !mcview::circuit::consumer_cells(&s.design).is_empty() {
        return Err("a lamp is added on a judge chest's port".into());
    }
    Ok(None)
}

/// A piston door (3x3, or 2x2 with `door-size: 2`) in the viewer against the door judge's run in
/// redsim (`piston_door::cosearch`, the judge `door verify` mirrored: `place_and_settle` in its
/// watch box, then lever cycles, each stepped until idle): the viewer's opening is the judge's
/// (`DoorSpec::door_cells`, in order), the same blocks every tick from the settled rest through
/// every cycle the judge runs (3, more until the rest state repeats), and the same open and
/// close ticks per cycle measured the judge's way (open: the opening and its walkway passable
/// and still; closed: the opening's blocks back as at rest; from the tick the condition holds to
/// idle). None when the door judge's redsim cannot take the design (entities: the viewer may
/// still play them; a block it does not model) or meets something it does not model.
fn door_pair(text: &str) -> Result<Option<String>, String> {
    use piston_door::cosearch;
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let mut s = mcview::Session::new(text, &mcview::Options::default())?;
    // a door the judge's redsim does not take (entities, an unmodeled block): nothing to compare
    let Ok(rs) = cosearch::RsDoor::from_design(&d) else { return Ok(None) };
    if !s.simulated() {
        return Err(format!("the door judge's redsim takes it but the viewer does not simulate it ({})", s.unsimulated.clone().unwrap_or_default()));
    }
    let mcview::Mode::Door { door, lever, along_z, size } = s.mode.clone() else { return Err(format!("not a door in the viewer: {:?}", s.mode)) };
    let cells = rs.spec.door_cells();
    if mcview::session::door_cells(door, along_z, size) != cells || lever != rs.spec.lever {
        return Err(format!("the viewer's opening {:?} (lever {lever:?}) is not the judge's {cells:?} (lever {:?})", mcview::session::door_cells(door, along_z, size), rs.spec.lever));
    }
    // the judge's verdict and ticks at the design's own position; a door it refuses still replays
    // its first lever cycle the same (a calibration that fails on purpose)
    let opts = cosearch::JudgeOpts { cycles: 3, max_ticks: 400, offsets: vec![[0, 0, 0]], empty_walkway: true };
    let (judged, refused): (Vec<Option<(u32, u32)>>, Option<String>) = match cosearch::judge(&rs, &opts) {
        Ok(c) => (c.into_iter().map(Some).collect(), None),
        Err(cosearch::Fail::Model(_)) => return Ok(None),
        Err(f) => (vec![None], Some(format!("{f:?}"))),
    };
    // the judge's world: its watch box and margin, placed like the judge, settled
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (wlo, whi) = cosearch::watch_box(&rs.blocks, &rs.spec);
    let m = 4;
    let mut w = redsim::World::new(bp([wlo[0] - m, wlo[1] - m, wlo[2] - m]), [whi[0] - wlo[0] + 1 + 2 * m, whi[1] - wlo[1] + 1 + 2 * m, whi[2] - wlo[2] + 1 + 2 * m]);
    let list: Vec<(redsim::BlockPos, redsim::Block)> = rs.blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
    w.place_like_judge(&list);
    let (settle, idle) = cosearch::step_until_idle(&mut w, opts.max_ticks, |_| {});
    if !idle {
        return Err(format!("the judge's build never settles ({settle} ticks)"));
    }
    let frame = |w: &redsim::World| mcview::session::frame_hash(0xcbf29ce484222325, &w.blocks());
    if frame(&s.world) != frame(&w) {
        return Err(format!("at rest (the judge settled in {settle} ticks, the viewer at tick {}): the blocks differ", s.tick));
    }
    let mut walk: Vec<[i32; 3]> = cells.clone();
    walk.extend(rs.spec.corridor_cells());
    let passable = |w: &redsim::World| {
        walk.iter().all(|p| {
            let b = w.get(bp(*p));
            !cosearch::collides(&b) && !cosearch::is_moving(&b)
        })
    };
    let closed_door: Vec<redsim::Block> = cells.iter().map(|p| w.get(bp(*p))).collect();
    let restored = |w: &redsim::World| cells.iter().zip(closed_door.iter()).all(|(p, b)| w.get(bp(*p)) == *b);
    // the tick from which a condition held up to idle (`cosearch::settled_from_flags`)
    let settled = |flags: &[bool]| -> u32 {
        let mut first = None;
        for (i, &f) in flags.iter().enumerate() {
            if f {
                first.get_or_insert(i as u32 + 1);
            } else {
                first = None;
            }
        }
        first.unwrap_or(0)
    };
    let mut ticks = 0u32;
    for (cycle, judged_cycle) in judged.iter().enumerate() {
        for half in [0usize, 1usize] {
            let what = if half == 0 { "open" } else { "closed" };
            if !s.toggle(lever) || !w.use_block(bp(lever)) {
                return Err(format!("cycle {cycle}: the lever at {lever:?} does not flip"));
            }
            let mut flags: Vec<bool> = Vec::new();
            let mut n = 0;
            loop {
                s.step();
                w.tick();
                n += 1;
                ticks += 1;
                if frame(&s.world) != frame(&w) {
                    return Err(format!("cycle {cycle}, tick {n} after the flip to {what}: the blocks differ from the judge's"));
                }
                flags.push(if half == 0 { passable(&w) } else { restored(&w) });
                if w.idle() || n >= opts.max_ticks {
                    break;
                }
            }
            if w.idle() != s.world.idle() {
                return Err(format!("cycle {cycle}: {n} ticks after the flip to {what} the judge's world is {}idle, the viewer's {}", if w.idle() { "" } else { "not " }, if s.world.idle() { "is" } else { "is not" }));
            }
            let got = settled(&flags);
            if let Some((o, c)) = judged_cycle
                && got != if half == 0 { *o } else { *c }
            {
                return Err(format!("cycle {cycle}: {what} after {got} ticks in the viewer, {} by the door judge", if half == 0 { o } else { c }));
            }
        }
    }
    if let Some(why) = refused {
        return Ok(Some(format!("door {size}x{size}: its lever cycle, {ticks} ticks agree with redsim's (the door judge refuses it: {why})")));
    }
    let (o, c) = judged.first().copied().flatten().unwrap_or_default();
    Ok(Some(format!("door {size}x{size}: {} cycles, {ticks} ticks agree with the door judge in redsim (open after {o} ticks, closed after {c})", judged.len())))
}

/// A circuit's run under its judge's stimulus in the viewer and in `circuit sim`
/// (`circuit::run::sim_trace`, the judge's consumers included): every cell's full state at every
/// frame must agree. None when the design is no judged circuit.
fn circuit_pair(text: &str) -> Result<Option<String>, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    if !d.meta.contains_key("circuit") {
        return Ok(None);
    }
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(kind) = c.kind.clone() else { return Ok(None) };
    if matches!(kind, circuit::Kind::FlyingMachine) || c.kind.as_ref().is_some_and(|k| k.is_shulker()) {
        return Ok(None);
    }
    let gap = circuit::verify::sizing(&c, 3).unwrap_or(circuit::check::Sizing::default().gap);
    let s = circuit::check::stimulus(&c, &kind, &circuit::check::Sizing { gap });
    if c.inputs.is_empty() && s.events.is_empty() {
        return Ok(None);
    }
    let trace = match circuit::run::sim_trace(&c, &s, 3) {
        Ok(t) => t,
        Err(why) => {
            // redsim cannot run it (a block or a random draw it does not model): the viewer must
            // say so too: shown as built, or stopping where redsim stops under the judge's
            // stimulus (vjeux: only what redsim simulates)
            let mut v = mcview::Session::new(text, &mcview::Options::default())?;
            if !v.simulated() {
                return Ok(None);
            }
            v.attach_demo(mcview::circuit::Schedule { edges: s.edges.clone(), ticks: s.ticks, events: viewer_events(&c, &s), levels: s.levels.clone() });
            for _ in 0..=s.ticks {
                if let Some(h) = &v.halted {
                    return Ok(Some(format!("{} not simulated after {h} (circuit sim: {why})", kind.name())));
                }
                v.step();
            }
            return Err(format!("circuit sim: {why}; the viewer runs on"));
        }
    };
    let (from, to) = circuit::run::watch_box(&c, 3);
    let inside = |p: &[i32; 3]| (0..3).all(|k| p[k] >= from[k] && p[k] <= to[k]);
    let mut session = mcview::Session::new(text, &mcview::Options::default())?;
    if !session.simulated() {
        return Err(format!("circuit sim runs it but the viewer does not: {}", session.unsimulated.clone().unwrap_or_default()));
    }
    session.attach_demo(mcview::circuit::Schedule { edges: s.edges.clone(), ticks: s.ticks, events: viewer_events(&c, &s), levels: s.levels.clone() });
    // the viewer shows a circuit from where its placement transient settled (`Session::new`:
    // at most SETTLE / 2): before the judge's first input, and the frames from there agree
    let first = s.edges.iter().flat_map(|e| e.iter().map(|(t, _)| *t)).chain(s.events.iter().map(|e| e.0)).chain(s.levels.iter().map(|l| l.0)).min();
    if let Some(f0) = first
        && session.tick > f0
    {
        return Err(format!("the viewer settles to tick {}, past the judge's first input at {f0}", session.tick));
    }
    let skip = session.tick as usize;
    // the judge's input devices (R7: a driven input's repeater, support and switch) are not the
    // circuit's: circuit sim leaves them out of its frames
    let judge: Vec<mcview::design::Pos> = session
        .circuit_inputs
        .iter()
        .filter_map(|i| match i.how {
            mcview::circuit::InputHow::Driven { dir, .. } => {
                let (r, sup, sw) = mcview::circuit::driver_cells(i.pos, dir);
                Some([r, sup, sw])
            }
            _ => None,
        })
        .flatten()
        .collect();
    let frames = trace.cells.len();
    for (f, cells) in trace.cells.iter().enumerate().skip(skip) {
        let theirs: Vec<(mcview::design::Pos, &str)> = cells.iter().filter(|(p, _)| inside(p)).map(|(p, i)| (*p, trace.names[*i as usize].as_str())).collect();
        let mine_all = session.cell_states();
        let mine: Vec<(mcview::design::Pos, &str)> = mine_all.iter().filter(|(p, _)| inside(p) && !judge.contains(p)).map(|(p, s)| (*p, s.as_str())).collect();
        if theirs != mine {
            let only = |a: &Vec<(mcview::design::Pos, &str)>, b: &Vec<(mcview::design::Pos, &str)>| a.iter().filter(|x| !b.contains(x)).take(6).map(|(p, s)| format!("{p:?} {s}")).collect::<Vec<_>>().join(", ");
            let diff = format!("circuit sim only: {}; the viewer only: {}", only(&theirs, &mine), only(&mine, &theirs));
            let hows = |a: String, b: String| if a == b { String::new() } else { format!("; inputs: circuit sim {a}, the viewer {b}") };
            return Err(format!("frame {f} differs from circuit sim's ({diff}){}", hows(format!("{:?}", c.inputs.iter().map(|i| format!("{} {:?}", i.name, i.how)).collect::<Vec<_>>()), format!("{:?}", session.circuit_inputs.iter().map(|i| format!("{} {:?}", i.name, i.how)).collect::<Vec<_>>()))));
        }
        // and what the viewer draws is that state (vjeux: nothing animated by the viewer itself)
        session.drawn_matches_simulation().map_err(|e| format!("frame {f}: drawn differs from redsim: {e}"))?;
        // the entities circuit sim records (an elevator's rider: kind and feet position)
        if let Some((_, theirs)) = trace.entities.iter().find(|(g, _)| *g as usize == f) {
            let mine = clock::sim::entity_snapshot(&session.world);
            if &mine != theirs {
                return Err(format!("frame {f}: the entities {mine:?} differ from circuit sim's {theirs:?}"));
            }
        }
        session.step();
    }
    Ok(Some(format!("{} {frames} frames agree", kind.name())))
}

/// Ticks of the drawn-state pass (`drawn_pass`) for a design without entities, TNT or shulker
/// boxes (their runs: TNT_TICKS).
const DRAWN_TICKS: u32 = 120;

/// Everything the viewer draws is redsim's state (`Session::drawn_matches_simulation`: the
/// blocks, and the entities and falling blocks at redsim's positions) on every tick of a run of
/// `ticks` as placed (a circuit's run under the judge's stimulus: in `circuit_pair`).
fn drawn_pass(text: &str, ticks: u32) -> Result<(), String> {
    let mut s = mcview::Session::new(text, &mcview::Options::default())?;
    // no ground but real blocks (vjeux 17:53Z): the viewer's look draws no decorative ground, and
    // a TNT duper's ground is checked cell by cell below (`Floor::now` against the world)
    if mcview::scene::viewer_look(&s).ground.is_some() {
        return Err("the viewer draws a ground that is not redsim blocks".into());
    }
    for _ in 0..=ticks {
        s.drawn_matches_simulation().map_err(|e| format!("tick {}: drawn differs from redsim: {e}", s.tick))?;
        if s.halted.is_some() {
            break;
        }
        s.step();
    }
    Ok(())
}

/// Ticks a TNT design runs in `web check` (TNT primed early explode by ~85; several fuses).
const TNT_TICKS: u32 = 400;

/// A design with TNT (`mcview::session::is_tnt_design`) in the viewer (redsim's TNT entities and
/// explosions: `explosion::explode_on`, seed 0, the viewer box's chunks) and in a reference run
/// built from the repo's tools (`clock::sim::drive_custom`: the clock judge's placement and
/// inputs in the viewer's box, the same configuration): the same blocks and entities (positions
/// and motions, bit for bit) every tick for TNT_TICKS ticks as placed, and the same blocks in the
/// wasm viewer when given. None when redsim does not run it (the viewer shows it as built).
fn tnt_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    if !s.simulated() {
        return Ok(None);
    }
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    // a TNT circuit as its judge builds it: its consumers and its driven inputs' devices (R7)
    let c = match circuit::Circuit::from_design(d.clone()) {
        Ok(cc) => match cc.with_consumers() {
            Ok(cc) => cc.with_drivers().clock,
            Err(_) => cc.with_drivers().clock,
        },
        Err(_) => match clock::Clock::from_design(d) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        },
    };
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let chunks = redsim::redstone::shulker::chunk_range(from, to);
    // a TNT duper stands on its judge's ground (`Circuit::ground_blocks`, laid raw once the design
    // stands, like `clock::sim::drive_tnt`), in the judge's box around it
    let ground: Vec<(redsim::BlockPos, redsim::Block)> = s.floor.as_ref().map(|f| f.blocks()).unwrap_or_default();
    // a duper whose judge found its TNT going off in mid-air (a precise drop, an air burst): its
    // ground lies below its burst point and none of them touches it first (vjeux 04:41Z: "Please
    // make the height of the gap at least the size of the drop before explosion")
    let surface = s.floor.as_ref().map(|f| f.hi[1] as f64 + 1.0);
    if let (Some(f), Some(below)) = (s.floor.as_ref(), mcview::session::burst_below(text)) {
        let lowest = f.design_box.0[1];
        let gap = lowest - 1 - f.hi[1];
        if gap as f32 <= below {
            return Err(format!("its ground is {gap} blocks under the machine, its TNT go off {below} blocks under it: they land first"));
        }
    }
    let mid_air = surface.is_some() && mcview::session::bursts_in_mid_air(text);
    let mut landed: Option<(u32, f64)> = None;
    let (mut h, mut hb): (u64, u64) = (0xcbf29ce484222325, 0xcbf29ce484222325);
    let mut blasts = 0usize;
    let mut destroyed = 0usize;
    // the viewer stops at redsim's first error (something it does not model: vjeux, only what
    // redsim simulates exactly is shown): the frames before it are compared
    let mut stop: Option<(u32, String)> = None;
    clock::sim::drive_custom(&c, from, to, TNT_TICKS, |w| {
        if !ground.is_empty() {
            w.place_raw(&ground);
        }
        redsim::explosion::explode_on(w, 0, chunks)
    }, |t, w| {
        if stop.is_none()
            && let Some(e) = w.errors.first()
        {
            stop = Some((t, e.clone()));
        }
        if stop.is_some() {
            return;
        }
        let b = w.blocks();
        h = mcview::session::frame_hash(h, &b);
        h = mcview::session::entity_hash(h, w);
        hb = mcview::session::frame_hash(hb, &b);
        blasts = w.explosions.len();
        destroyed = w.explosions.iter().map(|x| x.destroyed.len()).sum();
        if mid_air
            && landed.is_none()
            && let (Some(y), Some(x)) = (surface, w.explosions.iter().find(|x| x.center.y <= surface.unwrap_or(f64::MIN) + 0.5))
        {
            landed = Some((t, x.center.y - y));
        }
    })?;
    if let Some((t, dy)) = landed {
        return Err(format!("a TNT went off on its ground at tick {t} ({dy:.2} blocks over the grass): its judge has every TNT going off in mid-air"));
    }
    if s.frames_entities_digest(TNT_TICKS) != h {
        return Err(format!("frames 0..={TNT_TICKS} (blocks and entities) differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, TNT_TICKS)? != hb
    {
        return Err(format!("the wasm viewer's frames 0..={TNT_TICKS} differ from the reference run's"));
    }
    let shown = match &stop {
        Some((t, e)) => format!("shown up to tick {t}, where redsim stops: {e}"),
        None => format!("{TNT_TICKS} ticks"),
    };
    Ok(Some(format!("TNT design: {shown} agree (blocks and entities; {blasts} explosions, {destroyed} blocks destroyed)")))
}

/// Ticks a shulker design runs in `web check` (3 boxes and more of unstackables).
const SHULKER_TICKS: u32 = 1000;

/// A shulker loader or unloader in the viewer (redsim's driver of the judge's chests: endless,
/// unstackables unless it sorts) and in a reference run built from the repo's tools: the design
/// placed by the clock judge's placement (`clock::sim::world_for`) in the viewer's box, the
/// chests on `circuit::Circuit`'s ports, the settle, the supply, then the judge's visit before
/// every tick: the same frames for SHULKER_TICKS ticks as placed (the start lever off, as the
/// viewer starts; in the wasm viewer too when given) and, with a start lever, after the visitor
/// turns it on at once (the judge's run). None when the design is no shulker design, or when
/// redsim cannot follow its run (the viewer must then show it as built).
fn shulker_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use redsim::redstone::shulker::{Chests, SETTLE};
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(kind) = c.kind.clone() else { return Ok(None) };
    let (loader, stack) = match kind {
        circuit::Kind::ShulkerLoader(s) => (true, s),
        circuit::Kind::ShulkerUnloader(s) => (false, s),
        _ => return Ok(None),
    };
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let role = |r: &str| -> Vec<redsim::BlockPos> {
        let mut v: Vec<[i32; 3]> = c.stores.iter().filter(|s| s.role == r).map(|s| s.pos).collect();
        v.sort();
        v.into_iter().map(bp).collect()
    };
    let mut outs: Vec<[i32; 3]> = c.outputs.iter().map(|o| o.pos).collect();
    outs.sort();
    let rejects = role("reject");
    let item = match (rejects.is_empty(), stack) {
        (true, _) | (false, 1) => "iron_shovel",
        (false, 16) => "snowball",
        _ => "cobblestone",
    };
    let item = redsim::redstone::container::item_id(item).ok_or("no such item")?;
    let levers: Vec<[i32; 3]> = c.inputs.iter().filter(|i| i.how == circuit::InputHow::Lever && !(i.name == "mode" || i.name.starts_with("mode."))).map(|i| i.pos).collect();
    // the judge's sandbox (its watch box: seed and forced chunks), from the circuit crate; the
    // viewer computes it from its own design (`mcview::circuit::judge_box`): the same box, or the
    // viewer would draw another level random than the judge's
    let judge = circuit::run::watch_box(&c, redsim::redstone::shulker::JUDGE_MARGIN);
    let viewer_box = mcview::design::Design::parse(text).ok().and_then(|d| mcview::circuit::judge_box(&d));
    if viewer_box != Some(judge) {
        return Err(format!("the viewer's judge box {viewer_box:?} is not the judge's watch box {judge:?}"));
    }
    // the reference run, its start lever on (`lever`) or off: its frames' hash, None when redsim
    // meets something it does not model
    let reference = |lever: bool| -> Result<Option<u64>, String> {
        let mut w = clock::sim::world_for(&c.clock, from, to)?;
        redsim::redstone::shulker::judge_sandbox(&mut w, judge.0, judge.1);
        let mut ch = Chests::endless(loader, item, role("items"), role("boxes"), outs.iter().map(|p| bp(*p)).collect(), rejects.clone());
        ch.place(&mut w);
        for _ in 0..SETTLE {
            w.tick();
        }
        ch.start(&mut w);
        if lever {
            for p in &levers {
                w.use_block(bp(*p));
            }
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for t in 0..=SHULKER_TICKS {
            if t > 0 {
                ch.visit(&mut w);
                w.tick();
            }
            if !w.errors.is_empty() {
                return Ok(None);
            }
            h = mcview::session::frame_hash(h, &w.blocks());
        }
        Ok(Some(h))
    };
    let (placed, started) = (reference(false)?, if levers.is_empty() { None } else { reference(true)? });
    if placed.is_none() || (!levers.is_empty() && started.is_none()) {
        return if s.simulated() { Err("the reference run meets something redsim does not model but the viewer simulates it".into()) } else { Ok(None) };
    }
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but the reference run does", s.unsimulated.clone().unwrap_or_default()));
    }
    let placed = placed.unwrap();
    if s.frames_digest(SHULKER_TICKS) != placed {
        return Err(format!("frames 0..={SHULKER_TICKS} as placed differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, SHULKER_TICKS)? != placed
    {
        return Err(format!("the wasm viewer's frames 0..={SHULKER_TICKS} differ from the reference run's"));
    }
    let mut lever_note = String::new();
    if let Some(theirs) = started {
        // the visitor turns the start lever on before the first tick
        let mut v = mcview::Session::new(text, &mcview::Options::default())?;
        for i in 0..v.circuit_inputs.len() {
            if levers.contains(&v.circuit_inputs[i].pos) {
                v.set_input(i, true);
            }
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for t in 0..=SHULKER_TICKS {
            if t > 0 {
                v.step();
            }
            h = mcview::session::frame_hash(h, &v.world.blocks());
        }
        if h != theirs {
            return Err(format!("frames 0..={SHULKER_TICKS} after its start lever differ from the reference run's"));
        }
        lever_note = ", and after its start lever".to_string();
    }
    Ok(Some(format!("{} {SHULKER_TICKS} ticks agree (the judge's chests, endless){lever_note}", kind.name())))
}

/// Ticks an item sorter runs in `web check` (its judge visits every 20).
const SORTER_TICKS: u32 = 1200;

/// An item sorter in the viewer (redsim's chest driver: the judge's stream fed, the sinks emptied
/// every `circuit::shulker::SEG` gt) against a reference run built from the repo's tools: the
/// design placed by the clock judge's placement (`clock::sim::world_for`) in the viewer's box, a
/// chest on each of `circuit::Circuit`'s `items`, `pass` and output ports, the settle, the
/// judge's own base stream (`circuit::sorter::Scenario::base`) and visits: the same frames for
/// SORTER_TICKS ticks (in the wasm viewer too when given); and the items the viewer's sinks
/// receive visit by visit are those of the judge's redsim screen (`circuit::sorter::sim_run`'s
/// flow). None when the design is no sorter, or when redsim cannot follow its run (the viewer
/// must then show it as built).
fn sorter_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use redsim::redstone::container as rc;
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(circuit::Kind::Sorter(item)) = c.kind.clone() else { return Ok(None) };
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let role = |r: &str| -> Vec<[i32; 3]> {
        let mut v: Vec<[i32; 3]> = c.stores.iter().filter(|s| s.role == r).map(|s| s.pos).collect();
        v.sort();
        v
    };
    let mut outs: Vec<[i32; 3]> = c.outputs.iter().map(|o| o.pos).collect();
    outs.sort();
    let sources: Vec<redsim::BlockPos> = role("items").into_iter().map(bp).collect();
    let sinks: Vec<redsim::BlockPos> = role("pass").into_iter().chain(outs).map(bp).collect();
    let sc = circuit::sorter::Scenario::base(&item, &circuit::sorter::design_kinds(&c)).map_err(|e| format!("{e:#}"))?;
    let mut stream = Vec::new();
    for st in &sc.stream {
        stream.push(rc::Stack::new(rc::item_id(&st.id).ok_or_else(|| format!("redsim has no item {}", st.id))?, st.count as u8));
    }
    let queue: Vec<rc::Stack> = (0..mcview::circuit::SORTER_PASSES).flat_map(|_| stream.iter().copied()).collect();
    let seg = circuit::shulker::SEG;
    if seg != mcview::circuit::SORTER_SEG {
        return Err(format!("the viewer's sorter visits every {} gt, the judge's every {seg}", mcview::circuit::SORTER_SEG));
    }
    // the reference run: its frames' hash and the items its sinks got at each visit
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    let chest = redsim::Block::parse(circuit::shulker::CHEST)?;
    for p in sources.iter().chain(&sinks) {
        w.set_block(*p, chest, redsim::world::flags::ALL);
    }
    for _ in 0..c.clock.free_from() + circuit::stimulus::SETTLE {
        w.tick();
    }
    let mut ch = redsim::redstone::shulker::Chests::judged(true, sources.clone(), Vec::new(), sinks.clone(), Vec::new(), Vec::new(), queue);
    ch.start(&mut w);
    let count = |w: &redsim::World| -> u64 { sinks.iter().map(|p| redsim::redstone::shulker::chest_stacks(w, *p).iter().map(|s| s.count as u64).sum::<u64>()).sum() };
    let mut h: u64 = 0xcbf29ce484222325;
    let mut flow: Vec<(u32, u64)> = Vec::new();
    let mut total = 0u64;
    for t in 0..=SORTER_TICKS {
        if t > 0 {
            if t > 1 && (t - 1) % seg == 0 {
                let got = count(&w);
                if got > 0 {
                    total += got;
                    flow.push((t - 1, total));
                }
                ch.visit(&mut w);
            }
            w.tick();
        }
        if !w.errors.is_empty() {
            return if s.simulated() { Err("the reference run meets something redsim does not model but the viewer simulates it".into()) } else { Ok(None) };
        }
        h = mcview::session::frame_hash(h, &w.blocks());
    }
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but the reference run does", s.unsimulated.clone().unwrap_or_default()));
    }
    if s.frames_digest(SORTER_TICKS) != h {
        return Err(format!("frames 0..={SORTER_TICKS} differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, SORTER_TICKS)? != h
    {
        return Err(format!("the wasm viewer's frames 0..={SORTER_TICKS} differ from the reference run's"));
    }
    // the judge's own redsim screen: the same deliveries at the same visits
    let log = circuit::sorter::sim_run(&c, &sc, 4)?;
    let theirs: Vec<(u32, u64)> = log.flow.iter().copied().filter(|(t, _)| *t < SORTER_TICKS).collect();
    if theirs != flow {
        return Err(format!("the sinks' deliveries {:?} differ from the judge's redsim screen's {:?}", &flow[..flow.len().min(6)], &theirs[..theirs.len().min(6)]));
    }
    if total == 0 {
        return Err(format!("nothing reached its sinks in {SORTER_TICKS} ticks"));
    }
    Ok(Some(format!("sorter {SORTER_TICKS} ticks agree (the judge's chests and stream; {total} items delivered, as the judge's redsim screen)")))
}

/// Ticks an auto-smelter runs in `web check`: its judge's whole base run (16 items a furnace + 2
/// at 200 gt each, the first result after the settle and a cook).
const SMELTER_TICKS: u32 = 4400;

/// An auto-smelter in the viewer (redsim's chest driver: the judge's load in the input chest, its
/// coal in the fuel chest, the output emptied every `circuit::shulker::SEG` gt) against a
/// reference run built from the repo's tools the way the smelter judge's redsim run builds it
/// (`circuit::smelter::sim_run` and its `drive`): the design placed by the clock judge's placement
/// in the viewer's box, a chest on `items`, `fuel` and `output` (in that order, with updates),
/// the settle, then the judge's own load (`circuit::smelter::scenario_for`, the base run) topped
/// up into the input chest and its coal into the fuel chest, and at every visit the output read
/// and emptied and both topped up again: the same frames for SMELTER_TICKS ticks (in the wasm
/// viewer too when given); and the results the output gets visit by visit are those of the
/// judge's redsim run (`sim_run`'s flow). None when the design is no smelter.
fn smelter_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use redsim::redstone::container as rc;
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(circuit::Kind::Smelter(_)) = c.kind.clone() else { return Ok(None) };
    // a super smelter (a double input chest): its judge's chest run, frame by frame (shown as
    // built when redsim does not model a block of it, a blast furnace: so does the twin)
    if c.stores.iter().any(|s| s.name == "items.b") {
        let shown = mcview::Session::new(text, &mcview::Options::default())?;
        return match circuit::smelter::sim_frames(&c, circuit::smelter::Run::Chest, 3) {
            Err(why) if !shown.simulated() && why.contains("not modeled") => Ok(None),
            Err(why) => Err(format!("its judge's twin cannot run it ({why})")),
            Ok(frames) => twin_frames_pair(text, &c, &frames, 3).map(Some),
        };
    }
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let role = |r: &str| c.stores.iter().find(|s| s.role == r).map(|s| bp(s.pos)).ok_or_else(|| format!("smelter: no `port {r}:`"));
    let (items, fuel) = (role("items")?, role("fuel")?);
    let output = c.outputs.first().map(|o| bp(o.pos)).ok_or("smelter: no `port output:`")?;
    let (sc, _) = circuit::smelter::scenario_for(&c, circuit::smelter::Run::Base).map_err(|e| format!("{e:#}"))?;
    let redsim_stacks = |v: &[circuit::shulker::Stack]| -> Result<Vec<rc::Stack>, String> { v.iter().map(|st| Ok(rc::Stack::new(rc::item_id(&st.id).ok_or_else(|| format!("redsim has no item {}", st.id))?, st.count as u8))).collect() };
    let (load, coal) = (redsim_stacks(&sc.stream)?, redsim_stacks(&sc.fuel)?);
    let seg = circuit::shulker::SEG;
    if seg != mcview::circuit::SORTER_SEG {
        return Err(format!("the viewer's smelter visits every {} gt, the judge's every {seg}", mcview::circuit::SORTER_SEG));
    }
    // the reference run, the judge's way
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    let chest = redsim::Block::parse(circuit::shulker::CHEST)?;
    for p in [items, fuel, output] {
        w.set_block(p, chest, redsim::world::flags::ALL);
    }
    for _ in 0..c.clock.free_from() + circuit::stimulus::SETTLE {
        w.tick();
    }
    // the judge's `topup`: the chest's stacks, then the list's next ones up to 27 slots, written
    // back (slots 0..) when anything was added
    let topup = |w: &mut redsim::World, at: redsim::BlockPos, list: &[rc::Stack], cursor: &mut usize| {
        if *cursor >= list.len() {
            return;
        }
        let mut have = redsim::redstone::shulker::chest_stacks(w, at);
        let before = have.len();
        while have.len() < 27 && *cursor < list.len() {
            have.push(list[*cursor]);
            *cursor += 1;
        }
        if have.len() != before {
            let slots: Vec<(usize, rc::Stack)> = have.into_iter().enumerate().collect();
            rc::set_contents(w, at, &slots);
        }
    };
    let (mut cursor, mut fuel_cursor) = (0usize, 0usize);
    topup(&mut w, items, &load, &mut cursor);
    topup(&mut w, fuel, &coal, &mut fuel_cursor);
    let mut h: u64 = 0xcbf29ce484222325;
    let mut flow: Vec<(u32, u64)> = Vec::new();
    let mut total = 0u64;
    for t in 0..=SMELTER_TICKS {
        if t > 0 {
            if t > 1 && (t - 1) % seg == 0 {
                let have = redsim::redstone::shulker::chest_stacks(&w, output);
                if !have.is_empty() {
                    total += have.iter().map(|s| s.count as u64).sum::<u64>();
                    flow.push((t - 1, total));
                    rc::set_contents(&mut w, output, &[]);
                }
                topup(&mut w, items, &load, &mut cursor);
                topup(&mut w, fuel, &coal, &mut fuel_cursor);
            }
            w.tick();
        }
        if !w.errors.is_empty() {
            return if s.simulated() { Err(format!("the reference run meets something redsim does not model ({}) but the viewer simulates it", w.errors[0])) } else { Ok(None) };
        }
        h = mcview::session::frame_hash(h, &w.blocks());
    }
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but the reference run does", s.unsimulated.clone().unwrap_or_default()));
    }
    if s.frames_digest(SMELTER_TICKS) != h {
        return Err(format!("frames 0..={SMELTER_TICKS} differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, SMELTER_TICKS)? != h
    {
        return Err(format!("the wasm viewer's frames 0..={SMELTER_TICKS} differ from the reference run's"));
    }
    // the judge's own redsim run: the same results at the same visits, all of them
    let log = circuit::smelter::sim_run(&c, &sc, 4)?;
    let theirs: Vec<(u32, u64)> = log.flow.iter().copied().filter(|(t, _)| *t < SMELTER_TICKS).collect();
    if theirs != flow {
        return Err(format!("the output's results {:?} differ from the judge's redsim run's {:?}", &flow[..flow.len().min(6)], &theirs[..theirs.len().min(6)]));
    }
    // (a calibration that jams smelts less: the viewer shows the judge's run either way)
    Ok(Some(format!("smelter {SMELTER_TICKS} ticks agree (the judge's chests, load and coal; {total} of its {} items smelted, as the judge's redsim run)", sc.total())))
}

/// Ticks of a farm's run the wasm viewer is compared over in `web check`: the settle, the first
/// growths and harvests (the wasm interpreter runs ~300 ticks/s: the judge's whole run on w86's
/// whole track took the gate past its bound; the native viewer is held to the twin every tick).
const FARM_WASM_TICKS: u32 = 2000;

/// A crop harvester in the viewer against the farm judge's redsim twin (`circuit::farm::sim_run`,
/// which is its game run draw for draw: clk-misc's `farm-run --compare`), tick for tick: every
/// tick of the twin's run (`farm::sim_run_probe`: from after the placement, the planting and the
/// settle to the end of its rest) has the viewer's state (`farm::sim_digest`: the judge's box's
/// blocks, every entity's kind, position and motion, the level random and `Level.randValue`); the
/// viewer's farm box, intakes and constants are the judge's; the wasm viewer's frames are the
/// native viewer's when given. The twin's record (growths, the chests, the intruders) is
/// printed. None when neither runs it (a crop redsim does not model: the viewer shows the build
/// as built).
/// A mob farm's builds as its judge builds them (`circuit::mobfarm::judge`: every turn, then both
/// places) as design texts, each checked to be the viewer's farm too.
/// One judge run of a mob farm (its build `build` of the judge's 8: [`mob_judge_builds`], seed
/// `seed`) played both ways, the judge's runner (`farm::Run` with the card's hand step) and the
/// viewer's session, each world compared every tick (time, mob random, every entity's kind,
/// position and motion: `mcview::session::entity_hash`): None when they are the same to the end
/// with no world error, else where and how they part (or the view stopped).
pub fn mob_diff(design: &Path, build: usize, seed: i64, ticks: u32) -> Result<Option<String>> {
    use circuit::mobfarm as judge;
    use redsim::entity::mob::farm::{HandHide, Run, WalkMode};
    let text = std::fs::read_to_string(design).with_context(|| format!("reading {}", design.display()))?;
    let c = circuit::Circuit::from_design(mctas::design::Design::parse(&text)?)?;
    let offsets = clock::offsets(2);
    let (t, k) = (build / offsets.len(), build % offsets.len());
    let rc = c.turned_pure(t as u8).and_then(|r| r.translated(offsets[k]))?;
    let btext = rc.clock.design.to_text();
    let md = mcview::Design::parse(&btext).map_err(anyhow::Error::msg)?;
    let spec = judge::spec_of(&rc).map_err(anyhow::Error::msg)?;
    let site: Option<&'static redsim::entity::mob::site::Site> = match judge::terrain_site(&rc) {
        Some(s) => Some(Box::leak(Box::new(s.map_err(anyhow::Error::msg)?))),
        None => None,
    };
    let opts = redsim::entity::mob::farm::judge_opts_in(seed, judge::JUDGE_DAYS * 24000, WalkMode::Random, site);
    let rspec = match site {
        Some(s) => s.spec(&spec),
        None => spec.clone(),
    };
    let mut r = Run::new(&rspec, &opts).map_err(anyhow::Error::msg)?;
    let mut hand = mcview::mobfarm::hand_hide_of(&md).then(HandHide::default);
    let own: std::collections::HashSet<u32> = r.own.iter().copied().collect();
    let mut s = mcview::session::Session::new(&btext, &mcview::session::Options { mob_seed: Some(seed), ..mcview::session::Options::live() }).map_err(anyhow::Error::msg)?;
    if let Some(why) = &s.unsimulated {
        bail!("the view does not run it: {why}");
    }
    let dig = |w: &redsim::world::World| -> u64 {
        let mut h = mcview::session::entity_hash(0, w);
        for b in format!("{} {} {:?}", w.time, w.mob.day_offset, w.mob.rng).bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        h
    };
    let ents = |w: &redsim::world::World| -> Vec<String> {
        w.entities.list.iter().filter(|e| !e.removed).map(|e| format!("{:?} pos ({:.4}, {:.4}, {:.4}) motion ({:.4}, {:.4}, {:.4})", std::mem::discriminant(&e.kind), e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z)).collect()
    };
    if dig(&r.world) != dig(&s.world) {
        return Ok(Some(format!("they differ before the first tick: judge time {} rng {:?}, view time {} rng {:?}", r.world.time, r.world.mob.rng, s.world.time, s.world.mob.rng)));
    }
    let t0 = std::time::Instant::now();
    for n in 1..=ticks {
        if let Some(h) = hand.as_mut() {
            h.before_tick(&mut r.world, &rspec, &own);
        }
        r.tick();
        s.step();
        if let Some(h) = &s.halted {
            return Ok(Some(format!(
                "tick {n} (world time {}): the view stopped: {h}; the judge's world errors so far: {:?}; the digests at this tick: judge {:x}, view {:x}",
                s.world.time,
                r.world.errors.iter().take(4).collect::<Vec<_>>(),
                dig(&r.world),
                dig(&s.world)
            )));
        }
        let (a, b) = (dig(&r.world), dig(&s.world));
        if a != b {
            let mut out = format!("tick {n}: the runs part (world time judge {} view {}; mob random judge {:?} view {:?})", r.world.time, s.world.time, r.world.mob.rng, s.world.mob.rng);
            let (ea, eb) = (ents(&r.world), ents(&s.world));
            out += &format!("\n  entities: judge {}, view {}", ea.len(), eb.len());
            for e in ea.iter().filter(|e| !eb.contains(e)).take(6) {
                out += &format!("\n  judge only: {e}");
            }
            for e in eb.iter().filter(|e| !ea.contains(e)).take(6) {
                out += &format!("\n  view only:  {e}");
            }
            return Ok(Some(out));
        }
        if n % 20000 == 0 {
            eprintln!("tick {n}: the same ({:.0} s)", t0.elapsed().as_secs_f64());
        }
    }
    if let Some(e) = r.world.errors.first() {
        return Ok(Some(format!("the same every tick over {ticks} ticks, but the judge's world logged: {e}")));
    }
    Ok(None)
}

/// [`WebCmd::Mobdiff`]: [`mob_diff`], failing when the runs part.
fn cmd_mobdiff(design: &Path, build: usize, seed: i64, ticks: u32) -> Result<()> {
    match mob_diff(design, build, seed, ticks)? {
        None => {
            println!("{} build {build} seed {seed}: the judge's run and the view's are the same every tick over {ticks} ticks", design.display());
            Ok(())
        }
        Some(why) => bail!("{} build {build} seed {seed}: {why}", design.display()),
    }
}

fn mob_judge_builds(c: &circuit::Circuit) -> std::result::Result<Vec<String>, String> {
    use circuit::mobfarm as judge;
    let offsets = clock::offsets(2);
    let mut builds: Vec<String> = Vec::new();
    for t in 0..4u8 {
        for off in &offsets {
            let rc = c.turned_pure(t).and_then(|r| r.translated(*off)).map_err(|e| format!("{e:#}"))?;
            let text = rc.clock.design.to_text();
            let ours = mcview::mobfarm::spec(&mcview::Design::parse(&text)?)?;
            let theirs = judge::spec_of(&rc)?;
            if format!("{ours:?}") != format!("{theirs:?}") {
                return Err(format!("build {} (turn {t}, offset {off:?}): the viewer's farm is not the judge's", builds.len()));
            }
            builds.push(text);
        }
    }
    Ok(builds)
}

/// The judge's runs (every build, every seed: [`mob_judge_builds`], `judge_seed`) through the
/// viewer's session, each golem's spawn on a cell the view shows as a spawn spot
/// (`mobfarm::spawn_check_seeded`; vjeux 2026-11-08, "show the places where golems spawn"): the
/// runs, the golems, the most cells shown open at once.
fn mob_spawn_spots_check(c: &circuit::Circuit) -> std::result::Result<(usize, u32, usize), String> {
    use circuit::mobfarm as judge;
    let jo = judge::JudgeOpts::default();
    let builds = mob_judge_builds(c)?;
    let ticks = jo.days * 24000;
    let jobs: Vec<(usize, i64)> = (0..builds.len()).flat_map(|b| (0..jo.seeds).map(move |k| (b, redsim::entity::mob::farm::judge_seed(b as u32, k)))).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let per = jobs.len().div_ceil(threads);
    let mut results: Vec<std::result::Result<(u32, usize, Vec<String>), String>> = vec![Err(String::new()); jobs.len()];
    std::thread::scope(|s| {
        for (chunk, slots) in jobs.chunks(per).zip(results.chunks_mut(per)) {
            let builds = &builds;
            s.spawn(move || {
                for ((b, seed), slot) in chunk.iter().zip(slots.iter_mut()) {
                    *slot = mcview::mobfarm::spawn_check_seeded(&builds[*b], *seed, ticks);
                }
            });
        }
    });
    let (mut golems, mut most) = (0u32, 0usize);
    for (r, (b, seed)) in results.into_iter().zip(&jobs) {
        let (g, m, outside) = r.map_err(|e| format!("build {b} seed {seed}: {e}"))?;
        if let Some(first) = outside.first() {
            return Err(format!("build {b} seed {seed}: {} golems spawned outside the view's spawn spots, the first {first}", outside.len()));
        }
        golems += g;
        most = most.max(m);
    }
    Ok((jobs.len(), golems, most))
}
fn cmd_mobrate(designs: &[PathBuf], threads: usize) -> Result<()> {
    use circuit::mobfarm as judge;
    let mut bad = 0;
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        let c = circuit::Circuit::from_design(mctas::design::Design::parse(&text)?)?;
        let jo = judge::JudgeOpts::default();
        let builds = mob_judge_builds(&c).map_err(|e| anyhow::anyhow!("{}: {e}", d.display()))?;
        let ticks = jo.days * 24000;
        let jobs: Vec<(usize, i64)> = (0..builds.len()).flat_map(|b| (0..jo.seeds).map(move |k| (b, redsim::entity::mob::farm::judge_seed(b as u32, k)))).collect();
        let t0 = std::time::Instant::now();
        let per = jobs.len().div_ceil(threads.max(1));
        let mut results: Vec<std::result::Result<(u32, u32, u32), String>> = vec![Err(String::new()); jobs.len()];
        std::thread::scope(|s| {
            for (chunk, slots) in jobs.chunks(per).zip(results.chunks_mut(per)) {
                let builds = &builds;
                s.spawn(move || {
                    for ((b, seed), slot) in chunk.iter().zip(slots.iter_mut()) {
                        *slot = mcview::mobfarm::view_counts_seeded(&builds[*b], *seed, ticks);
                    }
                });
            }
        });
        let mut iron_rates = Vec::new();
        let mut golem_rates = Vec::new();
        for (r, (b, seed)) in results.iter().zip(&jobs) {
            let (g, i, _) = r.clone().map_err(|e| anyhow::anyhow!("{}: build {b} seed {seed}: {e}", d.display()))?;
            let hours = ticks as f64 / 72000.0;
            iron_rates.push(i as f64 / hours);
            golem_rates.push(g as f64 / hours);
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let (iron, golems) = (mean(&iron_rates), mean(&golem_rates));
        // a card metric (`key value` items of the `# CIRCUIT:` line; a value starts with a digit:
        // `iron per hour 295`, not `iron per hour per block 1.56`)
        let card = |key: &str| {
            let line = text.lines().find(|l| l.starts_with("# CIRCUIT:"))?;
            line.split(", ").find_map(|item| item.split_once(&format!("{key} ")).map(|(_, rest)| rest).filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit())).map(|s| s.trim().to_string()))
        };
        let (card_iron, card_golems, card_range) = (card("iron per hour"), card("golems per hour"), card("iron range"));
        let ours = (format!("{iron:.0}"), format!("{golems:.1}"));
        let same = card_iron.as_deref() == Some(ours.0.as_str()) && card_golems.as_deref() == Some(ours.1.as_str());
        if !same {
            bad += 1;
        }
        println!(
            "{} {}: the view's {} runs ({} builds x {} seeds, {} days each, {:.0} s): {} iron/h, {} golems/h; the card: {} iron/h ({}), {} golems/h; the page's run (build 0, seed {}): {:.0} iron/h, {:.1} golems/h",
            if same { "ok  " } else { "FAIL" },
            d.display(),
            jobs.len(),
            builds.len(),
            jo.seeds,
            jo.days,
            t0.elapsed().as_secs_f64(),
            ours.0,
            ours.1,
            card_iron.unwrap_or_default(),
            card_range.unwrap_or_default(),
            card_golems.unwrap_or_default(),
            jo.seed0,
            iron_rates[0],
            golem_rates[0]
        );
    }
    if bad > 0 {
        bail!("{bad} farms whose view does not reproduce their card");
    }
    Ok(())
}

/// Mob farms' native tick-for-tick span ([`mob_pair`]: the viewer's world against redsim's run), and
/// the shipped wasm's (the interpreter is slow).
const MOB_TICKS: u32 = 6000;
const MOB_WASM_TICKS: u32 = 1200;
/// A farm in its terrain sample's span (every tick's state key covers the sample's 5.5M cells).
const MOB_TERRAIN_TICKS: u32 = 600;

/// Mob farms (`circuit: mob-farm`; mobviewer 2026-11-08, vjeux: "Just do the real work!?"): the
/// viewer runs the farm through redsim's own runner (`entity::mob::farm::Run`) on the judge's farm,
/// the judge's first run: its constants are the judge's, its farm the circuit crate's
/// (`studies::mobfarm::spec_of`), its world redsim's run's every tick ([`MOB_TICKS`]; in the
/// shipped wasm [`MOB_WASM_TICKS`]), and over the judge's days its golems and iron are the judge's
/// run 0's (`farm::run`: the judge's own loop, with its checks).
fn mob_pair(text: &str, path: &Path, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use circuit::mobfarm as judge;
    use redsim::entity::mob::farm::{RunOpts, WalkMode};
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let md = mcview::design::Design::parse(text)?;
    let jo = judge::JudgeOpts::default();
    if (jo.seed0, judge::JUDGE_DAYS) != (mcview::mobfarm::JUDGE_SEED, mcview::mobfarm::JUDGE_DAYS) {
        return Err(format!("the viewer plays seed {} for {} days, the judge's first run is seed {} for {} days", mcview::mobfarm::JUDGE_SEED, mcview::mobfarm::JUDGE_DAYS, jo.seed0, judge::JUDGE_DAYS));
    }
    let theirs = judge::spec_of(&c)?;
    let ours = mcview::mobfarm::spec(&md)?;
    let (a, b) = (format!("{theirs:?}"), format!("{ours:?}"));
    if a != b {
        let at = a.chars().zip(b.chars()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        let lo = at.saturating_sub(60);
        return Err(format!("the viewer's farm is not the judge's: ...{} vs ...{}", &a[lo..(at + 60).min(a.len())], &b[lo..(at + 60).min(b.len())]));
    }
    // a farm judged in its terrain sample (`mcview::terrain`): the view's site is the judge's
    // (`circuit::mobfarm::terrain_site`: the same spot, the same blocks dug, placed and filled, the
    // same AFK spot and moved farm), both runs built through it
    use redsim::entity::mob::site::Site;
    let site: Option<&'static Site> = match mcview::terrain::sample_of(&md) {
        Some(sample) => {
            let js: &'static Site = Box::leak(Box::new(judge::terrain_site(&c).ok_or("the judge puts it in the void")??));
            let view = mcview::terrain::farm_for(text, &md, &ours)?.ok_or_else(|| format!("no file for the terrain sample {}", sample.name))?;
            let key = |s: &Site, spec: &redsim::entity::mob::farm::FarmSpec| format!("{:?} {} {} {:?} {:?} {:?}", s.offset, s.dug, s.placed, s.fills, s.afk, s.spec(spec));
            let (a, b) = (key(js, &theirs), key(view.site, &ours));
            if a != b {
                return Err(format!("the view's site in {} is not the judge's: {} vs {}", sample.name, &a[..a.len().min(200)], &b[..b.len().min(200)]));
            }
            Some(js)
        }
        None => None,
    };
    let pair_ticks = if site.is_some() { MOB_TERRAIN_TICKS } else { MOB_TICKS };
    let (want, _) = mcview::mobfarm::reference_digests(&md, pair_ticks, site)?;
    let (got, _) = mcview::mobfarm::view_digests(text, pair_ticks)?;
    if let Some(t) = want.iter().zip(&got).position(|(x, y)| x != y) {
        return Err(format!("the viewer's world leaves redsim's run at tick {t} after the mobs arrive"));
    }
    let ticks = judge::JUDGE_DAYS * 24000;
    // the judge's run 0 with the card's hand step (the view performs it as the T run does:
    // `farm::run_t`; the run itself is `farm::run` without it)
    let hand = mcview::mobfarm::hand_hide_of(&md);
    let rep = match site {
        Some(js) => redsim::entity::mob::farm::run_t(&js.spec(&theirs), &redsim::entity::mob::farm::judge_opts_in(jo.seed0, ticks, WalkMode::Random, Some(js)), u32::MAX, hand)?.0,
        None => redsim::entity::mob::farm::run_t(&theirs, &RunOpts::steady(jo.seed0, ticks, WalkMode::Random), u32::MAX, hand)?.0,
    };
    let (golems, iron, chest) = mcview::mobfarm::view_counts(text, ticks)?;
    // (a run its own checks stop, after its 4th failure, holds no totals to compare: the farm
    // fails its judge, which the card says; the view is redsim's run tick for tick all the same)
    let stopped = rep.failures.len() >= 4;
    let with_hand = if hand { " with the card's first-night hide by hand" } else { "" };
    if !stopped && (golems, iron) != (rep.golems, rep.iron_dropped) {
        return Err(format!("over {} days the viewer summons {golems} golems dropping {iron} iron, the judge's run 0{with_hand} {} and {}", judge::JUDGE_DAYS, rep.golems, rep.iron_dropped));
    }
    let totals = if stopped {
        format!("the judge's run 0 stops on its farm's failures (the first: {}), its totals not compared", rep.failures[0])
    } else {
        format!("{} days = the judge's run 0{with_hand}: {golems} golems ({:.1}/h), {iron} iron ({chest} in the chests)", judge::JUDGE_DAYS, golems as f64 * 72000.0 / ticks as f64)
    };
    let wasm = match probe {
        // (a terrain world in the wasm interpreter: its native run is the browser's, the same code)
        Some(_) if site.is_some() => format!(", in {}: the judge's site, native = redsim's run over {pair_ticks} ticks", site.map(|s| s.sample).unwrap_or_default()),
        Some(p) => {
            let (v, g) = mcview::mobfarm::view_digests(text, MOB_WASM_TICKS)?;
            let native = mcview::mobfarm::chain(&v, g);
            let w = p.mob_digest(text, MOB_WASM_TICKS)?;
            if w != native {
                return Err(format!("the wasm viewer's mob farm run ({MOB_WASM_TICKS} ticks) differs from the native one ({w:x} vs {native:x})"));
            }
            format!(", wasm = native over {MOB_WASM_TICKS} ticks")
        }
        None => String::new(),
    };
    let (runs, spawned, most) = mob_spawn_spots_check(&c)?;
    // the converse (vjeux 2026-11-08, "can golems really spawn in the air?"): every spot drawn is a
    // place a golem can spawn by the game's rule, read afresh, and every such place is drawn
    let (looks, drawn, bad, stopped) = mcview::mobfarm::spots_rule_check_seeded(text, jo.seed0, pair_ticks, 100)?;
    if let Some(why) = stopped {
        return Err(format!("the view's run: {why}"));
    }
    if let Some(first) = bad.first() {
        return Err(format!("{} differences between the drawn spawn spots and the game's rule, the first at {first}", bad.len()));
    }
    // and against the GAME (vjeux 2026-11-08, "why is the spawning ui not respecting the game
    // mechanics?"): the farm in the bridge tick for tick with redsim, the game's own scan and
    // checkSpawnObstruction for the villagers the view summons from, every drawn cell one of the
    // game's and every one of the game's drawn, and every golem the game spawns on a drawn cell
    let game = mob_spots_game(path, MOB_GAME_TICKS)?;
    if let Some(first) = game.differences.first() {
        return Err(format!("{} differences between the drawn spawn spots and the game's, the first: {first}", game.differences.len()));
    }
    // (in terrain the judge puts its base build at its k-th cheapest spots instead: the view's
    // runs here are the turned builds', each at its own cheapest spot)
    let runs_what = match site {
        Some(s) => format!("{runs} runs of its turned builds, each at its cheapest spot in {}", s.sample),
        None => format!("the judge's {runs} runs"),
    };
    Ok(Some(format!(
        "mob farm: the judge's farm and run; redsim's world every tick over {pair_ticks}; {totals}{wasm}; {runs_what}: all {spawned} golems on the view's spawn spots (at most {most} cells); every drawn spot a spawn spot by the game's rule and every one drawn ({looks} looks, at most {drawn} cells); in the game (the bridge): as the mobs arrive the drawn cells = the game's own ({} cells), and over {} game ticks all its {} golems on drawn cells",
        game.most, game.ticks, game.spawns
    )))
}

/// Game ticks the game runs a mob farm on alone after the arrival ([`mob_spots_game`]): a day.
const MOB_GAME_TICKS: u32 = 24000;

/// The view's golem spawn cells against the game's over `ticks` ticks of the farm's exact run in
/// the bridge (`mobfarm::exact::spots_check`; vjeux 2026-11-08, "why is the spawning ui not
/// respecting the game mechanics?"): every villager's golem search (any of them summons when it
/// panics), the view's drawn cells read on redsim's world as the mobs arrive, the game's on its
/// own.
fn mob_spots_game(path: &Path, ticks: u32) -> Result<mobfarm::exact::SpotsCheck, String> {
    let d = mcview::design::Design::parse(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)?;
    let opts = mobfarm::exact::Opts {
        seed: mcview::mobfarm::JUDGE_SEED,
        ticks,
        weather: false,
        probe: false,
        batch: 50,
        full: false,
        bare: false,
        draws_at: None,
        terrain: mcview::terrain::sample_of(&d).map(|s| s.name.to_string()),
        day0: None,
        build: None,
        no_spawning: false,
        only: Vec::new(),
    };
    mobfarm::exact::spots_check("26.3", path, &opts, |w, _| {
        let starts: Vec<[i32; 3]> = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager)).map(|e| e.block_position().arr()).collect();
        let drawn = mcview::mobfarm::spawn_spots(w).into_iter().filter(|(_, r)| mcview::mobfarm::spot_open(r)).map(|(c, _)| c.arr()).collect();
        (starts, drawn)
    })
    .map_err(|e| format!("in the game: {e:#}"))
}

fn farm_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use circuit::farm;
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(circuit::Kind::Harvester(crop)) = c.kind.clone() else { return Ok(None) };
    let Some(planted) = farm::CROPS.iter().find(|(k, _)| crop.trim_start_matches("minecraft:") == *k).map(|(_, st)| *st) else { return Ok(None) };
    if redsim::Block::parse(planted).is_err() {
        // a crop redsim does not model (cactus): the viewer shows the build as built
        return Ok(None);
    }
    // the viewer's copy of the judge's numbers
    let consts = [
        (farm::RTS, mcview::circuit::FARM_RTS, "random tick speed"),
        (farm::RUN, mcview::circuit::FARM_RUN, "run"),
        (farm::BATCH, mcview::circuit::FARM_BATCH, "batch"),
        (farm::RTS_PLANTS, mcview::circuit::FARM_RTS_PLANTS, "plants at full speed"),
        (farm::RTS_MIN, mcview::circuit::FARM_RTS_MIN, "slowest speed"),
        (farm::EMPTY, mcview::circuit::FARM_EMPTY, "chest emptying"),
        (farm::REGROW, mcview::circuit::FARM_REGROW, "regrowth"),
        (circuit::shulker::REST, mcview::circuit::FARM_REST, "rest"),
    ];
    for (j, v, what) in consts {
        if j != v {
            return Err(format!("the viewer's farm {what} is {v}, the judge's {j}"));
        }
    }
    let ours: Vec<(String, u8)> = mcview::circuit::FARM_INTRUDERS.iter().map(|(i, n)| (i.to_string(), *n)).collect();
    let theirs: Vec<(String, u8)> = farm::INTRUDERS.iter().map(|(i, n)| (i.to_string(), *n as u8)).collect();
    if ours != theirs {
        return Err(format!("the viewer drops {ours:?}, the farm judge {theirs:?}"));
    }
    if mcview::circuit::FARM_DROP_SAMPLE != farm::DROP_SAMPLE {
        return Err(format!("the viewer drops the intruders on {} cells of each kind, the farm judge on {}", mcview::circuit::FARM_DROP_SAMPLE, farm::DROP_SAMPLE));
    }
    let md = mcview::design::Design::parse(text)?;
    let (jfrom, jto, _) = farm::judge_box(&c, 3);
    if mcview::circuit::farm_box(&md) != Some((jfrom, jto)) {
        return Err(format!("the viewer's farm box {:?} is not the judge's {:?}", mcview::circuit::farm_box(&md), (jfrom, jto)));
    }
    let intakes = circuit::intrusion::intakes(&c);
    if mcview::circuit::intakes(&md) != intakes {
        return Err(format!("the viewer's intakes {:?} are not the judge's {intakes:?}", mcview::circuit::intakes(&md)));
    }
    // the judge's speed for this farm (`farm::game_rts`: `rts_for`, no faster than its collection
    // carries, from its vanilla twin run) and its run's length. The browser runs a farm of up to
    // RTS_PLANTS plants at `rts_for` (no speed on the page): the judge's must be that; a bigger
    // farm runs at the page's `data-farm-rts` (shown as built without it)
    if mcview::circuit::farm_rts_for(&md) != farm::rts_for(&c) {
        return Err(format!("the viewer's speed for this farm is {}, the judge's rts_for {}", mcview::circuit::farm_rts_for(&md), farm::rts_for(&c)));
    }
    let vanilla = farm::vanilla_run(&c, 3);
    let rts = farm::game_rts(&c, &c, &vanilla);
    let small = farm::rts_for(&c) == farm::RTS;
    if small && rts != farm::RTS {
        return Err(format!("the judge runs this small farm at random tick speed {rts}, the browser's view (no data-farm-rts) at {}", farm::RTS));
    }
    let ticks_run = farm::game_ticks(&c, rts);
    if mcview::circuit::farm_ticks(&md, rts) != ticks_run {
        return Err(format!("the viewer's run at speed {rts} is {} gt, the judge's {ticks_run}", mcview::circuit::farm_ticks(&md, rts)));
    }
    let mut s = mcview::Session::new(text, &mcview::Options { farm_rts: Some(rts), ..mcview::Options::default() })?;
    // the judge's twin's run, every tick's state (`farm::sim_run_probe`: the box's blocks, every
    // entity's kind, position and motion, the level random and `Level.randValue`); a run redsim
    // stops (something it does not model) is followed up to there, where the viewer stops too
    let mut lines: Vec<String> = Vec::new();
    let run = farm::sim_run_probe(&c, rts, ticks_run, 3, Some(&mut lines));
    if lines.is_empty() {
        return match run {
            Err(why) if s.simulated() => Err(format!("the viewer simulates it but the judge's twin cannot run it ({why})")),
            _ => Ok(None),
        };
    }
    // a run the twin follows to its end (its every tick: the run and the rest)
    let complete = lines.len() as u32 > ticks_run + 2 * circuit::shulker::REST;
    if !s.simulated() {
        // the viewer shows a farm as built when redsim stops in its run (its probe); so does
        // the twin, early
        if !complete {
            return Ok(None);
        }
        return Err(format!("the viewer does not simulate it ({}) but the judge's twin runs it", s.unsimulated.clone().unwrap_or_default()));
    }
    let ticks = lines.len().saturating_sub(1) as u32;
    for (t, line) in lines.iter().enumerate() {
        let mine = farm::sim_digest(&s.world, jfrom, jto);
        if mine != *line {
            let (a, b): (Vec<&str>, Vec<&str>) = (mine.split("; ").collect(), line.split("; ").collect());
            let at = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            return Err(format!("tick {t} of the run differs from the judge's twin: the viewer {:?}, the twin {:?}", a.get(at), b.get(at)));
        }
        if t < lines.len() - 1 {
            s.step();
        }
    }
    // the wasm viewer runs the same frames as the native one over the settle, the first growths
    // and harvests (FARM_WASM_TICKS)
    let wasm_ticks = ticks.min(FARM_WASM_TICKS);
    if let Some(p) = probe
        && p.frames_digest(text, wasm_ticks)? != mcview::Session::new(text, &mcview::Options::default())?.frames_digest(wasm_ticks)
    {
        return Err(format!("the wasm viewer's frames 0..={wasm_ticks} differ from the native viewer's"));
    }
    let what = if farm::is_stem_crop(&crop) { "fruits grew" } else { "blocks grew" };
    Ok(Some(match run {
        Ok(Ok(log)) => format!(
            "harvester {ticks} ticks agree with the farm judge's twin tick for tick (farm::sim_run: {} {what}, {} items in the judge's chests, {} intruders delivered; tallest {}, longest stopped {} gt)",
            log.grown, log.delivered, log.intruders_delivered, log.height_max, log.full_max
        ),
        Ok(Err(why)) if complete => format!("harvester {ticks} ticks agree with the farm judge's twin tick for tick (its run fails the judge: {why})"),
        Ok(Err(why)) | Err(why) => {
            // the twin stopped at something redsim does not model: the viewer meets it on its
            // next tick
            s.step();
            if s.world.errors.is_empty() && s.halted.is_none() {
                return Err(format!("the judge's twin stops after tick {ticks} ({why}) but the viewer runs on"));
            }
            format!("harvester {ticks} ticks agree with the farm judge's twin tick for tick, up to where redsim stops ({why})")
        }
    }))
}

/// A clock's numbers from both detectors (and the wasm's when given): Some(summary) when they
/// agree, None when the design is not a redsim clock, Err when they differ.
fn clock_pair(text: &str, horizon: u32, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    let design = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    if design.ports.contains_key("door") {
        return Ok(None);
    }
    let Ok(c) = clock::Clock::from_design(design) else { return Ok(None) };
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon, margin: 4 });
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let mine = s.period(horizon);
    let wasm = match probe {
        Some(p) => Some(p.period(text, horizon)?),
        None => None,
    };
    match (r.verdict, mine) {
        (clock::sim::Verdict::Clock | clock::sim::Verdict::Stops | clock::sim::Verdict::Composite, Ok(p)) => {
            // a circuit's view starts at rest (`Session::rest_tick`: redsim runs it until idle
            // before its first frame, as its judge lets it rest before the first input): clock
            // sim's warmups counted from that tick (auto-door-lock-v9 and -plates settle in 1 gt)
            let shift = if s.design.meta.contains_key("circuit") { s.rest_tick } else { 0 };
            let theirs = (r.state_warmup.saturating_sub(shift), r.state_period, r.warmup.saturating_sub(shift), r.period);
            let ours = (p.state_warmup, p.state_period, p.warmup, p.period);
            if theirs != ours {
                return Err(format!("clock sim {theirs:?} vs viewer {ours:?}"));
            }
            if let Some(w) = wasm
                && w != Some(ours)
            {
                return Err(format!("clock sim and the native viewer {ours:?} vs the wasm viewer {w:?}"));
            }
            Ok(Some(format!("W{} P{} (state W{} P{})", p.warmup, p.period, p.state_warmup, p.state_period)))
        }
        (clock::sim::Verdict::Unmodeled | clock::sim::Verdict::Invalid | clock::sim::Verdict::NoRepeat, Err(_)) => match wasm {
            Some(Some(w)) => Err(format!("clock sim {} but the wasm viewer finds {w:?}", r.verdict.word())),
            _ => Ok(None),
        },
        // the clock judge's output rule (vjeux 2026-10-20: the output must be an empty cell where
        // the signal arrives) is a verdict on where it is read, not on the simulation: the
        // viewer runs the build as redsim does
        (clock::sim::Verdict::Invalid, Ok(p)) if r.why.contains("the output cell") => Ok(Some(format!("W{} P{} (state W{} P{}); clock sim INVALID by the output rule: {}", p.warmup, p.period, p.state_warmup, p.state_period, r.why))),
        (v, Ok(p)) => Err(format!("clock sim {} but the viewer finds W{} P{}", v.word(), p.state_warmup, p.state_period)),
        (v, Err(e)) => Err(format!("clock sim {} (W{} P{}) but the viewer: {e}", v.word(), r.state_warmup, r.state_period)),
    }
}

#[cfg(test)]
mod tests {
    /// D1U's judge build 7 seed 1769114314 (`judge_seed(7, 4)`): the viewer's session is the
    /// judge's runner every tick over the 5 days (2026-11-09: both stopped at t103206 on natural
    /// water pouring past the sample's west edge, until b686373716; it had looked like a wasm
    /// divergence). `--ignored` (40 s).
    #[test]
    #[ignore]
    fn d1u_build7_seed_1769114314_view_is_the_judge_run() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // (the terrain sample's file is read from the repository root, as `mctas` runs)
        std::env::set_current_dir(&root).unwrap();
        let why = super::mob_diff(std::path::Path::new("designs/farms/iron/day-1/d1u-underground.txt"), 7, 1769114314, 120000).unwrap();
        assert_eq!(why, None);
    }
}
