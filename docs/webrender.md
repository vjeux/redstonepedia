# The browser viewer (webrender)

Every design renders and runs in the browser: redsim (the exact simulator) and a voxel renderer,
both Rust compiled to WebAssembly (`crates/mcview` + `crates/mcview-web`), no hand-written JS.
REDSTONEPEDIA (the site) embeds it on every page: thumbnails in the filter grids, a live viewer on
each build's page.

## Embedding (the contract with the site)

```html
<div data-mcview="thumb" data-src="designs/clocks/x.txt" style="width:240px;height:180px">fallback</div>
<div data-mcview="full" data-src="designs/doors/y.txt" style="width:100%;height:480px"></div>
<script type="module">import init from "./viewer/mcview.js"; init();</script>
```

* The bundle is `mcview.js` + `mcview_bg.wasm` (wasm-bindgen `--target web`; the wasm is found
  next to the js via `import.meta.url`); `mctas web bundle -o web/viewer` builds it, and a built
  copy is committed at `web/viewer/`.
* Every element with `data-mcview` (or class `mcview`) is a viewer, including elements added
  later (MutationObserver). Mode: `data-mcview` (or `data-mode`) = `thumb` (default) | `full`
  (`live`).
* `data-src` = URL of a design `.txt` (the repo format, relative to the page) or `data-design` = the
  text itself. Optional: `data-tick` (the still frame's tick), `data-yaw`, `data-pitch` (degrees),
  `data-autoplay` (a full viewer starts playing). (`data-trace`, a real-game recording, is no
  longer played, vjeux 2026-10-20: the view shows only what redsim simulates exactly; a design
  redsim cannot run is shown as built with `not simulated: <reason>`.)
* `data-site` (a farm judged in its terrain sample, with `data-terrain`): its site offset there,
  `x,y,z`, found ahead by the site build (`mcview::terrain::site_offset`: the view's own search,
  the judge's spot, the cheapest of its 12; `site::pages::site_of`, cached in the view domain).
  The view prices that spot alone instead of searching and pricing all 12 (`site::price_spots`
  prices each spot on its own: the same site, its covers, seals and clearance, the same
  landscape: test `a_chosen_site_is_the_searched_one` over every farm in terrain); absent or
  unreadable, the view searches. 2026-11-09: ~55 ms natively, the first 3D view in WebKitGTK
  bed-plate-side-chute 1.19-1.31 -> 1.12-1.13 s, bed-plate-two-clusters 1.11-1.15 -> 1.02-1.05,
  cross-4-underground 2.19-2.23 -> 2.02-2.03 (two loads each).
  A part in its terrain (`mcview::terrain::is_part_in_terrain`: a design that is no mob farm, with
  `terrain:` and `site: x,y,z`; ironspec 2026-11-10 on the dug-only drop shaft drop-shaft-11) is
  its cells cut into the sample's world at that site (its reference farm's: the farm optimizer
  writes it, the farm page's own `data-site`), the landscape around it as a farm's; no farm runs
  (played, the world ticks). Its page's `data-site` is that `site:` (`site_of`). Its `ghost: x y z
  <state>` lines (its reference farm's blocks of other parts inside its box: drop-shaft-11's
  chamber walls and fence gates) are placed as the farm has them and drawn see-through (the look's
  ghost cells), never natural ground where the farm has a block (ironspec 2026-11-10).
  A chunk-aligned design (`chunk: <x> <y> <z> <X0>-<X1> <Z0>-<Z1>`, `mcview::design::ChunkAlign`;
  ironspec 2026-11-10, the carousel brief's rule: its reference block at design cell (x, y, z)
  goes at a world X with X mod 16 in X0..X1 and Z mod 16 in Z0..Z1, facing as the download does;
  the generator derives the ranges from the aligned part's box; a malformed line or an empty
  reference cell is refused) shows its chunk borders on the live view: the borders of every chunk
  its footprint touches, the reference block at the middle of its ranges (`ChunkAlign::borders`),
  as lines on its floor seen through the blocks (`Builder::chunk_grid`, drawn with the frame's
  part like the spawn sheet: the blocks' kept mesh keeps no X-ray face), as the game's F3+G draws
  them; no grid on its thumbnail. Its page's first "Build it" step says where the reference block
  goes (`site::plain::chunk_step`).
* `data-still` (thumbnails): the still frame found ahead, `mcview::tools::still_attr(text)` at
  build time (`mctas web still <designs>` prints it): `<tick>`, then ` out=x,y,z` (the clock
  judge's output it adds when the design declares none) and ` box=x0,y0,z0,x1,y1,z1` (the picture
  box). The browser then runs one pass to the tick instead of searching the clock's period again:
  the same picture, pixel for pixel (`data_still_replays_the_still_frame`; a page of four long
  clocks with and without it screenshots identical), in 116 / 32 / 9 / 7 ms instead of 3620 /
  1934 / 14 / 1405 ms (desktop Chromium; natively the search averages 21 ms over the clock
  archive, the slowest 2.5 s, the replay 0.1 ms). It depends on the viewer's code: the site keys
  it on the design text and mcview's code. Absent or unreadable, the viewer searches itself;
  `data-tick` wins over it.
* The viewer appends a `<canvas>` filling the element (absolute, so give the element a size) and
  adds class `mcview-ready` once it shows (hide your fallback child with
  `.mcview-ready .fallback {display:none}`); on failure `mcview-error` and a `title` with the reason.
* Swapping a design in place (the site's variant picker): change attributes on the existing
  element, `data-src` LAST; the viewer re-reads every `data-*` attribute at that moment and
  loads the new design without flashing its fallback: a live viewer keeps the person's camera
  when `data-keep-camera` is set (its direction always; its distance and target too unless the
  new build does not fit it, then they are fitted again: never cut), its play state and
  speed, and rebuilds its input buttons, tags and overlays; a thumbnail is drawn again. Live
  viewers that support it carry the class `mcview-live-src` once ready (`mctas web swap` checks
  it). A lost WebGL context (iOS drops the oldest when a page holds many; a GPU reset) is made
  again when the browser restores the canvas (`mctas web lose`).
* Thumbnails: drawn when the element comes near the viewport (IntersectionObserver, 300 px
  margin), through ONE shared WebGL2 context, copied into the element's canvas, cached by
  (source, size, tick, camera) so a re-added card paints at once. A click dispatches a bubbling
  `mcview-open` CustomEvent (detail = the src). They render in tasks, not animation frames, so
  background tabs and headless browsers draw them too. Only the ones still near the screen at
  their turn are drawn (one that left first is drawn when it comes back), and none during a
  fling (the page scrolling faster than 1200 CSS px/s, `FLING`: drawing the cards flying by made
  the scroll stutter): they wait until it slows or rests 150 ms. A page's new viewer elements
  (60 cards added by the index's infinite scroll) are measured all at once, then set up (one
  layout instead of one per card). A thumbnail's still frame asks for its clock's period twice
  and more: `Session::period` keeps its answer (`period_memo`).
* Live viewers draw and play only on screen (the same observer): off screen a viewer's time
  stands still and it resumes where it was (coordinator: only on-screen live viewers render and
  animate).
* Phone performance, `mctas web perf` (Chromium 375x812 at 3x, CPU slowed 4x; the Clocks tab,
  4177 cards, 2026-10-21): scrolling 8 s at 2500 px/s went from 191-210 frames (mean 38-42 ms,
  p95 117-150 ms, 44-45 over 50 ms; 38-40 long tasks, 3.5-4.0 s; 95-103 thumbnails drawn, 83-87
  of them already off screen) to 475 frames (mean 17 ms, p95 17 ms, 2 over 50 ms; 2 long tasks,
  0.1 s; 1 thumbnail drawn); the 7 thumbnails on screen when it stops are drawn 0.65-0.73 s later
  (was 0.36-0.51 s); a thumbnail takes 31 ms on average (median 21, p95 26-56; was 51, 31, 140).
  WebKitGTK (375x812 at 2x, not slowed): 54 frames (mean 149 ms) and 151 thumbnails drawn (120
  off screen) became 377 frames (mean 21 ms) and 16. The load (first cards 1.2 s, the first
  screen drawn 2.0 s in Chromium; 0.5 s and 0.8 s in WebKit) did not change: its longest task is
  the index's own data (`data/site.json`, 5.5 MB).
* Still frame: a door at rest (settled after placement: closed); a circuit once its placement
  transient settled, as its judge lets it before the first input (`circuit::stimulus::SETTLE`:
  redsim runs until idle, at most SETTLE / 2 ticks; the badge counts from there and a reset comes
  back to it; coordinator: lock3-q85's OUT read powered for its first 4 ticks on a paused page;
  `web check` compares the frames from there and fails a settle past the judge's first input);
  a clock at the most active tick
  of one period after its warmup (most changing cells on, nothing moving), the way
  `mctas clock shot` picks the real-game pictures. A clock's output (`port output`, else the
  clock judge's own output) is in the circuits' OUT box and tag (vjeux, 2026-10-20: not the
  pictures' orange stained-glass marker, which only `web compare` keeps); every output's box,
  clocks' and circuits', turns red (outline, a translucent red fill, its tag's text) while the
  output is powered: its own power state when it has one (a lamp lit, dust with power, a
  powered repeater or observer), else a redstone signal into its cell (a big dripleaf, a solid
  block); air never is (`Session::output_powered`, redsim's state every tick). A clock's empty
  output cell holds the clock judge's lamp (`clock::sim::judge_lamp`, clocks only: not a design
  with a `circuit:` kind or an input port), placed after the design's blocks; its OUT signal is
  the power delivered into the cell, not the lamp's lit state. Lime outlines on
  levers / buttons / input ports. The complete build shows from tick
  0: the `port start` kick (a block placed last, a button pressed) is applied before tick 0 is shown,
  in the clock judge's order.
* Full viewer (touch first): one finger or the mouse drags to orbit, two fingers pinch to zoom and
  drag to pan (right-drag / shift-drag with a mouse, wheel to zoom), a double tap / double click
  resets the view, a tap on a lever or button uses it at any time (paused: it flips and the
  simulation starts, so the build responds); the camera stays above a TNT duper's ground and starts on the
  side the default camera picks (`scene::best_camera`, below): one every lever, button and input cell can be tapped from at its centre (an empty input cell
  counts when a ray reaches it before any block, `scene::centre_point`), else from any point of
  it (a door's lever is often behind the build from the pictures' side; a gate's input behind its
  OUT lamp); no tag covers an input's cell. Bar of
  44 px buttons: reset (r), play/pause (space), one game tick (right arrow), speed (cycles 1x to
  100x of 20 gt/s, then 1/4x, 1/2x; a machine whose blocks repeat in under 10 gt, a 4 gt clock
  that would flicker at 5 Hz, starts at 1/4x, the button showing it and the left badge saying why
  (`slowed (tap ¼× to change): it repeats every 4 ticks (0.2 s)`, two lines on a phone), until the person picks a
  speed: `Session::short_cycle`; not a TNT design, whose flight and blast are the point), flip the lever (f, doors); a strip under the counter shows
  each output's last 40 gt (red: powered), captioned `output, last 2 s`, on clocks and circuits;
  badges at the top: what to do or what runs on the left, up to two lines on a narrow viewer and
  sized once from its longest text, measured unrounded (`tap the lever`, `press the inputs`, the demo's phase,
  `not simulated: <reason>` for a design shown as built, `stopped at tick N: <reason>` when redsim
  meets something it does not model during the run (`dropper picks a random slot among 9 stacks`):
  the view stops at the last exact state;
  it wraps between clauses, never inside a short one: `demo: A off,` / `B off`), the
  time in plain words on the right (`4.10 s · 82 ticks`; fixed width, tabular digits: nothing
  moves while it runs, `mctas web layout` checks it; on a viewer under 520 CSS px, a phone, on
  two lines, seconds over ticks, so the left badge keeps room for its words). No tag or label
  goes under the badges or the Record button, and the output strip sits under the time badge.
  The flip button says `Flip` beside its lever icon (on a phone whose bar also holds the
  contents button, the icon alone).
* Circuits (`circuit:` designs, the contract with the site): `data-stimulus` (the judge's own
  stimulus, `circuit::stimulus` syntax: `in 40+ 44-; ...` or `pulse L every T x K [from S]`,
  input names = the port names, `in` for a bare `port input:`), `data-phases` (`from-to label;
  ...`), `data-input-mode` = `pulse` | `toggle` (default from the kind: edges, limiters,
  extenders, T flip-flops, delays, RS latches and counters pulse; gates, D latches and piston
  extenders toggle). Inputs are driven like `Circuit::actions`: a plain input like a real build
  (the judge's R7, 2026-10-20: `circuit::assign_drivers` ported as `mcview::circuit::inputs`):
  the judge's delay-1 repeater in the first free side cell out of the build, a smooth stone under
  it, the judge's smooth stone in an empty input cell (or the design's own receiving block), and a
  redstone block one cell further out as its switch, placed or removed 2 gt before each input edge
  in the demo (the input cell's power changes at the stimulus tick) and at once on a tap (2 gt
  later through the repeater); an input with no way in keeps the old cell writing (the judge
  refuses the design); `lever` used at every edge; `button` used at rising edges. One
  44 px bar button per input, labeled with its name uppercased (IN, A, B, S, R, D, E): pulse =
  on while held (a tap = a 2 gt pulse), toggle = a tap flips it (shown pressed while on); a tap on
  the input's cell, its device or its tag does the same, and a press held there past 250 ms
  holds the input on until released (like holding its button). A press that begins on an input
  never orbits or pans the camera; nothing moves the camera by itself except a flying machine
  while it plays (piston extenders included: their payload comes back). Nothing in a viewer
  selects text or opens a callout on a long press (user-select / touch-callout off, no context
  menu). Lime tags name the inputs over their cells, orange tags the
  outputs (OUT, or the port name); a strip under the counter shows each output's last 40 gt.
  `data-autoplay` + `data-stimulus` = the auto-demo: the judge's stimulus replayed from tick 0,
  or from 1 s before its first change when that is more than 3 s away (a BUD switch's judge waits
  240 gt before its first event; every tick is simulated, the counter shows the real tick, a
  reset comes back there: `Player::skip_demo_wait`)
  (the left badge says the phase in words: `demo: A on, B off`, `demo: 2nd input pulse`,
  `demo: all inputs off`, `demo: an input pulse of 3 ticks (0.15 s)`, `circuit::phase_words`;
  every tick count in the site's words, `circuit::ticks_words`, never "gt"; as wide as its widest phase from the start so it
  never resizes, and at least as tall as its tallest at that width, never clipped: as wide as it
  may be when a narrower box would push a phase to a third line, `live::fit_badge`; a driven
  input's bar button lights with the stimulus at the edge, not with the judge's switch 2 gt
  before it, `Session::input_level`: critique, A lit while the badge still said "A off"), looped until the person's first input (any button, tap, play/pause/step/reset);
  then the inputs are theirs.
* Nothing simulates until it is played (vjeux 2026-10-21: "Freezes my browser" on the Auto-brewer
  group, "Why is it simulating everything?"). The root cause: every card ran `Session::new`'s
  look-aheads (a brewer's, sorter's, smelter's or farm's 1000-tick probe for something redsim
  does not model; a flying machine's run to its end; a TNT duper's ground grown) and then a still
  search (`go_to_still`: the brewer's 2-hopper clock, 7,625 ticks), all on the main thread, card
  after card; a page's view did the same at load (plus a 20,000-tick period search for a clock's
  output). Now a card is a still (`Options::still`: the design as the judge places it, raw, with
  its judge's chests and crop, its own entities and a TNT duper's ground; nothing ticks; the
  page's `data-still` gives only its output cell and picture box), and a page's view loads as the
  same still (`Player::new_lazy`) and becomes the session that plays when it is played or pressed
  (`Player::wake`: the judge's seeding, its settle and chests, run then; the auto-demo, its
  skipped wait and a door's demo start with play: no page autoplays). The browser's session has
  no look-ahead (`Options::live`, `probe` false): it meets an unmodeled tick when it gets there,
  finds a flying machine's end as it flies (`Session::fly_end_due`, `at_fly_end`: the same tick
  as the look-ahead's), and a TNT duper stands on the judge's first ground. A frame plays at most
  64 ticks and no more than 12 ms of them (`Player::update_with`, `STEP_BUDGET_MS`): what it cannot
  play is dropped. Gates: `web stall` (each page in headless Chromium: loaded, scrolled through,
  every live view played; it fails on a page that stops answering, a main-thread task over 200 ms
  or a view never drawn; `--budget 100` is the bar since 2026-10-22, `--show <ms>` prints each
  task over that with what the viewer logged in it under `?mcview-debug`) and `web cardcost`
  (each card's still and mesh timed natively). Tests `a_card_is_a_still_and_never_ticks`,
  `a_page_view_plays_only_when_played`.
* No task over ~50-100 ms (the coordinator 2026-10-22: pressing play on bamboo-line64 ran one
  445 ms task, super-smelter-8 329 ms, melon-lattice8 490 ms; a page's start 120-190 ms). Long
  work is `async` code with pause points (`mcview::staged`: a `Budget` that pauses once its
  clock passes the deadline its executor gives, a `Task` polled once a frame or task; natively
  `block_on` runs it to its end: the same ticks in the same order). On play the view builds its
  session over frames (`Session::build`: a pause after every tick of every settle loop, its
  placement and its chests; `Player::wake` with the browser's clock, `Player::set_clock`, a
  `WAKE_SLICE_MS` 10 ms slice a frame, the still shown meanwhile, its badge unchanged: `web
  layout` counts a badge that changes size as a layout shift), and the short-cycle look with it
  (`Session::short_cycle_staged`; crop farms no longer look: random growth has no cycle, and it
  was 300+ ms of a big farm's wake). A skipped
  demo wait, `data-tick` and a demo's reset play through unseen within the frames' time
  (`Player::fast_forward`), and what the person presses meanwhile waits for it, in order
  (`Player::defer`). Tests `a_staged_build_is_the_same_session` (ten kinds, staged and at once,
  300 ticks each, block for block and entity for entity), `a_page_view_wakes_over_frames`. The
  page's start is split the same way: the WebGL context, its adapter, its device and its renderer
  each in a task of their own (`next_task`), the thumbnails' context made only once a page has a
  card (a design page has none), the game's atlases decoded row by row over tasks
  (`atlas::vanilla::decode_png_staged`, test `a_staged_png_decode_is_the_whole_frame`), its
  tiles, its block models and its upload in three tasks, a live view's still and its camera
  search in two (`live::load`, `finish_load`), and a card's picture over several work slices
  (`ThumbJob`: its still, camera, mesh and triangles, then its draw). An item label's icon is a
  PNG encoded in Rust (`labels::png_data_url`: a canvas's `toDataURL` took 300-450 ms the first
  time in Chromium). What stays over 50 ms in a fresh headless Chromium: the browser's own GPU
  set-up in a page's first second, which lands in the view's first context or adapter step
  (50-110 ms; a context made a second later takes 5 ms). `web wakecost` times it all natively
  (`--slices`: every slice of the wake; `--slowest N`: at the end the N slowest to their first
  frame, the still + its camera + the first picture, the slowest first: every site design in a
  minute natively). 2026-11-09 (part 11): a farm in its terrain's first frame natively 25-35%
  sooner (d1u-underground 226 -> 155 ms, bed-plate-side-chute 385 -> 280, cross-4-underground
  1345 -> 1030): the landscape's cells checked air first and the cutaway's rows in a fast map
  (`terrain::landscape_staged`), the block models shared (`Rc`, no copy per block), a face its
  neighbour hides dropped before its corners are worked out (`Model::sides`, the same side), the
  camera picker's cell lists dense (`pick::CellLists`, test `dense_cell_lists_are_the_maps`);
  the same pictures byte for byte (`web shot` thumbnails and phone live views of 31 designs).
  2026-11-09 (part 12): the cards: a camera ray's walk (`pick::Picker::hit`, 4 rays per cell
  weighed, for every candidate direction) starts where the ray enters the boxes' cells and stops
  where it leaves them, instead of walking every empty cell from a camera 50 blocks away (test
  `hits_skip_the_empty_cells`: 9000 rays, the same block and distance bit for bit); a session's
  blocks (`Session::render_blocks`) skip air before the floor's and the cutaway's look-ups and
  name each block state once. Natively, a card (`web cardcost` over every site design, the
  slowest): clk-item-dropring-1d-r1-n426 98 -> 21 ms, clk-rt-rect-pulse-n373 86 -> 18,
  cross-4-underground 70 -> 50, cross-4 63 -> 39, clk-item-dropring-2d-n169 51 -> 13, rail line-b
  49 -> under 12; the same thumbnails byte for byte (102 designs). 2026-11-09 (part 13): a live
  view's terrain sample and design text are fetched when the view is set up, side by side; they
  were fetched when its load began, after the game's textures and its WebGL context, the sample
  and then the text: two round trips in a row on the page's critical path. With the rig's
  requests 100 ms late (`MCTAS_RIG_LATENCY_MS=100`), the first 3D view drawn: d1u-underground
  1.75 -> 1.50 s, bed-plate-two-clusters 1.76 -> 1.51, cross-4-underground 2.97 -> 2.62, cross-4
  1.32 -> 1.19, an adder 1.18 -> 1.07; with no latency the same as before (d1u-underground
  1.04-1.21 s before, 1.01-1.08 after, five loads each). A page's first frame on the phone, where
  it goes (`web perf --trace --trace-plain --trace-over 0`, d1u-underground): the viewer's module
  starts at ~350 ms (the page, the 9 MB module's compile), the game's textures ~100 ms, the WebGL
  context ~125 ms, the terrain farm ~215 ms (redsim's site and the landscape, over tasks), the
  live view's still and camera ~80 ms, its picture over three frames at `MESH_BUDGET_MS`.
  2026-11-09 (part 14): a card's design text is fetched when the card comes on screen
  (`App::on_intersect`), not when its turn comes after the game's textures and the thumbnails'
  context; with the rig's requests 100 ms late the first card drawn: the home page 1.36 -> 1.25 s,
  the iron farms' index 1.20 -> 1.15, the circuits' index 1.18 -> 1.12 (two loads each), the other
  indexes the same. A big build's start is split further (the 2x2 iron farm,
  2026-11-08: its camera task 86-98 ms and its first frame 104-124 ms in WebKitGTK, both before
  and after the mob layer came): the live camera's search weighs a few candidate directions a
  task (`scene::LiveCameraSearch`, `CAMERA_BUDGET_MS` 20; the same candidates in the same order:
  the `--ignored` test `every_archived_design_keeps_its_live_camera`, 4462 cameras of every
  archived circuit and door on a desktop and a phone, the same), the first picture's mesh is built over frames
  (`scene::Staged`, `Player::triangles_within`, `MESH_BUDGET_MS` 24: the first pass and the
  light, then 16 blocks between checks; the same faces in the same order: test
  `a_staged_still_is_the_one_shot_mesh`) and drawn in a frame of its own, and the renderer's
  programs and the frame targets at the canvas's size are made by a draw offscreen in two tasks of
  their own (`Renderer::warm_up`, `warm_up_targets`), not in the first frame. A mob farm's blocks'
  mesh, when one changes, is staged the same way while its previous blocks keep being drawn. The
  2x2 farm's longest task in WebKitGTK went from 124 ms to 35 ms; `web perf --engine webkit` (no
  task list in that engine) gives its slowest load frame 69-73 ms (was 110: two tasks with no
  frame between them). So that no load frame goes over 50 ms either (the parent, 2026-11-08), a
  frame comes between pieces of work: `next_task` waits for the browser's next frame (`next_frame`:
  an animation frame, then a task after it; 100 ms at most in a hidden page) once 12 ms of work ran
  since the last one (`YIELD_AFTER_MS`; a live view's frame marks its start, `mark_frame`), and
  again when a live frame ran long meanwhile; a card's work slice waits for a frame the same way,
  and a card's camera and mesh pause between candidates and blocks (`Budget::over`). And the
  steps that were one task each: a still never looks for a harvester's intruder drops (13 ms of a
  2492-block sweeper's still natively), the picker finds each state's boxes once (16 ms to 3 of a
  big farm's camera; test `a_picker_built_per_state_is_the_one_built_per_block`), the live
  camera's search set-up is a task of its own, the demo badge measures at most its 48 longest
  texts (a counter's 2306 labels took 269 ms of layouts), the game's block models are parsed in one
  task and their texture's mip levels made in the next, the entity atlases, the particle sprites
  and their upload are three tasks. `web perf --engine webkit --trace` prints each load frame over
  50 ms and what the viewer did in it (`?mcview-trace`: its frames, card slices, staged slices,
  WebGL steps, its work by where it runs and its debug lines, timed, in `window.__mcTrace`), and
  when its module started: a slow frame before that is the browser's own (the page, the module's
  compile). WebKitGTK, three loads of each page (devvm69723, 2026-11-08), frames over 50 ms before
  → after: iron-day1 2-3 (66-67 ms) → 0 (41-44); bamboo-sweeper-10x64 4-6 (73-87) → 0-1;
  cane-lattice-16x16 2-4 (66-71) → 0-1; counter576-items 2 (305-324) → 0 (42-44); bamboo-line64
  2-3 (67-70) → 0-1; melon-lattice8 2-3 (68-70) → 0-1. Every remaining one (52-73 ms) but one
  ended before the viewer's module started; the one (52 ms) was the block models in one task,
  since split.
* Doors with `data-autoplay`: the door demo flips the lever like the record videos (`door
  video`): 1.5 s closed, the lever on until the door settles open (`Session::idle`), 1.2 s open,
  the lever off until it settles closed, again; lever uses only (what runs is redsim's), the
  badge saying the stage (`demo: lever on, the door opens`, `demo: the door, open`, ...); the
  person's first flip, tap or bar input stops it.
* 2x2 doors (clk-item's `door-size: 2`, designs/piston-door-2x2/; the coordinator 2026-10-21:
  "door viewer on a 2x2 opening"): `Mode::Door` carries the opening's side (`size`, 3 by default,
  2 from the header, anything else refused like the judge's `door_size`), and the opening is its
  4 cells (`session::door_cells`, the judge's `DoorSpec::door_cells` order): the picture frames
  them, the walkway box follows, `Session::door_blocks` counts 4 when closed. `web check`
  (`door_pair`) runs every piston door, 3x3 and 2x2, against the door judge in redsim
  (`piston_door::cosearch`: its watch box and margin, `place_like_judge`, the settle, then the
  lever cycles it runs, each until idle): the viewer's opening and lever are the judge's, its
  blocks are the judge's every tick, and its open and close ticks per cycle are the judge's
  (`cosearch::judge` at the design's position, measured the same way). Test
  `a_2x2_door_demo_opens_and_closes_its_4_cells`.
* `data-events="<frame> <op> <x> <y> <z> [<state>]; ..."` (design coordinates, frames like
  `data-stimulus`: after placement): the judge's actions at cells besides the inputs
  (`circuit::stimulus::Schedule::events`: a BUD switch's target and footprint probes), replayed
  in the auto-demo (with `data-autoplay`, with or without inputs): `place` (setBlock with
  updates, flags 3, like a player), `clear` (removed with updates), `shape` (changed with shape
  updates only, flags 2), `update` (`updateNeighborsAt` the cell, nothing changes). `web check`
  replays the BUD switches' events through this parser frame by frame against `circuit sim`.
  Cyan tags name a BUD switch's TARGET cells.
* `data-levels="<frame> <input> <value>; ..."` (frames like `data-stimulus`, inputs by name,
  values 0..15): a strength input's values (`port input.a: x y z strength`, the judge's hex
  kinds, `circuit::stimulus::Schedule::levels`), replayed in the auto-demo like the judge: the
  input's chest placed again (its state unchanged), then its contents set to the value's fill
  (`circuit::strength_items`: the fewest cobblestone whose comparator signal is the value). A
  tap on a strength input flips it between 0 and 15. `web check` replays them against `circuit
  sim` (cal-hex-gt: 28201 frames). A circuit `circuit sim` cannot run (a random draw redsim does
  not model) must stop at the same tick in the viewer (`stopped at tick N: <reason>`).
  An item frame input (`port input.d0: x y z frame`, study 14's combination locks) turns one
  step per level step, like the judge's right-click (`Action::UseEntity`: clk-item's
  `redsim::entity::frame::use_frame`; redsim keeps frames in `World::frames`, the comparators'
  read of them, tick-exact against the game by `circuit trace-compare`); a press (its button,
  its tag or its cell) turns it one step. A lock's frames are
  tagged `DIAL 1`..`DIAL n` and `CODE 1`..`CODE n` with their turn after the name (`DIAL 1 · 3`,
  the judge's digits; critique 41f1ec62a: "Tag the 3 dials and the 3 code frames"); their bar
  buttons say `Dial 1`.. and `Code 1`.. (critique 058332315: not the raw port names), and when
  the bar has no room for them the tags press the
  inputs (all of a bar's input buttons go). The frames are drawn with the game's `item_frame`
  model and their item's sprite (the design's `Item`) turned their turn x 45 degrees
  (`ItemFrameRenderer`, `mesh::Builder::item_frame`; `Session::entities` draws `World::frames`
  after the entities and falling blocks, and `drawn_matches_simulation` checks each). The
  default camera prefers seeing a lock's dials from the side they face (`Targets::frames`: 6 x
  the dials' facing, code frames a quarter); the code frames, set once from their tags, neither
  count as ports nor need a tappable side (critique on lock3-q85: facing away from the dials,
  they held the camera in front, the dials seen edge-on). `web check` replays the archived lock's
  whole stimulus against `circuit sim`.
* TNT designs (any TNT block) run live with redsim's own TNT and explosions (2026-10-20,
  clk-misc: `explosion::explode_on`, seed 0 with the world box's chunks, like the clock traces;
  entities tick only in the sandbox's forced chunks, so a TNT flung out of them freezes there and
  never explodes, as in the game):
  each primed TNT is redsim's `PrimedTnt` entity (its kick from the mirrored level random; moved
  by the engine: gravity, collisions, slime, moving blocks), drawn like the game's `TntRenderer`
  (swelling over the last 10 ticks of its fuse, flashing 75% white every 5), and at fuse 0 it
  EXPLODES exactly like the game (`redsim::explosion`): the blocks it breaks vanish, their drops fly
  as items, a TNT block it hits primes, other TNT is pushed; a blast destroying a moving block
  drops what it moves. A TNT duper with its explosions stands on its judge's ground (vjeux
  2026-10-20 17:53Z: "Please don't put a fake ground if the tnt goes through it"): real blocks of
  the simulated world laid like the judge's strict run (`circuit::run::sim_tnt`,
  `Circuit::ground_blocks`: grass over two dirt over bedrock, its top the archived safe drop
  under the design (`# CIRCUIT: ... safe drop N blocks`, 0 when absent) or, for a duper that
  declares its stand (`stand: N`, a precise drop: its TNT fall straight down and go off in mid-air
  under it), that stand when higher, as its judge judges it (`Session::ground_drop`: vjeux 04:41Z,
  "Please make the height of the gap at least the size of the drop before explosion"; at the
  safe drop its TNT landed before going off; every other duper keeps its safe drop: vjeux 04:42Z,
  "For horizontal this is fine"), `reach` blocks around the
  design as written, in the judge's world box around it), placed raw once the design stands like
  `clock::sim::drive_tnt`. The reach is recomputed like the judge's: 24, doubled while a burning
  TNT gets more than `reach - 1` blocks sideways from the design within the viewer's first 600
  ticks (the judge's stimulus replayed for a triggered or switchable duper), at most 96 (cached
  per design: `Session::with_ground`). Its TNT land, bounce and explode on it and the craters
  show: the ground's cells that changed (`Floor::changed`, followed from the world's write log,
  redsim's opt-in `World::log_writes`: no scan of the big world per tick; a block written
  outside the machine's box and the ground is drawn too, `Session::outside`) and their
  neighbours are drawn as blocks, the rest by its intact faces (`mesh::floor`: the grass tops,
  each layer's side at its edge, the bedrock's underside); the machine's blocks come from its own
  box (`Session::machine_box`). A duper standing above its ground says how high (`8 blocks
  above the ground` beside a dashed line from its bottom down to the ground, or to the view's
  bottom when the ground lies below the view) and, up to 8 blocks, is framed with the ground
  under it, so it never reads as sitting on the grass; one standing higher is framed with its
  TNT's fall under it down to their burst point (coordinator: "Camera should frame machine +
  burst point"). A TNT flying past the ground's edge falls into the void, as in
  the judge's world, or freezes where it leaves the forced chunks. `web check` fails a duper whose
  judge has its TNT going off in mid-air (`precise: ... in mid-air`) when its ground lies at or
  above its burst point (`in mid-air X blocks below the machine`) or one of its TNT goes off on
  the ground in the viewer's run (without its `stand:`, the precise drop fails: "its ground is
  4 blocks under the machine, its TNT go off 10 blocks under it: they land first").
  `web check` runs each TNT design against a reference built from the repo's tools
  (`clock::sim::drive_custom` with the same configuration, the same ground): the same blocks and
  entities every tick for 400 ticks, native and wasm. The camera frames the TNT's flight in
  redsim's run within 8
  blocks of the build (a launcher's TNT leaves the view as it flies on).
* Shulker loaders and unloaders (`circuit: shulker-loader|shulker-unloader`) run live like their
  judge (redsim's `redstone::shulker::Chests`, the driver `mctas circuit shulker compare` checks
  tick for tick against the real game): the judge's chest on each `items`, `boxes`, `output` and
  `reject` port (tags ITEMS, BOXES, OUT) placed after the design, the settle (40 gt), then the
  box chests and a loader's sources kept full and the outputs emptied before every tick
  (endless), with unstackables (27 per box: a box every ~11 s) unless the design sorts (a
  `reject` port: its kind's own item). The world is the judge's sandbox for the design (since
  2026-10-20, clk-misc): its level random mirrored (`World::mirror_level_random`) and its drops
  seeded from the seed the judge builds it with (`redstone::shulker::judge_sandbox`: the watch
  box, the design's blocks and ports plus 3, gives the seed and the forced chunks), so a dropper
  choosing among stacks (dropper75) and a hopper choosing among minecarts (the rail unloaders)
  draw what the game draws, and every archived shulker design runs live. A start lever is
  the visitor's to flip (the judge flips it at its start); always-on designs start by
  themselves. A design whose run redsim cannot follow (it runs 1000 gt ahead on load) is shown
  as built (with the judge's chests, nothing else added): none of the archive today. Chests and
  shulker boxes are drawn
  with the game's `ChestModel` / `ShulkerModel` and entity textures. `web check` runs every
  shulker design against a reference run built from the repo's tools (the clock judge's
  placement, `circuit::Circuit`'s ports, the judge's sandbox from `circuit::run::watch_box`,
  which must equal the viewer's box), as placed and after its start lever: the same frames,
  native and wasm. A design shown as built never offers inputs or input outlines.
* Elevators (`circuit: elevator N`, track fly's two-way vertical flying machines; the coordinator
  2026-10-21: "rider + button presses play") carry their judge's rider: the armor stand the
  circuit crate adds at `port rider` (`Circuit::from_design`, after the design's own entities) is
  added the same way by `Session::new`, so it stands on the platform and rides it; the page plays
  the judge's button presses (its stimulus) and a visitor's clicks press the floors' buttons.
  `web check` compares, besides the blocks, every entity's kind and feet position with circuit
  sim's every frame of the judge's run (`clock::sim::entity_snapshot`; fly-elevator-5: 42822
  frames). Test `an_elevator_carries_its_rider`.
* Randomizers (`circuit: randomizer N`, study 16) run live (site 2026-10-21: rand2-token stopped
  at tick 44, "a dropper picks a random slot ... not modeled without the level random"): the
  world's level random is mirrored like the judge's base run (`World::mirror_level_random`,
  seeded with `build_seed` of the judge's watch box at salt 0, its forced chunks drawing 12 a
  tick: `circuit::random`), so the demo plays the judge's test press for press and a visitor's
  own presses draw from the same random. The watch box (like the shulker sandbox's) is the design
  AS WRITTEN, its blocks and ports plus 3: the judge's consumers and the input drivers the viewer
  adds stay out of it (a driver behind rand2-token's input moved the box one block and every
  draw with it). Test `a_randomizer_draws_the_judges_random`; `web check` compares every frame
  with circuit sim's under the judge's stimulus (rand2-token: 2561 frames).
* Camera: every view (live viewers and thumbnails) looks from the default camera's direction
  (vjeux 2026-10-20 23:53Z: "Can we have some kind of heuristic to figure out the default camera
  position. Many of them look really bad right now": `scene::best_camera`): of 20 candidates
  (`scene::candidate_dirs`: the pictures' direction, 29 degrees off the south axis and 28 up, and
  its 7 mirror images, at 28 and at 45 degrees up: each 29 degrees off an axis, three faces seen,
  never straight along a flat face, never from below; then the 4 face-on directions, low
  (`FACE_ON_PITCH`, 12 degrees up: critique, "at ~25° rather than face-on"), for a wall (one block
  thick, at least 3 by 3 and 12 cells across its face, of every block drawn as it stands: the
  judge's chests too, not its see-through rig) seen from its face, `FACE_ON_BONUS`, any other build
  only at a cost, `FACE_ON_COST`: critique, a flat extender "reads as loose floating pieces" at 45
  degrees; a small thin build, a NOT gate or a BUD switch, reads better from three sides),
  each framing the build as below, the one
  scoring best on 3 x the ports seen (inputs and outputs 3, the other ports 1.5, a door's opening
  0.5 a cell, a TNT duper's TNT 2 in all; an item sorter's PASS chest 3, like an output) + 2 x the components seen (`scene::part_weight`: pistons, observers, comparators,
  repeaters, torches, levers, buttons 1; lamps, note blocks, bulbs 0.8; dust, containers, rails,
  TNT 0.6; slime 0.4; the 64 heaviest) + 2 x the frame fill (the mean of how much of the window
  the framed points fill and how big the build's own cells come out: a latch's two inputs side
  by side, a duper bigger than its landing zone) + 0.75 x how
  far the inputs are on the camera's side - 0.2 from 45 degrees up (the pictures' elevation
  unless the higher one shows more) + 0.5 x how far a build without inputs has its output's end
  toward the camera when that output lies over 2 blocks from its middle (critique: a long ring
  of droppers seen with its working end far away) - 0.5 for each pair of a live view's tags that
  would hang on one spot (`scene::tag_anchors`, closer than 0.16 by 0.08 of half the view's
  height: critique, a gate's OUT tag sat on its A tag; 1.5 at most) - 0.5 for each pair of a
  thumbnail's drawn port cells one right behind the other (closer than 0.6 of a block there: a
  BUD switch's TARGET box over its observer; 1 at most); a port cell counts when it is drawn (a
  block, or an outlined cell); the containers a design's labels are about (its own containers
  holding items, but on a shulker design: a sorter's filter hopper, a ring's droppers) count like
  ports, six at most (critique: a sorter's filter hopper hid behind its PASS chest) - 2 x the
  ports (weight 1.5 or more) seen less than half, each counted by its weight over an output's and
  by how hidden it is, 2 at most (critique: the sorter's PASS chest mostly hidden behind its ITEMS
  chest: a fuller frame is no excuse); a cell is
  "seen" by the share of 4 points (its main
  model box's centre and its 3 faces turned to the eye) that are in the view and that a ray from
  the eye reaches first (`pick::Picker`: every model box indexed by the cells it reaches). A side
  every input can be tapped from at its centre comes first, then one each can be tapped from
  somewhere; ties go to the earlier candidate (the pictures' own first); scores are compared at
  a thousandth (deterministic). A TNT duper on its judge's ground looks from the pictures' own
  direction alone, the duper in front of its landing zone (critique: "frame the duper plus its landing
  zone from the side-ish angle used before"). A circuit's input buttons go in the bar left to
  right as the default camera shows the inputs (`scene::inputs_left_to_right`; critique: the AND
  gate's A tag on the right, B on the left, the reverse of its A, B buttons); inputs spread more
  up and down the view than across it keep the design's order (a lock's dials: D1, D2, D3). The framing fits the build's whole moving extent
  (`scene::view_points`: the corners of every cell it occupies or sweeps: its blocks now and as
  placed, a door's opening, a clock's swept box over a period, a piston extender's travel, its
  ports, inputs and marker, its entities and where redsim's run takes them in 200 gt, within 8
  blocks; a TNT duper's TNT near the ground (2 blocks at most above it) within 16, its landing
  zone, counted at the ground's surface where they sink into their crater, and within 8 in a portrait view, `view_points_for`: beside the
  duper on a phone it left the duper a speck) at the pictures' cropped field of view: 80% of the width, between
  the bar and the badges in a live viewer (`live_window_css`: 74 px clear of the bottom and 46 px
  of the top at the viewer's own size, in CSS pixels (the canvas's device pixels put the build
  under the badges on a phone); on a phone's viewer (under 520 px wide) 66 px of the top, the
  time badge on two lines and the output strip under it, and the bottom clear of the Record pill
  only when the build would come out under it: `live_camera`, `PILL`), and each driven input's whole lime box; centred in a thumbnail (`THUMB_WINDOW`, the still build
  only: no extender travel or entity look-ahead, no TNT duper's ground under it, no judge's
  see-through test rig (critique: gate tiles small and washed out), and only what
  it draws: a thumbnail has no outlines but a BUD switch's TARGET box (`scene::thumbnail_look`:
  critique, the BUD tile was a lone observer "with no TARGET"), so its other empty input and port
  cells are not framed: the inputs and ports count where the look outlines them; a thumbnail fits
  the build itself however small, its margins are enough); in a live view a build smaller than 3
  blocks across and 2 high in every direction is framed a block bigger than it is, up to that (a
  lone block as 2 across; the camera as far as that box needs,
  the build itself centred: `scene::fit_view`; a 2-block clock stands in a
  little sky around). A long thin build's 3/4 view along its length comes from the default
  camera's frame-fill score (the 45-degree rule it replaces: `scene::view_dir`, kept for
  `default_camera`). The browser draws like a
  client with a 24-chunk render distance (`Renderer::view_chunks`; `web compare` keeps the
  pictures' 6). No ground but real blocks (vjeux 17:53Z): the judges' worlds are void under a
  design (only a TNT duper's judge lays a ground, above), so the viewer draws no surface there:
  the sky goes on under the horizon and fades into a quiet void tone (`Renderer::void_below`),
  nothing an item, a cart or a falling block could pass through. On a TNT duper's real ground the
  grass tops are lit like the game's smooth lighting: each corner the smooth blend of the light of
  the four air cells around it over the grass, from the build's light (`light::Light`, below):
  the build's shadow. `web check`
  fails when the viewer's look draws a ground (`Look::ground`) or a ground cell differs from the
  world's (`Floor::now`, what the ground draws, against the world in `drawn_matches_simulation`).
  `web compare` alone keeps the real-game pictures' camera and grass (`scene::picture_camera`, 56
  blocks).
* Item sorters (study 13, `circuit: sorter <item>`; critique: sorter-classic stood still at 0
  ticks): they run like their judge's redsim screen (`circuit::sorter::sim_run`): a chest on each
  `items`, `pass` and `output` port (set like a player's), the settle, then the source topped up
  to 27 stacks from the judge's base stream (`circuit::sorter_stream`: its kind mixed with
  64-stack, 16-stack and unstackable kinds, the kinds the design holds left out; 8 times over)
  and the sinks emptied every 20 gt (`circuit::SORTER_SEG`), by redsim's chest driver
  (`redstone::shulker::Chests`, as a loader's). Tags: ITEMS `kept full` (in stacks), PASS (blue)
  and OUT `received` with the run's tally (below; the judge's stream starts with a full stack of
  its kind, so PASS gets its first items after about 26 s at 1x); no auto slow motion (a hopper's 8-gt cycle is no
  flicker). `web check` compares its frames with a reference built from the circuit crate (the
  judge's own stream and chests) and the items its sinks get, visit by visit, with the judge's
  redsim screen (`sorter_pair`); the site autoplays sorter pages.
* Auto-smelters (study 16, `circuit: smelter N`; the coordinator 2026-10-21: the 3D view did not
  run the smelter study) run like their judge's base run (`circuit::smelter::sim_run`): a chest on
  `items`, `fuel` and `output` (in that order, set like a player's), the settle, then the input
  chest topped up from the judge's load (`circuit::smelter_load`: 16 items a furnace of raw gold,
  cobblestone, sand, oak logs, kelp and raw copper, and two iron shovels, in the judge's order),
  the fuel chest given its coal (one per 8 items and two a furnace, at least a stack) and the
  output emptied every 20 gt, by redsim's chest driver (the load its queue, the coal its supply).
  The furnace is redsim's (`AbstractFurnaceBlockEntity`, tick-identical with the game): the
  game's block model, `furnace_front_on` while lit (a still texture in the game too). Tags: ITEMS,
  FUEL (grey) and OUT with the run's tally. Every furnace is labelled by default: its slots and
  its screen's fire and arrow (`labels::Cooking`, AbstractFurnaceScreen's steps: the flame
  `ceil(lit / total * 13) + 1` of 14 while lit, the arrow `ceil(cooking / total * 24)` of 24).
  `web check` compares its frames for 4400 ticks (the whole base run) with a reference built the
  judge's way (`smelter_pair`: the circuit crate's load and coal, its topping up and emptying)
  and the results the output gets, visit by visit, with the judge's redsim run; the site
  autoplays smelter pages. Tests `a_smelter_runs_like_its_judge` (all 18 items out, smelted, the
  furnace lit, the arrow through) and `a_furnace_shows_its_screens_fire_and_arrow`.
  A super smelter (a double input chest, `port items.b:`) runs its judge's chest run instead
  (`circuit::smelter::Scenario::chest`, the coordinator 2026-10-22: "match the chest run"): both
  halves topped up from one load of 54 stacks of raw iron (`smelter_chest_load`), the coal it
  needs and a stack more, never refilled; `web check` holds it to the judge's own redsim twin of
  that run frame by frame (`twin_frames_pair` on `circuit::smelter::sim_frames`: every non-air
  cell of the judge's watch box with its block entity's data, every gt from the view's start to
  the twin's end). Blast furnaces stay as built (redsim has none).
* Shown as built on purpose (`circuit::not_replayed_yet`, `circuit::NOT_REPLAYED`; vjeux's
  standing rule: the 3D view shows only what redsim simulates exactly): a kind whose view does
  not replay its judge's run frame for frame yet shows its still, `not simulated: this view does
  not replay our test exactly yet (...)`, and no play: today the auto-composter (its judge runs a
  row of its slices, the view one). `web check` counts such a design as a pass (and fails it if
  the viewer simulates it); `web check --unverified` holds them to their judge anyway, and
  `Options::unverified` runs their replay for their tests. Auto-brewers are held to their
  judge's twin frame by frame the same way (`chest_twin_pair` on `circuit::brewer::sim_frames`).
* Crop harvesters (farms, `circuit: harvester sugar_cane|bamboo|kelp|cactus|pumpkin|melon`; site and the coordinator
  2026-10-21: the view showed a lamp on the judge's chest and bare soil, then "replay the judge's
  run exactly") replay their judge's run (`circuit::farm::game_run`, whose redsim twin
  `farm::sim_run` is its game run draw for draw, clk-misc's `farm-run --compare`): the world is the
  judge's sandbox from before the design is placed (`circuit::farm_sandbox`, `farm_box`: the watch
  box raised for the crop; its seed for the level random, the drops, the loot tables and
  `Level.randValue`, its forced chunks ticked in the game's order), the judge's chest on every
  `port output:` (raw; no lamp on store ports: sorters, smelters, harvesters and minecart
  stations keep none), random ticks off while the crop is planted at age 0 on every `port crop:`
  (in the ports' name order, as the judge lists them) and settles, then the judge's speed for
  its runs (`circuit::farm::game_rts`: `rts_for`, 300 (`FARM_RTS`, 100 times vanilla: a cane
  grows a block about every 11 s at 1x) for up to 8 plants, 300 x 8 / plants for more (at least
  30), and no faster than its collection carries, from its 720,000 gt vanilla twin run: a page
  gives it as `data-farm-rts`, `Options::farm_rts`; without it a farm of up to 8 plants runs at
  300 (`web check` holds every archived one to that) and a bigger one is shown as built, `not
  simulated: this view does not replay our test exactly yet (a big crop farm, its judge's speed
  not given)`) for its run (`farm::game_ticks`: `FARM_RUN` 24000 gt, up to 4 x longer when its
  collection slowed it; growth stops then), its intruders summoned as the bridge summons them at
  its quarters (`farm_drops`: a cobblestone, 64 iron ingots, a shovel on up to 8 open hopper
  intakes, `circuit::intakes`, and above up to 8 crop columns, evenly spread, `FARM_DROP_SAMPLE` =
  `farm::DROP_SAMPLE`, in the judge's order: each item's own
  random follows its summon number), its chests emptied every 500 gt of the run and its rest when
  they hold a crop item or an intruder (`FARM_EMPTY` = `farm::EMPTY`; the OUT label tallies what
  it takes), and after the rest growth resumed in 500 gt batches while a crop item lies on a crop
  (`FARM_REGROW` = `farm::REGROW` 1600 gt at 300, `farm::sim_finish`; coordinator 2026-10-22:
  "follow each design's own judge parameters"). The harvest: the observer, the
  piston breaking the grown block (or the fruit beside an attached stem), its item onto the
  hopper (or a hopper minecart) and into the chest (OUT tag with the chest's count). Sugar cane,
  bamboo, kelp, cactus, pumpkin and melon stems on farmland, their fruits and the melon's slices
  from its loot table's random (clk-misc), the waterlogged copper grate (its water held in) and
  waterlogged blocks' water drawn in their cell are redsim's. A crop redsim cannot follow (bamboo
  in a shaft: its light model) is shown as built, with the judge's chests when the build is
  modeled. `web check` (`farm_pair`) computes the farm's `game_rts` as the judge does and holds
  the viewer at that speed to the judge's twin every tick of its run
  (`farm::sim_run_probe`, to the end of its rest: the box's blocks, every entity's kind, position
  and motion, the level random and `Level.randValue`, `farm::sim_digest`), and up to where redsim
  stops when it meets something it does not model (the viewer stops there too); the wasm viewer's
  frames are the native viewer's over the settle and the first growths and harvests
  (`FARM_WASM_TICKS` 2000: the wasm interpreter runs ~300 ticks/s); it checks the viewer's box,
  intakes and constants against the judge's and prints the twin's record: every
  farm in the archive and w86's track agrees. Tests `a_harvester_grows_and_harvests_like_its_judge`,
  `a_melon_farm_grows_and_harvests_like_its_judge`.
* Mob farms (ironfarm's iron farms, `circuit: mob-farm iron_ingot <tier>`; vjeux 2026-11-08 on
  the pages' "the view shows the farm as built" caveat: "Why all of these caveats? Just do the
  real work!?") run their judge's own run (`mcview::mobfarm`): the view builds the judge's farm of
  the design as written (`mobfarm::spec`, the circuit crate's `spec_of`: its blocks, entities,
  output chest and watch box) and steps it with redsim's one runner, `entity::mob::farm::Run`,
  the code the judge runs (`start`, its 200-tick water settle paused between ticks over the wake's
  frames, `arrive`, then `tick`: the golems' random walking policy, then the world's tick), seed
  `JUDGE_SEED` = the judge's first run of its base build, played on past its 5 days. A farm
  whose card says "hide the zombie by hand the first night" (its design does not claim
  `restart: yes`) gets that step as the judge's T run performs it (`farm::HandHide`, shared with
  `farm::run_t`): at day time 12000 of its first night the trapdoors flip so no villager sees the
  zombie, and flip back once every villager sleeps; the badge says "you hid the zombie" meanwhile
  and the scrubber marks each step in amber. Nothing of
  the mob layer is approximated in the view: what it adds is the client's drawing of redsim's
  state (`MobRun::draw` over `mob::view`): `LivingEntityRenderer`'s transforms (a sleeper laid on
  its bed toward its pillow, a dying mob falling sideways over 20 ticks, a walking golem's sway,
  a mob in a boat at the boat's yaw, its head within 105 degrees), the client's walk animation
  from the position deltas (`LivingEntity.calculateEntityAnimation`, client side), the 26.3
  client's models (`IronGolemModel`, `VillagerModel` with the villagers' 0.9375 scaling and its
  plains type layer, the zombie's `HumanoidModel` riding, arms out and bobbing:
  `entitymodel::{iron_golem, villager, zombie}`, with `CubeDeformation` growth and child parts),
  a golem's crackiness layer by its health (`Crackiness.GOLEM`), the red overlay of a hurt or
  dying mob (`OverlayTexture`'s 0xB2 red: vertex layer `GAME_LAYER_RED`), the flames of a burning
  mob (`FlameFeatureRenderer`: `fire_0`/`fire_1` squares turned about the vertical to the camera,
  `Mesh::flames`), the drops (iron ingots, poppies: a flower's item is its block texture's sprite,
  `BLOCK_SPRITE_ITEMS`), all from the vendored atlases (the overlay layers composited into one
  texture each at packing: `MOB_COMPOSITES`). The natural mobs a terrain view draws (2026-11-11,
  the parent's ask: they had been stand-in cubes; which kinds from `mctas web kinds` over every
  mob farm's 5 days) have their 26.3 models too, each its layer definition and `setupAnim` with
  its renderer's transforms (`entitymodel::{spider, creeper, skeleton, enderman, bat, witch,
  chicken, sheep, zombie_horse}`, the baby zombie's `BabyZombieModel`): the spider and cave
  spider (0.7) with their eyes layer and the enderman with its own (`EyesLayer`: the 26.3 `eyes`
  pipeline blends the texel times white over the model, no shading, no lightmap; its texels are
  opaque or empty, so the layer is the model's faces again at full brightness,
  `a_spiders_eyes_are_opaque_or_empty`), a dying spider falling onto its back
  (`getFlipDegrees` 180), the skeleton's arms hanging or, aggressive with its bow, raised along
  its look (`BOW_AND_ARROW`; redsim's `aggressive`, `MobState::mainhand`), the enderman's limbs
  at half swing and its carried block in its arms (`CarriedBlockLayer`, `carried_block_transform`),
  the bat's keyframe animations (`BatAnimation`: resting hung upside down, flying from the tick
  it took off, `AnimationState` kept per mob like the walk), the witch's pointed hat (a part
  added again keeps its children: the villager's hat rim and nose stay) and its nose wobbling at
  `entityId % 10`'s pace, the chicken's wings beating off the ground (`Chicken.aiStep`'s flap,
  kept per mob), the sheep's undercoat (dyed) and wool layers tinted by its colour
  (`ColorLerper.Type.SHEEP`, `coloredCutoutModelCopyLayerRender`) and its head while it eats
  grass (`EatBlockGoal`'s tick), the zombie horse walking (never rearing nor eating: redsim keeps
  no horse animation; its tail's swish is the client's own random), the zombie's arms higher
  when aggressive and a baby's down while it holds an item (`animateZombieArms`). Tests
  `a_spider_is_drawn_with_the_games_model`, `natural_mobs_stand_like_the_game` (each model's
  feet and top from its layer definition). A humanoid's (zombie, baby zombie, skeleton) item in
  its right hand and its armor (2026-11-11; `web kinds` lists what each kind holds and wears and
  where it is first seen: the terrain farms' skeletons hold bows, natural zombies iron spears and
  shovels, and they wear leather, golden or copper armor): the item through
  `ItemInHandLayer.submitArmWithItem` (the arm, a skeleton's a pixel in, a quarter turn back, a
  half turn, the hand's offset, a baby's smaller) and its model's `thirdperson_righthand` display
  (the bow's own, a spear's `spear_in_hand` with its `_in_hand` sprite, a tool's `item/handheld`,
  else `item/generated`), its sprite a flat double-sided square like a dropped item's (the game
  extrudes it a pixel deep); the armor as `HumanoidArmorLayer` draws it: each worn slot's
  `createArmorMeshSet(0.5, 1.0)` parts (the legs' the inner deformation) posed like the model,
  each layer of its material's equipment asset (`humanoid`, `humanoid_leggings`; leather's base
  dyed its undyed 0xA06540, then its overlay), neither under the red overlay (`NO_OVERLAY`). A
  still's mob wears what its data gives it (`equipment:{mainhand:..., head:...}`:
  `scene::still_equipment`); one redsim refuses to place (only a main hand's item is modeled in
  a design's data) is drawn as placed in a still (`Session::unspawned`: the page's first
  picture had dropped it; test `a_still_draws_the_mobs_redsim_refuses`). Their textures took the game texture past WebGL2's 2048: a shelf
  of entity textures that starts below the item atlas now starts at the left edge, and the
  texture is twice the block atlas wide (2048 x 1584 with the glint textures: the entity textures
  shelved beside the block atlas first; test `the_game_texture_fits_webgl2`). Tests
  `a_humanoid_wears_its_armor_and_holds_its_item`, `a_design_mob_wears_its_equipment`. A
  left-handed mob (redsim's `left_handed`, the spawn roll's 5%; a design's `LeftHanded:1b`) holds
  its item in its left hand: the left arm, the hand's offset the other way, its model's
  `thirdperson_lefthand` (else the right's) with `ItemTransform`'s left-hand fix (the
  translation's x and the rotation's y and z negated), a skeleton's bow aimed with its left arm
  (`poseLeftArm`'s BOW_AND_ARROW). A zombie (or zombie villager) holding a spear does not raise
  its arms (`animateZombieArms` skips a SPEAR arm pose): the humanoid walk swing, the spear's arm
  held out along the look (`SpearAnimations.thirdPersonHandUse`, not in use). The zombie villager
  and its baby (2026-11-11) draw with their own models (`ZombieVillagerModel`,
  `BabyZombieVillagerModel`: a villager's head, nose, hat and robe, a zombie's arms) and
  textures, their biome's type a layer over the whole model (`VillagerProfessionLayer`: redsim's
  `variant`, plains when it gives none; a baby's own type textures), the adult's armor on its own
  mesh (`ZombieVillagerModel.createBaseArmorMesh`). Tests
  `a_left_handed_mob_holds_its_item_in_its_left_hand`, `a_zombie_villager_wears_its_biome`. The
  sulfur cube (2026-11-11; cross-4-underground's, from tick 7964) is two translucent cubes, its
  inner body inside its outer (`SulfurCubeInnerLayer`, `SulfurCubeModel`; a baby's
  `SmallSulfurCubeModel`; `entityTranslucent`: quads sorted with the frame's other translucent
  faces, `entitymodel::mob_translucent`), sized and lifted by `SulfurCubeRenderer.scale` (test
  `a_sulfur_cube_is_two_translucent_cubes`). The wandering trader and its trader llamas
  (2026-11-11; no terrain farm's run has one: `web kinds` over the 15) draw as
  `WanderingTraderRenderer` (the villagers' model, its own texture; nothing while it is invisible
  after its night's potion: redsim's `invisible`) and `LlamaRenderer` (`LlamaModel`, its
  variant's texture from redsim's roll, the trader decor: `LlamaDecorLayer`'s `trader_llama`
  asset on the model grown 0.5) do, each llama's lead (`LeashFeatureRenderer`: 24 steps of a
  slack curve, two crossed brown ribbons, every other step darker, lit from end to end) from its
  attach point to the trader's rope hold (`Session::leash_draw`). Tests
  `a_trader_leads_its_llamas`, `a_wandering_trader_is_a_villager_and_hides`; `trader_scene_png`
  (ignored) draws the scene (redsim's spawns: no design's data can say a lead). The cat
  (2026-11-11) draws as `CatRenderer` does: `AdultCatModel` (the feline body, scaled 0.8:
  `CAT_TRANSFORMER`) walking, its variant's texture (redsim's `variant`, `CatSpawner`'s pick; a
  design's `variant:"minecraft:jellie"`; black without one: `Cat.DEFAULT_VARIANT`), no collar (a
  village cat is untamed); test `a_cat_wears_its_variant`. A skeleton drawing its bow (redsim's
  `using_item`: `RangedBowAttackGoal`) holds `bow_pulling_0`, `_1` from 13 ticks drawn, `_2`
  from 18 (`items/bow.json`'s `use_duration` at scale 0.05). A drinking witch holds its potion at
  its nose (`WitchItemLayer`, redsim's `witch_drink`), a wandering trader its potion, milk bucket,
  glass bottle or bucket in its crossed arms (`CrossedArmsItemLayer`, redsim's `hand`), each with
  its ground display; a potion is its bottle over its contents' overlay tinted by the contents'
  colour (`PotionContents.getColor`: water breathing, fire resistance, healing, swiftness,
  invisibility). An enchanted item or armor piece (redsim's spawn enchantments; a design's
  `"minecraft:enchantments"`) glints (`entity.fsh`'s GLINT: `GlintAlpha` 0.75 times the glint
  texture, squared and added over the lightmap; its coordinates the vertex's own through
  `setupGlintTexturing`: scaled 8 for an item, 0.16 for armor, turned 10 degrees, scrolled by the
  clock: the browser's, a native picture's the run's 50 ms a tick; a leather piece's base only):
  vertex layers `GAME_LAYER_GLINT_ITEM`, `_ARMOR`, the glint textures packed with the particles.
  A cube mob squishes (`AbstractCubeMob.tick` from its `onGround`, each tick in
  `MobRun::follow`: flattened at a landing, stretched at a leap; `applySizeAndSquish`). A hurt
  sulfur cube's translucent faces take the red overlay too. Tests `a_skeleton_draws_its_bow`,
  `a_drink_is_held_where_the_game_holds_it`, `an_enchanted_piece_glints`,
  `a_sulfur_cube_squishes`. In the 15 terrain farms' runs no skeleton draws its bow and no witch
  drinks; enchanted bows and armor show in five. A baby zombie's and a baby zombie villager's
  armor is the baby mesh (`HumanoidModel.createBabyArmorMeshSet`: a big helmet, the body and
  arms, the legs and a waist, two feet hung from the legs; the deformations (-0.1, 0.5, 0.3) and,
  for the legs, (-0.1, 0.3, 0.3): `Cube::grow_axes`) with the `humanoid_baby` textures (every
  slot), posed by the baby model's own `setupAnim`; test `a_baby_wears_the_baby_armor` (no baby
  in the farms' runs wears any). Not
  drawn yet: a zombie
  villager's profession (redsim keeps none), the farm animals' warm and cold variants (every
  terrain site is temperate), a sulfur cube's swallowed block (redsim refuses one), a boat's
  lead, a cat's crouch and sprint (redsim keeps neither). Lava is drawn as the game draws it: its
  `lava_still` texture, solid, lit by its own light 15 (it was water tinted orange). The time
  badge is the readout (`mobfarm::readout`): the game's day and clock, the golems so far and the
  iron in the farm's chests (`day 1, 13:23 · 2 golems, 9 iron`, two lines on a phone); the left
  badge says what the villagers do (`villagers: 3 asleep`, `2 panicking, 1 calm`), sized once
  from its widest texts. A farm plays at the speed that brings a golem every few seconds
  (`mobfarm::default_tps`: its card's seconds per golem over ~4 s: 10x for one module, 2x for
  the 2x2 farm; the speed menu gained 10x). Its blocks change seldom (a bed taken, a trapdoor,
  the dusk pulse) and its mobs every tick, so its blocks' mesh is kept apart from its entities'
  until the world logs a changed cell (`Session::blocks_version`, `World::log_writes`;
  `scene::Part`): the 2x2 farm's block mesh takes ~70 ms natively. `web check` (`mob_pair`)
  holds each archived farm's view to its judge: the viewer's constants and farm are the judge's,
  its world is redsim's run every tick (6000 ticks natively, 1200 in the shipped wasm through
  `mcview_mob_digest`), and over the judge's 5 days its golems and iron are the judge's run 0's
  (`farm::run`, with its checks; `farm::run_t` with the card's hand step for a farm that has one); `mctas web mobrate` plays all 64 of the judge's runs (8 builds
  x 8 seeds) through the view's own session (`Options::mob_seed`) and requires the card's iron
  and golems an hour exactly. Test `a_mob_farm_runs_like_its_judge`.
  The golem spawn spots (vjeux 2026-11-08: "show the places where golems spawn"): every cell the
  judge's golem search reaches around the villagers now (`mobfarm::spawn_spots`: redsim's
  `spawn::golem_spawn_cells` per villager, its 17 x 17 columns scanned from y+5 to y-7 to the
  cell `possible_spawn_position` stops at, with `checkSpawnObstruction`'s verdict), found again
  when a villager changes cell or a block changes (`MobRun::refresh_spots`). A cell a golem can
  spawn on (a mob or a boat standing in its box only for now counts too) is a translucent column
  the golem's height, seen through the blocks around it (a pit is walled in: `Part::xray`, the
  vertex layer's top bit `mesh::XRAY`, which the vertex shader draws at the nearest depth): sky
  blue until a golem spawns there, then yellow to red by the golems spawned there so far against
  the busiest cell (`mobfarm::spot_color`); a cell the search reaches but refuses is a dim grey
  plate on its floor; each spawn flashes on its cell for about a second of the view
  (`Look::spawn_flash_ticks`, from the speed). The bar's "Spawn spots" toggle, on by default for
  a mob farm (`Look::spawn_spots`), says what the colours mean in its title; a still shows its
  spots uncounted. `web check` plays all of the judge's runs through the view's session and
  requires every golem to spawn on a cell shown open in the frame before it or the one it
  appears in (`mobfarm::spawn_check_seeded`, `mob_spawn_spots_check`: iron-day1's 64 runs, 7903
  golems, 9 cells); test `every_golem_spawns_on_a_shown_spawn_spot` also holds the view's
  verdicts to `check_spawn_obstruction`.
* Minecart stations, auto-crafters and item elevators (clk-obs's rail study `circuit:
  cart-unloader|cart-loader`, clk-item's `circuit: crafter <result>`, clk-misc's `circuit:
  item-elevator N`; site 2026-10-21) are shown as built with their judge's chests standing on
  their cells (`circuit::STATIC_CHEST_KINDS`, `static_judge_chests`: every empty `items`,
  `items.<x>` and `output` port: a cart unloader's chest under its hopper, a cart loader's supply,
  a crafter's ingredient and output chests, an item elevator's input and output chests), no lamp;
  their judge's item runs (carts, ingredients, loads) are not replayed yet. `web check`
  (`static_pair`) checks the chests and that no lamp is added (parse only). Test
  `a_cart_station_shows_its_judge_chests`.
* Containers: on a shulker loader or unloader, what tells the story is labelled by default: the
  judge's port chests (their contents inside their tag) and the shulker boxes being filled or
  emptied; any other container there (hoppers, droppers, dispensers, chests, carts, chest boats)
  shows its label when tapped (a tap again hides it). On every other design all containers are
  labelled (an item counter's droppers hold its count). The bar's contents button (grid icon and
  the word "Labels", gold while all are labelled; critique: "a four-squares button with no
  label") labels them all or only the story's; the flip and contents buttons say their word while
  the bar has room for it (the flip button's goes first). A label is ONE compact
  pill, live every tick: per item kind its icon (the game's item sprite, else its block texture)
  and total count with its unit (`1,344 items`; a judge's port chest in stacks, `27 stacks`: how
  full the judge keeps it, critique); shulker boxes first, as `12/27 slots` filled for one box or `3 boxes`
  with a fill bar for several (none while they are all empty); at most two kinds, then `+n`. A shulker design's same-kind port
  chests that stand together share one tag with their letters (ITEMS A–C over the three chests
  a-c, their contents summed; OUT A, OUT B: `circuit::port_groups`); named outputs read `OUT A`,
  `OUT B`; a judge chest's tag says what the judge does to it (`kept full`: a loader's sources
  and every box chest, so their counts never go down; `emptied`: the outputs, the rejects, an
  unloader's item chests, a sorter's PASS and OUT: their tag shows the run's tally, everything
  that reached the chest so far, and says `received` (`Session::received`: what the judge's visits
  took out, summed by kind, as the judge counts it; critique: a sorter's PASS and OUT never showed
  a count, the judge empties them as they fill; boxes counted as boxes, `1 box`)). Tags and labels are laid
  out every frame so that none overlap each other, an outlined cell, the badges or the bar, and no
  leader line crosses a tag or label (`labels::Layout`): one that would collide goes higher or
  aside with a thin leader line back to its cell; a label with no free spot is left out
  (`mcview::labels`: what a label says and the layout; `overlay.rs` draws). Tags stay 4 px
  inside the view, and an output's tag never covers the block right above its cell (a clock's
  observer over its OUT lamp: critique, "OUT" read as the observer's): it goes under the cell; an
  input's tag stays over its cell (under it, a gate's A tag landed on its OUT tag). No tag or
  label covers another tag's point (QA at phone size, lock2-alt: CODE 1 laid over OUT's point
  left OUT no spot its leader line could reach, and OUT lay on CODE 1).
* For the page's tests the viewer names its parts: the bar `.mcview-bar`, the tick badge
  `.mcview-badge`, the status line `.mcview-status`, the tags' layer `.mcview-tags`, and every
  control that presses an input `data-mcview-input` (its index; the door's lever `lever`); the
  debug state (`?mcview-debug`: `data-mcview-state`) also gives a lock's frame turns
  (`frames=0,3,...`).
* A press on a pulse input the demo left on lets it go (a pulse cannot start on an input already
  on: the press did nothing and stopped the demo; QA at phone size, dff-lock's CLK, the dividers'
  IN); the next press is a pulse again.
* Outlines (live viewers; thumbnails none but a BUD switch's TARGET): the inputs lime (when the design runs: something
  to press), every other circuit port in its tag's colour: OUT orange, ITEMS gold, BOXES lilac,
  REJECT pink, TARGET cyan (shown as built too); an output's box red while the output is powered,
  filled red but over a lamp (the clock judge's): the lamp shows its own state, the game's 4 gt
  off-delay and all (critique: a 4-tick clock's lamp seemed to go dark under the box's fill; it
  stays lit in the game and in redsim while the clock's output pulses). A driven input (R7) gets one lime box around its
  cell, the judge's repeater and its switch (a red redstone block while the input is on), so the
  judge's equipment reads as part of the input it drives; the repeater, its support and the
  switch are drawn see-through (`Look::ghost`, `mesh::GHOST_ALPHA`: the test rig, not the build;
  redsim's blocks there all the same).
* Record (vjeux 2026-10-20: "add a record video button using mediabunny in the viewer"): the
  `● Record` pill over a live viewer's bottom right (above the bar; hidden where the browser
  has no WebCodecs `VideoEncoder`) records the viewer's own rendering of redsim's run from tick 0
  (blocks, entities, OUT and port boxes, tags and labels, the badges as shown): a separate player
  (the live camera, look, speed and demo) stepped exactly `1/30` s of game time per frame (exact
  partial ticks, never a screen capture of real time) for the whole run
  (`Player::recording_ticks`: the auto-demo's schedule; a door's demo cycle,
  `player::door_demo_cycle`; a clock's warmup and whole periods, at least one, about 8 s at its
  speed; a flying machine's flight; else 10 s; at most 20 s at the current speed). Each frame is
  drawn into the view's canvas and composed on a 2D canvas of the view's own pixels (at most
  1920x1080, both even): the 3D view, then the overlays painted from the page's own layout
  (`record::paint`: pills, text line by line, icons, the leaders and the output strip; the bar
  and the button are left out). The vendored mediabunny 1.62.0 (MPL-2.0,
  `crates/mcview-web/vendor/mediabunny/`, beside the bundle as `mediabunny.min.js` and
  `mediabunny.LICENSE.txt`, imported only when a recording starts: nothing at page load) encodes
  it: H.264 in MP4 (`fastStart: in-memory`) when the browser can encode it, else VP9 in WebM,
  4:2:0, 30 fps (`record::Encoder`), and the file downloads as `<design>.mp4|.webm`. While it runs
  the button shows the progress and a press stops it (no file); the live view comes back as it
  was. `mctas web record <url> -o <dir>` presses it in headless Chromium and keeps the file. A
  combination lock (its page stays paused: its judge's sweep is tens of thousands of ticks) is
  recorded with its own demo (`player::lock_demo`: each DIAL turned to its CODE, a right-click
  every 8 gt, open 2.5 s, one more turn: shut; the badge names each step); it was 10 s of a lock
  standing still. With `?mcview-debug` the viewer's `data-mcview-recorded` note ends in `grid=`:
  the last frame's colours as the encoder read them (8 x 8 mean sRGB cells).
* `mctas web recqa` (2026-10-21, phone size): a door, a clock, the TNT drop and a lock recorded in
  Chromium (VP9 in WebM: its headless shell has no H.264 encoder), Chrome for Testing (H.264 High
  in MP4, `ftyp moov mdat`: the file most visitors get) and WebKitGTK (VP9 in WebM through
  GStreamer; Safari encodes H.264): every file decodes whole, has exactly the frames the viewer
  made at 30/s, no black frame, is tagged tv range BT.709 (WebKit's transfer bt709, Chrome's sRGB)
  and its last frame decodes to the canvas's colours within 1-2 levels in that reading (the next
  best, BT.601, 1.4-2.3 off); each plays and seeks in every browser that has its decoder
  (Chromium's headless shell has no H.264 decoder, this WebKitGTK no GStreamer H.264 decoder).

## Only what redsim simulates

vjeux (2026-10-20): the 3D view shows only what redsim simulates exactly; anything redsim cannot
simulate is shown as built with a plain `not simulated: <reason>`, never animated by
viewer-side logic. What the viewer draws is redsim's state of the tick; what it adds is the
game client's own rendering of that state: interpolation between the tick's start and end
positions (`partialTicks`), a minecart drawn on its rail's line (`AbstractMinecartRenderer
.oldRender`, from redsim's position), an item's bob and spin (`ItemEntityRenderer`, phase 0), a
primed TNT's swell and flash (`TntRenderer`), fluid heights from their levels, the game's light on
every block face (below: a lit lamp lit flat at its own light, its sides still shaded by direction
like the game's; critique: "Lamps never look lit" was its texture), an explosion's particles after
each of redsim's blasts (the game's emitter and puffs, `HugeExplosionSeedParticle` and
`HugeExplosionParticle`: camera-facing squares of its 16-frame sprite, `mcview::particles`; a
mid-air burst breaks nothing and showed nothing); and annotations
(tags, labels, outlines, the OUT boxes red while their output is powered; the tags are the
controls a player uses and the outputs only, never one that names a block the view shows: vjeux
2026-10-21, "please remove the water label, we can see it"). Audit of 2026-10-20
(removed):

* the viewer's own primed TNT (`mcview::tnt`: a fixed kick, gravity, collisions, vanishing at
  fuse 0): deleted; primed TNT is redsim's `PrimedTnt` entity and explodes in redsim;
* falling blocks drawn from `y - vy` (a guess at last tick's height): drawn from redsim's
  position before the tick (kept by the session each tick);
* real-game recordings played for designs redsim could not run (`data-trace`): not played;
* a still shulker box added to shown-as-built shulker designs: not added;
* a run redsim reports an error in (`World::errors`: something it does not model) kept
  animating: it stops at the last exact state (`Session::halted`) and says so.

Kept (exact): the auto-demo replays the judge's own stimulus and events; the shulker designs'
chests are the judge's driver; the clocks' still frame and loop are ticks of redsim's run.
`Session::drawn_matches_simulation` checks it: every still block drawn is the world's block (a
quartz-like block simulated as stone drawn under its own name), nothing in an air cell, the
entities and falling blocks drawn are exactly redsim's at redsim's positions; `web check` runs it
on every tick of every archived design's run (circuits under the judge's stimulus, every frame;
clocks 120 gt; entity, TNT and shulker designs 400 gt) and fails on any difference.

## How it works

* `mcview::design` parses the design format (no dependencies, so it compiles to wasm);
  `mctas web check` compares it with `mctas::design` on every file.
* `mcview::session` places a design like its judge (doors: raw paste, wire shapes, self updates,
  settle; clocks: the clock judge's placement and `port start` kick) and steps redsim one tick at a
  time; its period detector is the clock judge's (`clock::sim`), and `mctas web check` requires the
  same warmup and period on every archived clock (2369 agree; designs redsim cannot simulate are
  shown as built). `mctas web check --wasm web/viewer/mcview_bg.wasm` also runs the detector inside
  the shipped wasm (natively, with the wasmi interpreter) and requires the same numbers: the
  browser simulates exactly like redsim.
* `mcview::replay` reads a real-game recording (JSON Lines from `mctas clock misc trace-export`:
  changed cells and every entity per tick) for the tools; the viewer no longer plays them (see
  [Only what redsim simulates](#only-what-redsim-simulates)).
* Entities (redsim's entity list: boats and chest boats, minecarts and hopper minecarts, item
  entities, armor stands; redsim's falling blocks) are summoned like the judge (after the blocks),
  simulated live and drawn interpolated between ticks; their clocks' period follows the real-game
  judge's rule for entities (block frames over 4000 gt, 3 periods:
  `clock::sim::entity_clock_period`), which `web check` compares too. Water heights follow the
  game (source 8/9, flowing (8 - level)/9, falling and bubble columns full, water under water
  full), with no faces between fluid cells.
* `mcview::entitymodel`: entities drawn like the game's client (read with `mctas src --client`):
  `BoatModel` (both paddles where `animatePaddle` holds an idle boat; the chest boat's chest),
  `MinecartModel` with its display block (a hopper minecart's hopper, 0.75 scale, display offset
  1), `ArmorStandModel` in the default pose (`ArmorStand.DEFAULT_*_POSE`, arms hidden, base plate
  square to the world), each through its renderer's transform (`AbstractBoatRenderer`,
  `AbstractMinecartRenderer.oldRender`: a cart on a rail follows the rail's line and slope,
  `ArmorStandRenderer`), the boxes as `ModelPart.Cube` builds them (UV layout, mirror), lit like
  `light.glsl` (two lights, 0.6 + 0.4 ambient); dropped items bob and spin like
  `ItemEntityRenderer` (a block item: its block model at the `ground` scale 0.25; any other: its
  item sprite, double-sided); falling blocks: the block model half a block back
  (`FallingBlockRenderer`). The textures are the vendored entity atlas (boats and chest boats
  of every wood, the minecart, the armor stand) and the vendored item atlas, packed below the block
  atlas in the game texture when a page first draws an entity (`entities.png`,
  `entities.min.json`, `items.png`, `items.min.json` in a `data-mcview-textures` directory);
  until then (or without them) the viewer's stand-in boxes are drawn. A still's mobs placed by
  the design (`entity: minecraft:villager ...`, a design redsim does not run) are drawn with the
  game's models standing at their `Rotation` (`scene::still_mob`), and a vehicle's
  `Passengers:[{id:"minecraft:villager"}]` seated in it, legs forward: a minecart's at its seat
  (0.1875 up), a boat's at its floor (0.25 up) (`scene::passenger_ids`; 2026-11-08: the portal
  carousel illustration's villagers in carts and zombie in a boat had been stand-in boxes or
  missing).
* A design that travels (`travel:` / `distance:`, the circuits' headers) runs in its judge's
  corridor (`circuit fly sim`'s box: the design, kick cell and `stop:` blocks swept `distance`
  cells, plus 3; a piston extender sweeps its N), never in its own box; `stop:` blocks are placed
  first; the camera follows a flying machine while it plays, over a track under its whole path
  (behind it for one flying up or down: `Look::track`, thin pale bars, a tie at every block; an
  annotation like the outlines, not a ground: QA at phone size, followed over the void it seemed
  parked); it loops where
  `circuit fly sim` ends the run (the corridor's edge, rest or a break). `mctas web check` runs
  every archived flying machine against `circuit fly sim`: same end tick and same frames, native
  and wasm.
* A circuit runs as its judge builds it: the judge's consumer lamp (`circuit::CONSUMER`) in
  every air output cell (`consumer_cells`: not for extenders and flying machines, nor where a
  piston may push a block in); the lamp shows the output (no orange marker there).
  `mctas web check` runs every archived circuit under its judge's stimulus against
  `circuit::run::sim_trace`: every cell's full state (comparator strength, container contents)
  at every frame agrees.
* `mcview::models` / `mcview::atlas`: block models as cuboids with per-face tiles (pistons and
  heads, observers, dust by power, repeaters by delay and lock, comparators by mode, lamps, bulbs,
  sculk, hoppers, doors, trapdoors, rails, ...). The tiles are the game's own textures, vendored
  in the repo (`web/textures/26.3`, the block atlas of the judge's version, served with the site;
  `atlas::vanilla` maps each tile to its texture, tinting the gray grass, leaves and water).
  Thumbnails wait up to 4 s for them; without them (offline, blocked, or
  `data-mcview-textures="none"` on the page's root element, which can also point to another
  directory holding `atlas.png` + `data.min.json` + `block_definition.min.json` +
  `model.min.json`) the hand-drawn tiles of `atlas::art` (colors
  measured from the game, shapes after its layout) and procedural ones are used. Native tools
  (`shot`, `compare`, `strip`) read the same vendored files from the repo (`--procedural`:
  the hand-drawn tiles). A live view's GPU reads the app's atlas when its context is made,
  not the one captured when the view was set up: the game's textures replace it while the
  context waits, and a captured one left every live view on the hand-drawn atlas's white
  game texture — flat grey blocks on every page (2026-10-24, "still blank"). The smoke
  catches it: the view's center third must step (mean adjacent-pixel step >= 1.0), not just
  have colors.
* `mcview::gamemodel`: the game's own block GEOMETRY, from the vendored block definitions and
  models of 26.3 (read with the atlas from the repo): variants by properties,
  multipart conditions (`OR`/`AND`, `a|b`), weighted variants picked from the block's position
  like the game (`Mth.getSeed` + Java `Random.nextInt`), model parents, `#texture` variables
  (26.3's `{sprite}` objects too), elements with their rotations (and rescale), faces with uv,
  rotation, cullface and tint (redstone power, plains grass and foliage), quarter turns and
  uvlock. Their faces sample the game's block atlas uploaded as one 2D texture beside the tile
  array (vertex layer 65535). Blocks without game geometry (block-entity renderers: chests,
  signs, banners, skulls...) and water keep the viewer's own models (`MCVIEW_NO_GAME_MODELS=1`
  natively: the viewer's models everywhere).
* `mcview::mesh`: face culling against opaque neighbors (and translucent faces between blocks of
  the same kind: water, glass), the game's lighting of every still block's faces
  (`BlockModelLighter`, `Builder::face_light`): smooth lighting (each corner the mean of the four
  cells around it in front of the face: their shade for the ambient occlusion, 0.2 for a block
  whose collision fills its cell but glass, and their light, `smoothBlend`), flat for a block that
  gives off light or whose model turns ambient occlusion off (a torch), times the face's direction
  shade and the lightmap; any flat face of a block with a full collision box is lit from the cell
  in front of it, even one inside the block (`faceCubic`: a slime block's core; it was 20% too
  dark); water's sides drawn from both sides with the flowing texture (`FluidRenderer`, one-sided
  against glass, ice, slime, honey and leaves); moving blocks (`PistonMovingBlockEntity`) interpolated between ticks like the client's
  `PistonHeadRenderer`, translucent faces sorted back to front (the game's order-independent
  blending in 26.3 reconstructs the same order: no difference left once the slime's light was
  right), a TNT duper's real ground (`mesh::floor`), the pictures' floor of grass blocks (`web compare`).
* `mcview::light`: the game's light around a build, for those faces: sky light (15 straight down
  every open column, one less per block around opaque blocks, one more through a non-opaque full
  block, a fluid or leaves, none through a slab's or a stair's full face) and the light sources'
  block light, over the build's box down to the ground it stands on (the judge's grass, one block
  of air under the lowest cell, or a TNT duper's own ground); the lightmap (`lightmap.fsh` at
  noon, the default brightness: level 14 is 0.908). Dark under overhangs like the real game
  (fidelity pass 2026-10-21, `web compare --still` over 67 picks, both sides through the pictures'
  encoder: the mean difference over the build fell from 2.76 to 1.31). Entities are lit by it
  too, each by the light of the block at its eye (`EntityRenderer.getPackedLightCoords` at
  `getLightProbePosition`: redsim's position and eye height, a sleeper's 0.2; a burning mob's
  block light 15), a mob farm's as well: built apart from its blocks each frame, its entities
  borrow the blocks' kept light (`scene::build_entities`; until 2026-11-11 they were lit as under
  the open sky, a cave's spider as bright as the night outside); a hurt mob's red and a TNT's
  white are mixed in before the lightmap (`entity.fsh`), and the eyes layers are never lit
  (`mesh::EMISSIVE`). Test `a_mob_is_lit_at_its_eye`.
* `mcview::render`: wgpu with WebGL2 in the browser (and Vulkan natively): MSAA, the tiles as a
  mipmapped texture array (no bleeding between tiles) beside the game texture, sky gradient, back
  faces culled in both passes like the game's chunk render types (a glass block shows its three
  near faces only), and the game's fog (`fog.glsl` + `FogRenderer.setupFog`): the overworld's
  environment fog (spherical distance, linear from 0 to 1024 blocks) and the render distance's
  (cylindrical distance `max(|xz|, |y|)`, from 6 chunks minus a tenth to 6 chunks; a zoomed-out
  camera pushes the render distance back so the build is never cut), toward the sky colour.
  The sky is drawn after the opaque faces, with a depth test (only a sample still at the far
  plane takes it), before the translucent ones: the same picture as drawing it first under
  everything (`framecost --check` compares every frame with `Renderer::sky_first`, the order of
  before: 0 pixels), its shader (the gradient, the stars, the sun and the moon) run on the sky's
  samples alone. 2026-11-09 (part 8): a frame's draw step in WebKit at phone size 14.2-14.4
  against 15.0-15.5 ms on D1U, 14.3-14.6 against 15.0-15.4 on bed-plate-two-clusters.
* The labels over a live view (`mcview-web::overlay`): an output tag's check for a block right
  above its cell asks the session about that cell alone (`Session::drawn_block_at`, what
  `render_blocks` draws there: test `session::tests::drawn_block_at_is_what_render_blocks_draws`),
  the whole set of still cells read only when it cannot tell. It read every still cell, a farm's
  terrain's landscape among them, each time the blocks changed: every frame of
  bed-plate-two-clusters at 20x, 2-3 ms of its frames' 20 in WebKit (part 9: the labels step 0.9-1.0
  against 2.5-3.4 ms, p50 18-19 against 19-23).
* The camera's points that can be extreme (`camera::hull_candidates`, a farm's 8 corners a cell:
  1.16 million for cross-4-underground) by sorting instead of maps (`hull_candidates_sorted`;
  whole-number points in a modest box by dense grids, `hull_candidates_dense`; tests
  `hull_candidates_sorted_are_the_hashed`, `hull_candidates_dense_are_the_hashed`), and the last
  big answer kept (the camera search and the default camera ask the same points): 2 x 800 ms ->
  ~100 ms natively, cross-4-underground's view ready in 2.2 s against 3.6 (part 10).
  The varyings are sampled at the centroid of the samples a triangle covers in a pixel, not at
  the pixel's centre (2026-11-08: an edge pixel read the atlas sprite next to its face, a line of
  that colour along every block edge, a grid on a stone wall, or a texel the alpha test drops, a
  hole there).
* `mcview::occlude`: a mob farm's blocks stay on the GPU (`Renderer::set_fixed`); once the camera
  rests, only those of their triangles it can see are drawn, by an index list
  (`Renderer::cull_fixed`, worked out over a few frames within a budget: `occlude::Cull`). Exact:
  a triangle is left out only when the GPU would keep none of its samples (it faces away, is out
  of view, or opaque faces surely cover every sample it could touch, by conservative tests at the
  frame's pixels in 2x2 cells, with margins past the GPU's snapping and depth precision; coplanar
  block faces merged into rectangles cover their shared edges together). A software GL (WebKit
  on a GPU-less box: the phone rig) sets up every triangle it is given on one thread: D1U at phone
  size draws 13,500 of its 49,000 triangles, the frame 32 -> 20 ms. `mctas web framecost --cull
  --check` draws each frame both ways and counts the pixels that differ (0; and the test
  `occlude::tests::culled_frames_are_the_whole_frames`).
* The blocks' translucent faces (a farm's water) stay on the GPU too (`Renderer::set_fixed_blend`,
  `Player::fixed_blend`: sorted back to front once for an eye and its blocks, not every frame),
  drawn first in the translucent pass, and the cull tests them too without letting them hide
  anything (`occlude::Cull::step_with`: drawn without writing depth, one behind the opaque faces
  draws no sample; back faces are culled in both passes). cross-4-underground at phone size keeps
  3,700 of its 12,900 water triangles; natively its frame at rest draws in 25 ms against 28 (and
  no longer sorts and uploads 39,000 translucent vertices a frame); `framecost --cull --check`
  compares those frames too (0 pixels on 10 farms with water).
* `mcview::remesh`: a mob farm's blocks' mesh is updated in place when a few blocks change (the
  cells whose drawn block changed since: `Session::take_changed_cells`, each read again with
  `Session::drawn_block_at`): the first pass of the changed cells again (`Builder::begin_item`,
  `forget_cell`), the light of the columns within 16 of them again with the light around them as
  it was (`Light::recompute_columns`: a change reaches 15 cells sideways at most and the sky goes
  down whole columns, so it is the whole box's light again, cell for cell; test
  `light::tests::the_columns_again_are_the_whole_again`; a region over half the box, or light
  that spread out of it, computes it all), and only the faces of the blocks within two cells of a changed block
  (a face's ambient occlusion looks one past its corner's neighbours along its normal:
  `face_light`'s `permeable`) or within one of a cell whose light changed are drawn again
  (`Builder::emit_item`), spliced into the mesh in place of their old ones (the whole build's
  ranges: `Builder::ranges`, `Staged::keeping`). Anything else (a moving block, the light's box
  or floor moving, an output's box, a TNT duper's ground, too many cells) builds it whole; a
  staged whole build is no longer restarted by each change (it finishes, the next starts from
  the blocks of then). While the camera rests the blocks drawn change at most every
  `BLOCKS_HOLD_MS` (500 ms) so their cull is done in between and used for the frames after.
  2026-11-09: farms in their terrain whose blocks change every few ticks (grass dying and
  spreading, pressure plates) had never finished a mesh in the browser at 20x
  (bed-plate-two-clusters, cross-4-underground: 0 of 200 frames, `framecost --budget 10`; now
  every change in place, bed-plate's phone frame in WebKit p50 30 -> 19-23 ms with its blocks
  up to date). The ignored test `remesh::tests::updated_in_place_is_built_whole` holds every
  update to exactly the whole build's triangles (143 on bed-plate-two-clusters, 8 on
  cross-4-underground with its fires' light, 10 on dust-pulse-out-of-sight). Also: a staged
  build's calls no longer rebuild the cells' state map and item list each (it took a frame's
  budget on a 200k-block terrain), and a water cell no longer copies the waterlogged set.
  `?mcview-perf`'s steps line counts the frames drawn culled, the blocks' meshes that came and
  the frames held for the cull. 2026-11-09 (part 5): an update that changes no face keeps the
  mesh, its key and its cull (no upload, no cull again); the splice copies the old faces in runs
  between the fresh ones; cross-4-underground's update natively: light 8-10 -> 2 ms, the splice
  1.2-4 ms (`MCVIEW_REMESH_LOG` prints each update's parts).
* The cull carried over an update in place (part 6, `Renderer::carry_cull` with
  `remesh::Delta`, `Player::fixed_delta`): the frame right after an update draws the last cull's
  visible triangles where they are now, every triangle of the blocks drawn again, and every
  triangle whose place on screen meets the screen box of the opaque faces that changed (old and
  new, 4 pixels out): occlusion changes only where opaque faces changed, so it is a superset of
  what the camera sees and the same picture, at once, while the exact cull of the new mesh runs
  on (staged) and replaces it. bed-plate-two-clusters carries 21,000 triangles (the exact cull
  14,400, all 51,900), cross-4-underground 43,300 (40,900 of 164,000); `framecost --cull --check`
  compares each carried frame with the whole one too (`carried over N times`). WebKit at phone
  size, every frame of the run culled (cross-4-underground 127 of 127, against 109 of 123):
  cross-4-underground p95 57 against 65 ms, bed-plate-two-clusters p95 36-40 against 35-43.
  Part 7: only the blocks whose faces changed count as changed (a block drawn again the same, its
  neighbour changed, keeps its triangles' cull), each one's own screen box (tiles of 8 pixels,
  not one box around them all); the still translucent faces keep their sorted list, its key and
  its cull when none of theirs changed, and carry their cull too; and a carried cull within a
  quarter of the last exact one's triangles is kept as it is (no exact cull until the camera
  moves). From the start, 100 frames of 10 ticks: exact culls 64 -> 25 on bed-plate-two-clusters,
  9 -> 1 on cross-4-underground, 46 -> 21 on bed-plate-lava-in-sight; WebKit p95
  cross-4-underground 35/46 against 34/57 ms, bed-plate-two-clusters 36/33 against 40/36.

## Commands

| command | what |
|---|---|
| `mctas web shot <designs> -o <png or dir> [--live [--dpr 2]] [--crop x,y,w,h] [--part nether]` | offscreen PNG with the browser's renderer (Vulkan; lavapipe on a GPU-less box: `dnf install mesa-vulkan-drivers`): a thumbnail's picture, or with `--live` a live viewer's of that size (the camera it frames on load, and its outlines; `--dpr 2`: a phone's, its CSS size half the picture's, e.g. `--width 654 --height 960`); `--crop` keeps a part of it (a mob model, a flame); `--part nether` pictures the design's Nether part as a page's `data-part="nether"` viewer draws it (a mob farm's kill and storage: the farm part pages give it a second viewer) |
| `mctas web check <designs> [--wasm <mcview_bg.wasm>]` | parser and clock-period agreement with the repo's tools (and with the shipped wasm) |
| `mctas web mobrate <designs> [--threads 32]` | a mob farm's judge runs (8 builds x 8 seeds, 5 days each) played through the viewer's own session: their mean iron and golems an hour against the card's, and the page's run alone; fails unless they equal the card's |
| `mctas web mobdiff <design> --build B --seed S [--ticks 120000]` | one of a mob farm's judge runs (its build B of the 8, seed S) played both ways, the judge's runner (`farm::Run` with the card's hand step) and the viewer's session, every tick's worlds compared (time, mob random, every entity's kind, position and motion): the first tick they part and how (or the view stopping, with the judge's world errors then), else the same to the end; fails when they part. 2026-11-09: D1U's build 7 seed 1769114314 had stopped `web check` at t103206 and looked like a wasm divergence; both runs were the same every tick and both hit natural water past the sample's edge (fixed in b686373716); `cargo test -p web-cli -- --ignored d1u_build7` keeps it so |
| `mctas web compare <designs> -o <dir> [--zoom x,y,w,h] [--floor-margin m] [--still] [--before <dir>]` | real-game picture (`designs/clocks/img`) next to the viewer's frame of the same moment and camera (without the viewer's input outlines) and where they differ (red), with the pixel difference, the difference over the build alone (the grass's noise left out) and the edge offset (where ours sits against the picture, 1/8 px steps); ours is the game's whole window passed through the pictures' own encoder (`clock::shot::encode_picture`: the same integer crop, lanczos and JPEG; resampled apart, it sat a third of a pixel off); `<dir>/sheet.png` and `report.tsv` hold them all; `--zoom` writes a 4x crop of both and their mean colours, `--floor-margin` tries another floor reach, `--still` takes still frames from `web snapshot` as written (tick 0), `--before` an earlier run's folder (another viewer build): `before-after.png` and both means |
| `mctas web snapshot <designs> -o <dir>` | each design's still frame in the viewer (a door at rest, a circuit settled with its judge's rig, a clock at its still moment) as a plain design of the blocks drawn there, at the height the real game pictures it (lowest cell y = 72: the same random block variants); `mctas clock shot --still` pictures them in the game, `web compare --still` in the viewer: the fidelity pass's pairs (same blocks, same camera) |
| `mctas web kinds <designs> [--ticks N]` | each view played as the page plays it (default 120000 ticks: the judge's 5 days), every entity it draws: each kind (a mob or not), the most at once, the first tick and where (`--look-at` for a frame of it), the ticks played and why it stopped early; what a mob kind holds and wears (`holding minecraft:bow @tick x,y,z`, `wearing golden head @...`: each first seen); the census behind the mob models (`tools::drawn_kinds`) |
| `mctas web strip <design> -o <png>` | a grid of frames over time (a door from its flip, a clock from its still moment), half ticks apart: the animation |
| `mctas web sheet <pngs> -o <png>` | textures scaled up in a grid, to compare them by eye |
| `mctas web bundle -o web/viewer` | the wasm bundle (needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version =0.2.129`) |
| `mctas web framecost <designs> [--width 798 --height 960 --dpr 2] [--warm 400 --frames 40 --ticks 1] [--cull [--check]] [--budget ms] [--png-dir dir [--yaw deg --pitch deg --zoom f --look-at x,y,z --brightness b]] [--drive F [--follow eN[.pK]]]` | a live viewer's frames natively (lavapipe without a GPU): the mesh and draw time a frame, the vertices kept on the GPU and uploaded, the blocks' mesh rebuilds; `--cull` with the occlusion cull (`mcview::occlude`), `--check` each frame also drawn whole and the pixels that differ counted (must be 0); `--budget` each frame's staged mesh work cut after that many ms as in the browser (about a third of the browser's 24: native runs 2-3x faster), counting the blocks' meshes finished and the frames spent on one; `--png-dir` writes each frame's picture (`frame_00001.png`, ...: a film of the live view, its sky at the run's day time; `ffmpeg -framerate 30 -i dir/frame_%05d.png -vf scale=out_range=tv,format=yuv420p -c:v libvpx-vp9 -b:v 0 -crf 32 out.webm` makes a phone video), with the camera at `--yaw` / `--pitch` (framed as a page's `data-yaw` / `data-pitch`), `--zoom` times closer and aimed at `--look-at`; `--drive F` plays a part's bench run (its `--drive-out` file, as a page's `data-drive`), `--follow eN` / `eN.pK` aims the camera at that drive entity every frame, found by its id at the session's start (a film's follow shot: a napper riding its cart, through the siding, the bed and back), `--brightness b` draws with the game's Brightness option (0 Moody .. 1 Bright, 0.5 the game's default: a night's film at Bright, named on the film) |
| `mctas web unsimulated [--dir site] [--threads 32]` | every page of a built site whose 3D view says "not simulated" (vjeux 2026-11-10: none should): each page's full viewers (its design, its Nether part, its bench's drive, its farm's speed) built natively as the browser builds the live view's session, with the look-ahead; the views it does not run grouped by the reason their badge shows, and the views the site keeps as built (`data-asbuilt`) |
| `mctas web drive-check <design> --drive F` | a part's page plays its bench's run (`data-drive`: farmopt's `linear-bench` / `station-bench` and ironfarm's `queue-loop-run` write it with `--drive-out`; `redsim::entity::mob::drive` replays it: the benches' start (`Run::start`, the settle, the daylight detectors the start held back placed raw then updated as `Run::arrive` places them, then the entity lines), each `act` before the tick, `eN` the design's entity lines from 0): the view's own session run natively for the bench's ticks and held to every `expect` line (golems' ticks and cells, crossings, Nether deaths, carts' positions, the end's totals); the carousel's linear stand: 239 golems, 238 crossings, 236 Nether deaths and its Nether iron, the bench's; a drive's `# caption <words>` (what the bench drives) is the view's badge while it plays |
| `mctas web demo <designs> -o <dir> [--autoplay]` | a standalone demo page (live viewers + a thumbnail grid) |
| `mctas web single <page.html> -o <file.html>` | a generated page (or one laid out like one: a `data-mcview-textures` root, `data-src` viewers, the bundle's module script) as ONE self-contained file to share where only a file goes (an artifact): the viewer's script, WebAssembly and textures, the design and terrain files it fetches and the page's style inlined; a `fetch` shim serves them from inside the page and the viewer's modules load from blob URLs (2026-11-08: the portal carousel illustration, 18 MB) |
| `mctas web serve <dir> --port 8000` | serve it on localhost |
| `mctas web layout <url> [--window 390,844 --scale 2]` | layout-shift check: the page's live viewers (give them `data-autoplay`) in headless Chromium, each brought on screen in turn until it ran `--min-ticks` (a viewer plays only on screen); fails if any viewer, bar, counter or badge box moved (relative to the viewer) |
| `mctas web still <designs...>` | each design's thumbnail `data-still` attribute (`mcview::tools::still_attr`: the still frame found ahead), one `<path>\t<attribute>` line each (`-`: the design does not load in the viewer) |
| `mctas web qa --dir site/ [--kinds] [pages...] --out <dir> [--window 390,844 --at 2,8 --watch 24 --shards 4]` | the live viewer's visual QA at phone size in WebKit (WebKitGTK through the site's rig): each page's first live viewer on screen, screenshotted `--at` s after it is ready and cropped to the viewer; its tags (none over another, the badge or the status line, under the bar or cut by the view's edge); its ticks over `--watch` s (it plays; a demo restarts: when); the picture's brightness; every input control pressed once (the build reacts: its input levels, powered blocks, door or frames change); the console. `--kinds`: every circuit kind of the site's `data/site.json` (up to 4 designs: smallest, middle, largest), every door, a clock per common mechanism (`--clocks 18`). Writes `<page>-<k>.png`, `sheet.png` (every page's viewer and findings) and `report.txt` (2026-10-21: 104 pages, 16 with findings before, the lock tags, the pulse inputs, the dropper stop; see the commit) |
| `mctas web perf <url>... [--engine webkit --dir site/ [--trace]] [--window 375,812 --scale 3 --cpu 4] [--load 20 --scroll 8 --speed 2500]` | a page on a phone profile: DOMContentLoaded, load, first cards, first 3D view, the first screen's views all drawn, long tasks and frames over 50 ms; then a scroll at `--speed` px/s: its frame times, the views drawn meanwhile (and how many already off screen), how long the views on screen when it stops take; with `?mcview-debug` each thumbnail's time; with `--trace` (WebKit) each load frame over 50 ms and the viewer's work in it (`?mcview-trace`), and when the viewer's module started. Chromium (the CPU slowed `--cpu` times, touch, an iPhone user agent) or WebKitGTK (`--engine webkit`: the site's WebKit rig, `url` a page path in `--dir`). Several pages: each in turn, then a table of their first frames (the first 3D view drawn, the first screen's views all drawn), the slowest first. `--trace-over <ms>` (default 50; 0: every frame) picks the frames `--trace` prints; `--trace-plain` runs the page without `?mcview-debug` (a visitor's frames: the trace's own spans, no debug log lines nor layout reads). `MCTAS_RIG_LATENCY_MS=<ms>` in the environment answers every request of the WebKit rig that many ms late (a phone's round trip to the site) |
| `mctas web click <url> [--touch --window 390,844 --scale 2]` | input check through real browser input events (DevTools protocol over a pipe): clicks (or taps) each live viewer's lever or button while paused and requires it to flip, the simulation to start and the build to react (a door opens, a lamp lights) |
| `mctas web snap <url> -o <png> [--seconds 6 --scroll 900 --eval <js> --before <js> --before-at 4]` | a screenshot after some seconds of real time (playing viewers and demos advance), with the page's console lines (and a JavaScript value); `--before` runs JavaScript after `--before-at` seconds, then waits `--seconds` (an in-place swap: set `data-keep-camera` and `data-src`) |
| `mctas web click --hold <url> [--touch ...]` | presses and holds each live viewer's circuit input in the 3D view: on while held, off once released, the camera unchanged |
| `mctas web record <url> -o <dir> [--index 0 --window 1280,720]` | presses live viewer `index`'s Record button in headless Chromium and keeps the video it downloads (`<design>.mp4` or `.webm`) |
| `mctas web recqa --dir site/ <pages...> -o <dir> [--engines chromium,chrome,webkit --chrome <chrome-headless-shell>] [--window 390,844 --scale 3]` | the Record button's files at phone size: each page's live viewer recorded in each browser (the blob it offers caught, its download suppressed); every file played and seeked in every browser (from a blob, as the download is; a browser without the codec's decoder is noted, at least one must play it) and checked natively (ffprobe, ffmpeg): codec and size as the viewer said, exactly its frames at 30/s, none black, the MP4's `moov` before its `mdat`, and its colours: the last frame under each range and matrix against the encoder's input (`grid=`), its tags naming the reading that matches. Writes the files, `sheet.png` (frames at 0, 1/4, 1/2, 3/4, the end) and `report.txt`; fails on any problem |
| `mctas web luma <designs...>` / `mctas web luma --url <page>` | mostly-black guard: every design's still rendered natively, or every viewer of a page measured in headless Chromium through the real bundle (the WebGL path: a texture the GL backend misreads only shows there); fails under a mean luma of 40 or over half near black |
| `mctas web reference -o <dir>` | LOCAL ONLY, never committed: the game's block textures from the client jar, to measure colors (`mctas web palette`) and compare (`mctas web tiles`, `mctas web sheet`) |
| `mctas web game-texture -o <png>` | LOCAL ONLY: the game texture the viewer samples (block atlas, item atlas and entity textures packed below it), with where each entity texture went |
| `mctas web entities <design> [--trace designs/clocks/traces/exact/<stem>.trace] [--from T --to T]` | the viewer's simulated entities (redsim: end-of-tick positions and yaw) tick by tick next to a real-game trace's, with the largest distance: all 55 exact traces 0.0000 over their whole length (a minecart on a rail is DRAWN at the rail's line, like `AbstractMinecartRenderer.oldRender`; that drawn pose is not what is compared) |

A page can be checked without a desktop: `headless_shell --use-angle=swiftshader
--enable-unsafe-swiftshader --virtual-time-budget=60000 --screenshot=x.png <url>` (EPEL package
`chromium-headless`) runs the bundle with WebGL2 in software (`--window-size=390,844
--force-device-scale-factor=2` for a phone); add `?mcview-debug` to the page URL for timings in the
console (`--enable-logging=stderr`). Animation frames do not advance under a virtual time budget,
so a screenshot shows tick 0 of a playing viewer. `?mcview-perf` logs a playing view's frames every
5 s of play (how many, the ticks they ran, their mean and longest time) without the per-frame
lines of `?mcview-debug`: `mctas site smoke --engine webkit --page '<page>?mcview-perf' --frames 2
--interval 8000 --click 'css:.mcview-bar button[title="Play / pause (space)"]'` prints them (the
iron farms, 2026-11-08, WebKitGTK in Xvfb: one module at 10x 60 frames a second, 6-11 ms a frame,
the longest 46 ms past the wake; the 2x2 farm at 2x 42-54 a second, 14-18 ms, the longest 57 ms,
its first block mesh).
