# Tools

Every tool is a `mctas` subcommand, written in Rust (`./target/release/mctas ...` after
`cargo build --release`); `--help` on any command lists all of its flags. The rules the judge
enforces are in [rules.md](rules.md); how the simulator, the bridge and the videos work is in
[internals.md](internals.md). The clock study's commands (`mctas clock sim | verify | ...`), its
judge and its design format are in [clocks.md](clocks.md). The website of every build
(`mctas site build | publish | serve | smoke | deploy`) is in [site.md](site.md); Litematica
schematics of any design (`mctas litematic export | read | check | verify-ingame`) in [litematic.md](litematic.md).
The bedrock study's commands (`mctas bedrock scan | watch | vanilla | vanilla-matrix | design |
judge | minimize | graft | row | circuit | scenario`: bedrock breakers, the portal method, the
official server's checks) are in [designs/bedrock/NOTES.md](../designs/bedrock/NOTES.md#tools); its
circuit kind (`circuit: bedrock-breaker N`) is judged like every circuit (`mctas circuit verify`).
One tool is not a `mctas` subcommand: `trends` (`./target/release/trends suggest | related | rank`,
crates/trends), the Google autocomplete and Google Trends ranking of what people search about
farms and redstone. It needs the internet, so it is never used for design work; it is in
[search-demand.md](search-demand.md).

## The workflow behind the records

1. Validate a mechanism with ideal inputs: redstone blocks placed and cleared by a `door drive`
   script, or a `door ideal` schedule. (`door mech-synth` finds mechanisms and writes both.)
2. Wire it, by hand or with `door route` / `door route2` / `door mech-wire`.
3. Run `door verify <design> --extra-offsets 28` (the judge, 32 positions).
4. Shrink it with `door minimize`, `door resynth`, `door grow` and `door regrow`.
5. Keep it: `door verify ... --archive` copies it into the archive with a header.

Proven circuits go in the gadget library, `designs/gadgets/` (a small design plus a drive script
whose `expect`s prove the behavior; catalog in [mechanics.md](mechanics.md#10-gadget-catalog)).
Run them all with `mctas door drive --quiet designs/gadgets/*.drive`.

## Setup and the game's source

| command | when to use it |
|---|---|
| `mctas setup` | once per machine: installs Java 25, the 26.3 server jar and libraries, and compiles the bridge (everything lives in `$MCTAS_HOME`, default `~/.cache/mctas`) |
| `mctas src decompile` | once: readable game source in `$MCTAS_HOME` (Vineflower) |
| `mctas src grep\|find\|show\|path` | to read the ground truth for a mechanic: `src grep 'class PistonBaseBlock'`, `src show PistonBaseBlock --grep getNeighborSignal -C 12` |
| `mctas src --client decompile\|grep\|find\|show` | the CLIENT jar's source (models, renderers: `src --client show BoatModel`), decompiled once from the client jar already downloaded (`versions/<v>/client.jar`) |
| `mctas sim repl` / `sim place` | talk to a simulator in JSON lines ([internals.md](internals.md#bridge-protocol)) / place a design and show what is pending |
| `mctas bench` | measure throughput and memory with many workers and sandboxes |

## The judge and looking inside a door

| command | when to use it |
|---|---|
| `door verify <design> [--extra-offsets 28] [--timeline] [--cycles N] [--archive] [--no-rotations] [--orientations] [--tile <spec>]` | the judge ([rules.md](rules.md)); prints blocks, category, open/close ticks and the rotation check. `--archive` keeps a copy of a working design in `archive/` next to it (skips duplicates); `--cycles 40` catches slow failures like oxidizing copper; `--orientations` runs all 8 orientations; `--tile wide:N\|period:P:N\|stack:N\|vperiod:Q:N\|step:dx,dy,dz:N` judges the design as a tile module ([rules.md](rules.md#tileable-doors-door-verify---tile); `--tile-out` writes the arrangement) |
| `door trace <design> [--close] [--ticks N]` | print every block change tick by tick after each lever flip: how a door actually moves |
| `door drive <script>...` | place a design, apply scripted inputs (`set`, `clear`, `use`, `step`, `quiet`, `print`, `plane`, probes, `expect`), print a plane per tick; any failed `expect` fails the command. Validates mechanisms with ideal inputs and runs the gadget tests. Block entities (item clocks): `place <design>` places like the clock judge without settling (a design's `nbt:` lines merged after the raw paste), `data x y z <snbt>` (= `data merge block`), `items x y z`, `probe_items <name> x y z` (a container's item count) |
| `door ideal <schedule> [--set name=value] [--sweep name=lo..hi] [--between-ticks]` | judge a mechanism driven by scheduled ideal inputs (`ideal.rs`) and sweep its timing windows: which input ticks work. Since 2026-10-19 an off-plane redstone-block input at `t >= 2` acts in the TILE-TICK phase of tick `t`, where a wired observer's output arrives (built from a wall torch on a support block, so the game and redsim agree); the old rule (between ticks, after tick `t`: 1 gt optimistic for observer-driven inputs) is `--between-ticks` or a `phase between` line in the plan; other tools that parse plans keep the old rule unless the plan says `phase tile` |
| `door ideal-rs <schedule> [--set] [--sweep] [--vary x,y=state\|state\|keep\|air] [--vary-max K] [--without x,y,z] [--drop N] [--ok-only] [--trace]` | the same on redsim in milliseconds (`ideal_rs.rs`); `--vary` screens every combination of block alternatives per cell (in memory: e.g. sculk sensors, observers, wool around a missing input: 10-20k variants/s), `--vary-max K` keeps only combinations changing at most K of the cells (pruned while building: every single or pair of added blocks over a whole region, e.g. 20 cells x 6 states = 6961 plans in 0.3 s), `--drop N` every removal of up to N blocks (a redsim minimizer), `--without x,y,z` (repeatable) drops every plan event at that input cell (the input left to the `--vary` filling: a region screen without editing the plan). Found bridge-p2-sculk and bridge-e88-sculk ([tracks/bridge](../designs/piston-door-3x3/tracks/bridge/NOTES.md)) |
| `door feeds <schedule> [--box X0..X1,Y0..Y1] [--sculk] [--strict]` | per firing of an ideal plan (each redstone block it places): the pistons it powers and where a FLAT build could feed them (`feeds.rs`): cells that stay free all cycle inside the box (regions, their sizes and their cells; cells only the plan writes count as free) next to the piston (not its front), quasi-connectivity into the cell above it, a conductor block powered from a free cell, or a fixed observer of the design pointing in; firings with no option are listed last (cores for static wiring). Not checked: component support, co-firing, timing. On d67's q7b: 3 of 42 firings unfed (b at (0,-2) twice, SU(1,-2) at open). Regions: the cells each free region can watch and their first change per edge; a region that is the only feeder of a firing but sees no change 2 gt before it cannot start (free regions are electrically isolated: no path crosses a piston or two blocks; the lever's own input is exempt). On c70b's u0-7x7: (0,-2), the top-right and the bottom-right never start (d67 NOTES "Wiring u0-7x7"); pockets3's `cosearch pockets` ISLAND START is the replay-based version. `--sculk`: per free cell, when a sculk sensor there (normal: radius 8, active 30; calibrated: radius 16, active 10) would activate on each edge, from the plan's own vibrations (lever flips, piston extend = activate 10 / retract = deactivate 9): the first event heard while idle commits it, activation T + max(1, floor(d)), deaf until INACTIVE again (d67 NOTES "Sculk on u0"). `--strict`: the screen for searches' exports (macrosat --strict SATs before they go to wiring): a `via` conductor must stay the same block all cycle (`*` otherwise), and the plan FAILS (exit code 1, one `# strict:` line with the reasons) on a firing without a feed, one with flagged options only, a one-cell conflict or a region that cannot start; macrosat's m86-a fails it (15 weak, 7 starts), pockets3's wired e78k-s11 passes |
| `door shift-plan <schedule> [--shift x,y,z=k\|@name]... [--rest k\|@name] [--out F]` | re-time a plan: every event of the listed input cells moved by its shift, the others by `--rest`; `@name` writes `name+<tick>` and a `let name = 0`, for `ideal-rs --sweep name=lo..hi` (e.g. one area started later than the lever, a stack's levels at any tick: m69T 1681/1681) |
| `door prune-plan <schedule> --out F [--only x,y,z]... [--keep x,y,z]... [--exhaustive] [--extra-offsets 28] [--cycles 2]` | drop the input pulses a plan works without, TOGETHER: greedy, latest first, each drop screened at 4 positions and confirmed at 32 (pulses `door shuttle` lists as spare one at a time often can't all go); `--only` limits the drops to one area's input cells, `--exhaustive` tries every subset of the candidates (<= 16) for the largest one; level edges, lever uses and `--keep` cells stay. It checks the mechanism only, not island starts: `--keep` any pulse a wiring area needs to SEE as its start trigger (gf16: SW(4,1)'s, pockets3). Lighter areas for pockets / satwire (q69c-sh0: 4 close pulses) |
| `door cosearch pockets <flat design> --assist <plan> --box X0,Y0:X1,Y1 --focus X0,Y0:X1,Y1 [--keep-lever] [--offplane] [--k 1 --only-pockets 99] [--why-islands] [--sculk-islands] [--pin-assist x,y;..] [--fill-palette all,sculk,calibrated,wool]` | wiring screens of an ideal plan (track pockets, `cosearch_search.rs`): the free areas ("pockets"); FEED (per input: the pocket cells next to its piston or its QC cell, a lever outside the box on a never-moved piston's back); LOAD (per pocket: the distinct ticks only it can fire); ISLAND START, timed and co-fire aware (a pocket fires only after a change it can see, caused by a pocket already started, or the lever; a direct feed also QC-powers the piston below, a QC feed through a piston's cell powers that piston; prints the pockets that never start in time and the plan groups no area can fire, `--why-islands` the trigger of each); then EXHAUSTIVE fillings of every pocket up to `--k` blocks, each screened in the whole door with the other inputs ideal (which plan groups it reproduces), and combinations across pockets. `--k 1 --only-pockets 99` = the reports alone (seconds). Wants per-piston inputs (mech-synth style); macrosat exports go through `door ideal-rs --assist-dir` first |
| `door shuttle <schedule>... --target x,y,z[;x,y,z] \| --unfed \| --spare-only [--per-cell] [--edge open\|close] [--partial] [--train] [--absorb] [--payload observer,redstone_block] [--radius 3] [--holds 3,4,5,6,8,10,12] [--out-dir DIR]` | feed pistons that have no free neighbour (a wiggle piston boxed in the frame, a piston on a drop cell) from a SHUTTLED observer or carried redstone block (`shuttle.rs`, track clk-rt): a sticky piston carries the block one cell out and back, and a moved observer fires 2 gt after it lands, into the cell behind it, so one shuttle pulse gives two observer pulses (arrival, return). Every placement (piston cell and facing, carried block and facing, free landing cell, near a fed piston) and per-pulse arrival/return timing is judged on redsim with the target inputs removed, then at 32 positions; `--absorb` greedily drops other plan pulses that the observer's free pulses or extra shuttle pulses give, and lists pulses the plan works without anyway; `--unfed` targets every input `door feeds` finds no option for, `--per-cell` searches each target alone (batch mode over many plans: one line each). Target pulses the plan works without are reported first (each alone, then the largest set dropped together, confirmed at 32 positions; `--train` drops a tick only with both edges' copies, for one-train plans) and `--spare-only` prints just that report for every input: free load cuts (fly's r69t: 9 pulses, q68 / q69c / q69h: 4). `--partial` keeps shuttles that replace only some target pulses (load relief). Re-finds mm01z (c70b's 7x7, P + 5 of Q's pulses) and s69z (fly's 6x9, Qv) in under a second; on fly's s69 a riding observer feeds Qv o6/o18, B1 o24 and Qv@ o30 (wip/s69r). In-plane plan inputs block shuttle cells: move them behind their pistons first (`ideal-rs --offplane-dir`) |
| `door reuse-check <design>` | verify many times in one reused sandbox: catches state leaking across runs (torch burnout memory) |
| `door frontier [--check <file>...] [--any-category] [--all]` | the Pareto frontier over (open, close, size: the volume) from the verdict and properties caches; `--check` says whether a candidate lands on it or which doors dominate it |
| `door shapes [<design>...] [--branches] [--rule <preset or key<=n,...>]` | every working design's shape and swept shape relative to its door, the best rotation-safe design per shape rule (wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground) and league, and the (volume, blocks) frontier |
| `door tile-scan <design>... [--tiles wide:3,period:4..12:3] [--levers one\|each]` | every design under every tiling, in parallel: which tilings work and the smallest working period |
| `door layouts [--size 7x8x1] [--category solid] [--out FILE]` | the distinct PISTON LAYOUTS among the working doors of one size (main, origin/main and `explore/*` designs the leaderboard caches judge WORKS with those volume dims): a layout = the pistons at rest (position from the door, facing, sticky), wiring ignored; identical layouts grouped, left-right (and up-down) mirrors paired, layouts that contain a smaller found layout listed under it as its core; per layout doors, best blocks, best open/close, an example, and the first find (earliest commit carrying the exact file of one of its doors, and its track). Reads caches only (no simulation, ~15 s) |
| `door leaderboard [--fetch] [--sync-tracks] [--archive-dir DIR] [--cache-dir DIR] [--cached-only] --out designs/piston-door-3x3/LEADERBOARD.md` | judge every design on main and `explore/*` with main's verifier (verdicts cached by content in `archive/verdicts-v5.tsv`) and write the boards, each leader with its GIF, video and caption, ranked by size (the volume, [rules](rules.md#the-judge-mctas-door-verify-design---extra-offsets-28)). `--archive-dir` also archives every working design; `--cached-only` judges no new design (no cached verdict = left out); working designs without a volume in `archive/properties-v1.tsv` are measured (one recorded cycle each) and cached either way |
| `door catalog [--videos DIR]` | write the archive catalog `archive/README.md` (every archived design: size (volume), blocks placed, boards, ticks, rotations, buildability, video, how it works) from the verdict and properties caches; records nothing |

## Building designs

| command | when to use it |
|---|---|
| `door gen <name> --out <file>` | print a built-in design (`designs*.rs`, `lm.rs`, `observer_seq.rs`) in the text format; `@name` also loads a built-in anywhere a design is expected |
| `door patch --out <file> <design> <script>` | apply an edit script: `set x y z <state>` (an extended piston gets its head), `clear x y z`, `port <name> x y z` |
| `door turn <design> --out <file> [--turns N] [--mirror]` | write a design turned and/or mirrored by the game's own block-state rotate/mirror; a quarter turn gives an east/west-facing door (`door-axis: z`): the way to build a variant for that direction |
| `door route <design> --start='x,y,z;..' --goal='x,y,z' [--allow=..] [--swept-design <file>]` | dust auto-router: fewest new blocks, avoids cells the machine sweeps, the walkway, the 3x3 columns in front of and behind the door, and touching other components; `--swept-design` takes the swept cells from the working design a net was cut from |
| `door route2 <design> --out <file>` | port-aware, timing-aware net router: rips up groups of up to `--max-rip` control blocks and re-routes every receiver to get the same signal tick for tick with fewer blocks (dust, repeaters, observers, conductors); `--need` routes a new net |
| `door place <design> --groups <file> --out <file>` | try every combination of alternative patch groups: wiring a mechanism whose part placements are a 3D puzzle |
| `door mech-synth <base> [--add k] [--pulses-only] [--verify N --out-dir DIR]` | mechanism synthesis in a macro piston model (a port of `PistonStructureResolver`): pistons, slime and blocks around the door, cheapest open and close paths, one symmetric event sequence for both lever edges; writes ideal-input designs and `door ideal` schedules and replays the best. Catalogs: `tracks/mech-synth/NOTES.md` |
| `door mech-wire <schedule>...` | wire a mechanism from its ideal-input schedule: route every piston cell's wave from a lever at each lever spot, rank by total blocks, judge the best (an upper bound: ~12 blocks worse than hand wiring) |
| `door topology <families CATALOG.tsv> [--only 7x8,8x7] [--verbose]`, `door mech-synth <base> --just-base --topology x0,y0:x1,y1` | can a mechanism be wired inside a volume box with FULL seamless at all? (topology.rs, track box56): the free cells split into wiring components; a piston links two components when one feeds it and the other watches it, carrying one pulse per change of the watched cell (pulse mergers); reports WIRABLE / NOT per mechanism under strict and permissive parity (accepts e89, h44, flat-b-v7; rejects all 99 7x8/8x7 catalog entries under strict parity) |
| `door connect <families CATALOG.tsv> --boxes 7x9,7x8 [--only ids] [--cap N] [--verbose]` | connectivity + ORDER check (connect.rs, track wire64b), parity ignored: every placement of each entry in each box; regions of free cells (merged through unswept ring stones); a piston is fed from a region next to it or its quasi cell, directly, through a ring stone or through a door block parked there (conduction); a region wakes when it can watch a change; every event order (pulses, levels on pistons extended at rest) is searched with only the lever's region awake at each flip. A SEALED free cell (a region on its own with nothing beside or below it that stays put) holds an observer only: it feeds the cell opposite the change it watched, and reads no powered block. The lever is tried in every region and also OUTSIDE the box (the volume excludes it): on the back face of a piston that never moves (it holds that piston and, by quasi-connectivity, the pistons whose above cell touches the lever: e88's top trio) and on the outer face of a static block (track d67). NOT wirable = no order works (exhaustive), UNDECIDED = the state cap was hit. Accepts e89 and flat43-full in their 8x9 boxes, rejects both in 7x9; accepts the 7x8 e78s-56b-43 and the t7b family in their own boxes |

## Searching for fewer blocks or ticks

All candidates are screened at a few positions and accepted only by the full judge. The searches
(and the routers) keep the 3x3 columns in front of and behind the door clear. Every search takes
`--shape <preset or key<=n,...>` (only designs within those shape limits; the judge also checks the
swept shape) and `--tile <spec>` (judge each candidate as a tile module); `minimize`, `regrow`,
`grow` and `harvest` take `--rotation-safe` (accept only designs that also work turned). `minimize`,
`regrow`, `grow`, `harvest` and `resynth` take `--repeater-input` (accept only designs that also work
with repeater input, the Squid regulation's 3.1.3, at every judge position) and `--eligible` (that
plus the hallway check, footsteps for sculk designs, QUART SEAMLESS or better and the lever's place).

| command | when to use it |
|---|---|
| `door minimize <design> --out <file> --moves all [--plateau] [--depth 2] [--archive]` | local search (`search.rs`): removals of 1-3 blocks, repeater delays, moves, substitutions, swaps (2-3 blocks out, 1 in), dust re-routing and net re-synthesis; `--plateau` walks neutral edits; `--objective open\|close\|total\|volume` optimizes speed or the community volume (`door properties` volume; `--slack N` lets blocks grow); `--max-open/--max-close` keep a speed; `--to-sculk` lets it turn a solid design into a sculk one |
| `door harvest --out-dir DIR --moves all --plateau [--every 20]` | run that search on the best working designs of every branch and board not harvested yet |
| `door transplant` | learn edits from pairs of designs (a, then a smaller variant of a) and try each on the best designs of every board |
| `door grow <design> --out <file> --lo --hi --states [--k 2]` | add 1 or 2 blocks in a box to a design (usually one whose old wiring was cut), screening every combination; `--states` takes shorthands like `leaves` (persistent oak leaves) and `log` (3 axes); `--then-search --glue` seeds riders |
| `door regrow <design> --out <file> --size N` | cut every face-connected group of N control blocks and re-grow it with N-1 blocks; `--any` cuts any 2 blocks |
| `door synth <context> --lo --hi --from --to [--k K] [--palette ..] [--spec <script>]` | circuit synthesis by exhaustive simulation: every circuit of up to k blocks between two cell sets, judged by the door judge or a behavior script |
| `door resynth <design> --out <file> [--max-size 4] [--palette ..]` | large-neighborhood search by exact synthesis: clears every connected group of 2..max-size wiring blocks and tries every smaller circuit in its place (gravity 45 -> 42, flat 49 -> 43) |

## Correctness

| command | when to use it |
|---|---|
| `door material <designs> --out DIR [--extra-offsets 28] [--judge] [--legacy-walkway]` | OBSIDIAN ONLY WHERE NEEDED (vjeux 2026-10-21; `material.rs`, track clk-rt): every obsidian block tried as stone on the redsim door screen at the record judge's 32 positions x 3 cycles (all at once, else one at a time, then the passing ones together); a swap stays only when the door passes with the same open and close ticks. What stays obsidian is needed as an immovable block (doors have no explosions). The changed designs (header kept, plus a material line) and material.tsv go to DIR; `--judge` confirms them with the real judge at the same positions AND its rotation check (the design built turned 90/180/270 degrees, as the leaderboard judges): a stone design that breaks a turned build its original passes keeps its obsidian; `--legacy-walkway` for designs that predate the walkway rule. `door verify --archive` refuses a door whose obsidian can be stone (redsim screen, then the game's rotation check, only for such a door). 2026-10-21: 185 of the 399 archived doors with obsidian changed (198 blocks; the same ticks, rotations, category and size); 10 more passed the screen but broke at 180 degrees with stone and keep their obsidian (bridge-p2-sculk-r-49b, e78st-56b-41-2s-41b, e78st38-s2-fill-39b-*) |
| `door selftest [--quick] [--only a,b] [--list] [--jobs 6] [--jvm-jobs 2] [--keep]` | the known-answer suite every tool must pass before a push (AGENTS.md's push gate): the redsim judge and `door verify` on the archived anchors (c79, e88-t7b, t7b-nb2, s27-bl52, m79s-sculk) and on a broken c79 (must fail), the periodic judge on c79 + a once-per-cycle copper bulb, pockets re-finding c79's bottom-left and c88 s25's gapped bottom-right chain (and not without `--gaps`), satwire `--from-wired --check` = 0 mismatches on c79 and `--update-order` rejecting a saved 48-block BUD candidate, boxproof on c79 (SAT with `--split-rounds`, UNSAT without, a freed cell re-found, the model vs redsim fuzz), `door ideal-rs` vs `door ideal` on 3 plans (one with a 1 gt pulse), generator membership of docs/audit.md's acceptance list. One PASS / FAIL line per check; `--quick` ~40 s, full ~2 min (selftest.rs; fixtures in `designs/piston-door-3x3/selftest/`) |
| `door buildable <design>... [--positions 32] [--lever off\|on] [--out-dir DIR]` | can a survival player build it? searches for a placement order (real clicks on visible faces, the world updating after each, temporary blocks allowed) that ends exactly in the judge's rest state, then runs the judge's cycles from there; writes `*.build.txt` scripts (`tracks/buildable/scripts/`, read by the catalog) |
| `door vanilla-check <design>... [--extra-offsets 28] [--trace]` | run designs in an UNMODIFIED vanilla 26.3 server (official jar, RCON, a bot player clicks the lever, region files read back) and compare every tick with the simulator under the judge's rules, plus one real-time cycle. PASS = identical tick for tick; DIVERGES names the first differing tick and cells. Method: `tracks/vanilla-check/NOTES.md` |
| `door vanilla-build <script.build.txt>... [--real-time] [--slow N]` | replay a `door buildable` script on the unmodified server with a bot player (item in hand, the script's look, sneaking, a right-click on the given face), compare after every action, then run the judge's cycles with the bot. Method: `tracks/vanilla-build/NOTES.md` |
| `door vanilla-gadget designs/gadgets/<g>.drive...` | replay gadget drive scripts on the unmodified server and compare every tick ([mechanics.md](mechanics.md#11-vanilla-confirmation)) |
| `door vanilla-fuzz [--count 50]` | differential fuzzing: random small contraptions compared between the simulator and the official server |
| `door pistfuzz [--count N] [--mixed] [--shapes] [--slime] [--observers] [--gravity] [--gadgets] [--designs <files>] [--replay <files>] [--shrink] [--trace]` | differential fuzzing of `crates/redsim` (the exact Rust model: pistons, moving blocks, redstone) against the bridge: random piston setups (7x7x1 flat / 6x6x3) with scripted power (`--mixed`: redstone components next to the pistons; `--shapes`: slabs, trapdoors, scaffolding, fence gates), the gadget library's drive scripts, archived doors; every block state, moving block entity, queue size, drop and the idle flag compared after every tick; failures shrunk and saved as replayable scenarios ([tracks/pistmodel](../designs/piston-door-3x3/tracks/pistmodel/NOTES.md)) |
| `door mechmut <seeds> [--repair] [--add] [--validate] [--rank-unfed] [--max-actions K] [--no-inside] [--reserve X,Y;...] [--keep PIECES] [--reserve-wiring] [--macrosat-qc]` | local search from known mechanisms (`mechmut.rs`): each neighbour (drop / shift / turn a piston, toggle sticky / extended; `--add`: one more piece) solved in families' macro model (the parent's schedule replayed first), kept with a FULL frame, `--validate` replays it in redsim; ranked by redsim, extent (w86's `ideal-rs --extent`), unfed receivers, island start, d67's `feeds --strict`, feeds, pocket load, actions per piston. Seeds: lines `W H x0 y0 pieces`, or `door macrosat` dumps (a file or a folder: the rest grid as the seed, its actions as the first schedule, solved without QC coupling unless `--macrosat-qc`; a dump's own `export/c-<dump>.txt/.ideal` pair is judged directly: redsim, strict feeds, extent, and flagged CANDIDATE when all pass) |
| `door fullmech [--check <designs>] [--boxes 7x8,8x8] [--pulses 1,3] [--levels] [--pairs] [--groups] [--reduce] [--shrink] [--export DIR] [--verify-ideal <plans>]` | mechanism search on the exact model: every transition is the real tick-level game (spitting, 0-ticks, the block-event order, pistons carried by pistons keep their input, carried redstone blocks / observers); ideal inputs are physical (a redstone block behind the piston: QC powers the piston below); FULL seamless open + close back to rest. `--boxes` re-solves `door families`' candidates; `--export` writes each solved mechanism (design + exact `door ideal` plan); `--groups` merges inputs into the fewest independent signals (+ `-groups.ideal` plans); `--reduce` tries helpers (redstone block / observer) that remove inputs or signals; `--shrink` drops a piston and repairs with one helper; `--verify-ideal` replays plans in the real simulator; `--anneal` samples mechanisms outside the generator |
| `door eligible <design>... [--extra-offsets 28]` | record eligibility under the Squid regulation (section 3), one line per design: the judge + rotations, repeater input at every judge position (props-dyn's rig: the lever left off, a delay-1 repeater into its support block, both edges), the hallway check, footsteps (sculk), the SEAMLESS tier, the community volume, the lever's place; ELIGIBLE or what fails ([tracks/eligible](../designs/piston-door-3x3/tracks/eligible/NOTES.md)) |
| `door relever <design>... [--subs note,dust] [--out-dir DIR] [--why]` | make working doors repeater-input eligible: the lever's cell becomes a note block (the lever hung on it, or kept on its old support) or dust (the lever kept on its support); every variant judged like a record and checked with `--eligible`; table with ticks, seamless tier and volume |
| `door audit [designs or folders...] [--sections checks,mech,wiring,rules] [--jvm] [--out F] [--tsv MECH.tsv] [--explain]` | what our rules, checks, generators and wiring tools silently exclude, found on WORKING doors ([audit.md](audit.md)): every check on each design, its mechanism extracted from a redsim cycle and tested against families' own generator under each bound setting (the exact bound when it is out), connect / topology in its own box, what satwire / cosearch / boxproof cannot hold, every hard-coded rule with live file:line; `--explain` prints one door's paths and every move with the piston that made it |

## Videos and showcase (coordinator only: the X displays are shared)

| command | when to use it |
|---|---|
| `door video <design> --out <f.mp4> [--redo] [--relabel]` | the door opening and closing in the real Minecraft client: title card, then the design's one clip (still door camera; each flip plays until the door has settled) with the corner label (the size `80b` big, the blocks placed small). The clip is recorded only when missing or stale ([internals](internals.md#videos)); `--relabel` never records |
| `door video-all --out DIR [--upload videos] [--relabel] [--dry-run]` | every archived design's video cut from its clip: clips missing or stale are recorded first, a video is cut again (seconds, no Minecraft) when its clip, card or label changed, uploaded to the `videos` release, then the archive catalog; `--relabel` records nothing and cuts every video again; `--dry-run` lists the work |
| `door tick-video <design> --out <f.mp4>` | one open/close cycle tick by tick: still camera, captions for the flips and the settled states, a 3x3 map of the doorway per tick and a strip of the whole phase |
| `door card <design> --out <png>` | draw just a video's title card (and its corner label) |
| `door gallery --videos DIR [--upload videos] [--gif-only]` | a looping GIF of the best design of every board cut from its clip (the same clips as the videos; a leader without one is recorded first, `--gif-only` records nothing), uploaded to the release (LEADERBOARD.md shows them), plus the night's timeline chart |
| `door montage --videos DIR --out <f.mp4> [--board solid] [--upload videos]` | a board's record progression by blocks placed (the history before sizes were volumes) as one video: every record filmed by the same camera, labeled with its blocks placed and how it got there (solid: `archive/records.tsv`) |
| `door world --out DIR` | one Minecraft world (a save folder for the real game) with every working archived design in rows by board, smallest volume first, its size in red concrete above it, to try them by hand |

## Generated files

Never edit these by hand; change their generator or its inputs and regenerate:

| file | written by | inputs |
|---|---|---|
| `designs/piston-door-3x3/LEADERBOARD.md` | `door leaderboard` | every design on main and `explore/*`, the verdict and shape caches, the `videos` release (GIFs, videos, montages), `archive/captions.tsv` |
| `designs/piston-door-3x3/archive/README.md` | `door catalog`, `door video-all` | the archive files, the verdict cache, `archive/records.tsv`, `archive/captions.tsv`, `tracks/buildable/scripts/`, the `videos` release |
| the `gh-pages` branch (REDSTONEPEDIA, https://vjeux.github.io/redstonepedia/) | `site publish` | every archive and its caches, captions, traces, `web/viewer/`, `web/site/` ([site.md](site.md)) |

`archive/records.tsv` (the solid record progression, one row per record) and
`archive/captions.tsv` (one line per design: how it works) are hand-edited data: add a row when
a design takes a record or tops a board.

## Design files

Contraptions are human-readable layers (`crates/mctas/src/design.rs`):

```text
# comment
name: tiny-demo
origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
port lever: 2 1 0              # named positions (inputs, outputs, ...)
palette:
  S minecraft:stone
  P minecraft:sticky_piston[facing=up]
  L minecraft:lever[face=floor,facing=east]
layer 0:
  S.
layer 1:
  PL
```

Rows run north to south (z grows downward), columns west to east (x grows rightward). `.` and
spaces are air; every other character must be in the palette. A door design names the ports
`door` and `lever` ([rules.md](rules.md#the-door)); `@name` loads a built-in. Entity doors add
`entity: <type> x y z [snbt]` lines (category entity, [rules.md](rules.md#categories)); tile
modules add a `relay:` block. Block-entity data (container contents of item clocks) are
`nbt: x y z {snbt}` lines (world coordinates like `port:`), merged into the block entity right
after the raw paste exactly like `data merge block x y z {snbt}`, e.g.
`nbt: 0 1 0 {Items:[{Slot:0b,id:"minecraft:stone",count:1}]}`.

The search palettes (`synth`, `resynth`, `grow`, `regrow`) take shorthands: `leaves`, `log`, and
the exotic components (`trapdoor`, `ctrap`, `gate`, `skull`, `shelf`, `rail`, `arail`, `disp`,
`dropper`, `crafter`, `scaffold`, `grate`, `tinted`, `slab`, `stairs`, or `exotic` for all of them;
the exotic track's gadgets prove each). `regrow --require-any` keeps only candidates that use one
of the given blocks.
