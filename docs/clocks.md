# Clocks: the clock study

vjeux (2026-10-19): "Finding as many clock types for each tick count (or family of ticks). Build
the scaffolding for this and let agents find all sorts of clocks." This page is the one home for
what a clock is, how it is judged, the design format and the commands. The brief and the tracks:
[CLOCKS.md](../designs/clocks/CLOCKS.md) (copied from the coordinator's brief); the game mechanics
the clocks use: [mechanics.md](mechanics.md); the simulator: [internals.md](internals.md).

## What counts as a clock

* A structure that, once placed (and optionally after ONE start kick at a declared `start` cell),
  keeps producing a periodic signal forever with no further input. Self-starting and kicked clocks
  both count; the start is part of the record (`self-start`, `kick: ...`).
* **Period P** (game ticks, gt): the smallest P such that every cell's full state (its block
  state, a comparator's output strength, a container's contents and a hopper's cooldown) repeats
  exactly every P gt after a warmup W. The rest of the hidden state must repeat too: on redsim
  the WHOLE state is keyed exactly (`redsim::snapshot`: every cell, the pending ticks with their
  delays, priorities and order, block events, moving blocks, every block entity, the torch memory
  younger than 60 gt) and the first exact repeat gives the whole-state warmup and period (a
  multiple of P: the order of independent ticks due together can permute), which `clock sim`
  prints apart. The real game shows block states every tick and the block entities' data, so the
  judge asks the same full cell states to repeat with period P for at least 3 periods after W.
* **Output** (vjeux / coordinator, 2026-10-20: "would a builder actually use this?"): an exterior
  AIR cell where the judge places a redstone lamp (`sim::JUDGE_LAMP`, in the game and in redsim; not
  a design block: never counted, never in the family or the box) and reads the POWER delivered into
  it, what a lamp, dust or repeater a builder attaches there gets (the lamp's own 4 gt off-delay
  is the lamp's limit, not the clock's). The cell must stay free all cycle (no block moved into or
  through it, moving pistons included, no entity in it), sit on the swept box's boundary or one
  cell outside it with a face to free space, and not above tripwire, rails or water
  (`sim::placement_problem`): else INVALID. A design with no `port output` gets the best such cell
  (`sim::lamp_outputs`: evenly spaced pulses at the clock's own period first, then the longest
  on-time, the lowest, the fewest changing neighbours, a side over a top over a bottom face);
  `clock sim --exterior` prints it. A clock with none is not a usable clock. (Before 2026-10-20 a
  declared output could be a component read by its own state, `mctas clock misc declare-output`
  declared the auto-picked cell, and `clock sim` / `clock verify` could read an undeclared cell
  differently; the archive moved to exterior outputs with `clock retap`.)
* **Composite** (coordinator's ruling, 2026-10-19): when the output repeats faster than the whole
  structure AND the changing cells split into independent groups (cells linked when at most 2
  apart, or within a sculk sensor's range), the design is independent sub-clocks side by side
  (their period would be the lcm): `COMPOSITE`, rejected by `clock sim`, `clock verify`, `clock
  archive` and the census. One mechanism whose output fires twice per cycle (an observer watching
  both strokes of a piston) is a normal clock of the whole cycle. Beat clocks (the output combines
  sub-clocks that run alone) pass and carry `# composite` in their design: the catalog lists them
  apart. Dividers (flip-flop stages on a base clock) are normal clocks.
* **Waveform**: per changing cell, on-ticks per period (duty), rising edges per period and pulse
  widths; the same at the output.
  "On" ([`wave::active`](../crates/clock/src/wave.rs)): pistons extended, a piston head or moving
  block present, sculk sensors ACTIVE, hoppers locked, doors/trapdoors/gates open, dispensers,
  droppers and crafters triggered, leaves distance < 7, scaffolding distance > 0, else `lit`, then
  `power > 0`, then `powered`; a plain block: present. An output cell holding a plain block or air
  reads the power it receives (`getBestNeighborSignal > 0`).
* **Evenly spaced output** (coordinator's decision, 2026-10-20, from vjeux's site critique C38: a
  P476 minecart loop whose output pulses every 52..71 gt is no 24 s clock): the judge and redsim
  also record the GAPS, the ticks between the output's rising edges over one period
  (`CellWave::gaps`; verdicts-v1.tsv's 17th column, `[6, 6]`). With N equal gaps g the output is a
  clock of period g for a builder (g can be shorter than the state period); unequal gaps are an
  UNEVEN output: `clock archive` refuses it, `clock catalog` and the site leave such rows out (they
  stay in the repository). `clock gaps [--write]` backfills the column from redsim; `clock sim
  --irregular` lists the uneven ones. On 2026-10-20: 2544 of 4051 archived clocks were uneven.
* **A survival clock, indefinitely** (2026-10-20): verdicts-v1.tsv's 18th column `hidden` gives
  why a clock stays off the catalog and the site (empty: shown; `Row::shown()`); only judge
  failures in normal running hide: `no output` (no exterior cell that pulses evenly) and
  `command-only` (entity data a player cannot make: an item with Age -32768 or PickupDelay 32767,
  a minecart started with a Motion, NoGravity, Invulnerable). The 19th column `reload` states what
  a chunk reload mid-run does (`clock verify --reload`: the bridge saves, unloads and loads the
  sandbox's chunks after the warmup and 2 periods, without a tick): `ok`, `glitch then resumes`
  (at most 2 periods + 40 gt, then the same state period), `stops`, `changes period to P gt`, or
  `fails: why`, with `in K of N builds` when only some builds do it; a property the page states,
  never a reason to hide. Vanilla's own cause of most `stops`: PistonMovingBlockEntity saves its
  progressO as "progress", so a piston caught mid-move lands a tick late after loading (traced with
  `clock misc trace-export --reload-at T`). redsim judges the reload too (tickexact 2026-11-08,
  `redsim::reload`, which models the save, unload and load exactly: the scheduled ticks numbered
  again per chunk and run in the level's container order, the block entities registered again,
  that progressO): `clock verify --reload` runs every build's reload in redsim beside the game's
  run and prints `reload: redsim AGREES` (the game confirms redsim's verdict) or `DISAGREES` (a
  failure: a simulator gap). Other properties a page states instead of hiding (vjeux:
  no invented rules): the rotations and positions a clock works in (`robustness`), tripwire (any
  passing entity fires it), a daylight detector. Item clocks run past their items' 6000 gt
  lifetime (12000 gt, redsim and the game). The game horizon scales with the period (warmup + 4
  state periods, at most 100 000 gt: a P6926 jukebox carousel is still for 6920 gt during each
  song). `clock retap [--shard i/n] --results FILE` gives each archived clock its best exterior
  output (redsim ranks, the game confirms) or a reason to hide it; `--apply FILE...` rewrites
  designs and rows; `--reload-pass` writes the `reload` property of the shown ones; `clock hidden
  [--write] [--unhide REASON...]` counts and lists the hidden ones.
* **A usable output** (vjeux 2026-10-21 on clk-rt-ring3x3-p10-r-12b, "unusable for a redstone
  circuit"; the coordinator's rule, reading D, widened to the standard taps;
  [`usable`](../crates/clock/src/usable.rs)): where a builder takes the signal into a circuit,
  with a TAP in the tap cell: a REPEATER reading the signal there (the tap cell is the `port
  output`), a COMPARATOR reading a container or block of the build behind it, or an OBSERVER
  watching a block of the build that changes every cycle (these two join the design, a stone
  under a comparator, and the port is their output, the next cell outward). The tap cell: an air
  cell free all cycle the tap stands on (the build's ground level, or above it on a sturdy block
  of the build or on a block put under it), on the exterior (its input side inside the swept box,
  the way out horizontal and leaving it), the line outward free all cycle with nothing of the
  build next to it (a line leads away), not next to a moving part (a block that comes and goes, a
  piston head or moving block, an entity's cell). The TAP RUN: the tap in its cell (input toward
  the build), stone under it and under the line where the build has none, the judge's lamp in the
  next cell: the clock keeps its period, the lamp rises at evenly spaced ticks every g ticks, g
  the clock's own pulse spacing (its archived output's gaps: a move never changes a clock's time),
  and the tap's own signal into the lamp's cell rises just as often (every pulse: the lamp's
  4-tick fall merges none). A repeater tap also passes the judge's lamp test in its own cell.
  verdicts-v1.tsv's 20th column `usable`: `yes: repeater|comparator|observer`, or `no: <why>` (a
  plain warning on the site, never a reason to hide; the site sorts those after the usable ones
  within their period); empty: not checked (`clock archive` fills it from redsim). `clock retap
  --usable [--write]` judges every clock on redsim and moves a shown clock's unusable output to
  the best usable tap (the lowest level first, then the nearest the build's middle, then the
  evenest pulse: equal widths, a duty nearest half; then repeater, comparator, observer); a usable
  output stays, a hidden clock is judged but never moved; `--usable --game [--write]` judges each
  period's smallest shown clock (and the files given) in the game, the clock as archived and a
  repeater's tap run at its port, and writes the game's word (a clock redsim finds no tap for
  keeps its row's reason). On 2026-10-21 (redsim, 2310 shown): 287 usable already, 1367 moved
  (957 repeater, 116 comparator, 294 observer), 656 with no usable tap; the game pass over each
  period's smallest shown clock and the 18 group picks (223 clocks): 181 usable in the game, the
  42 others redsim's no-tap clocks, 0 disagreements. `--usable-strict` (the ground level
  only) compares the first reading.
* **Family**: the set of block types whose own state changes IN PLACE during one period (the same
  block in the cell before and after: lit, powered, power, open, extended, phase, a comparator's
  strength, a container's contents), plus a redstone block that moves (a moving power source),
  detected automatically. A block that only moves (a lamp a piston pushes, never lit) is listed
  apart (`moved only` in `clock sim`); plain blocks never count; piston heads count as their
  piston, wall torches as torches, every waxed copper bulb as `copper_bulb`. Written sorted and
  joined by `+`: `observer`, `comparator+redstone_wire`, `comparator+hopper`,
  `redstone_block+sticky_piston`, `redstone_torch+redstone_wire+repeater`.
* **Type**: the structure (block states with their facings, container contents) up to
  translation, the 4 horizontal rotations and the mirror image (a mirror pair is one type, noted
  `chiral`), and up to the swaps that leave the run identical (vjeux 2026-10-19, `clock::ident`):
  sticky pistons count as pistons when they never pull a block, any inert full solid (stone,
  wool, planks, metal blocks, obsidian...) as stone when its kind changes nothing (both checked
  per design on redsim: every block state, comparator strength and container content at every
  tick of warmup + 2 periods must match modulo the swap). `clock archive --dedupe` removes such
  twins (aliases.tsv: "built with sticky_piston, without piston"). The keys are cached in
  `designs/clocks/archive/type-keys.tsv` (not committed).
* **Dominance fold** (the gallery only, vjeux 2026-10-19): within a period, a clock is a variant of
  a clock with strictly fewer blocks whose block kinds are a subset of its own (the observer pair,
  2 blocks, hosts the observer + note block clock, 3); the galleries show only the hosts, and the
  variants are listed apart on `gallery/variants.md` (per period, per host a table: variant,
  blocks, the kinds it adds), linked once from the index; coverage.md counts types shown and
  variants (variants are left out of the matrix). The archive keeps them.
* **Size**: blocks placed (piston heads are part of their piston; a temporary start block is not
  counted) and the SWEPT box: every cell non-air at any tick of the run, plus the design's box and
  the start cell (a piston head poking out of the design counts). Ranking inside a period: fewest
  blocks, then smallest swept volume, then the waveform.
* **Must work in the real game** (`mctas clock verify`, the official judge): exactly periodic at N
  positions x 4 rotations, no entities created (a drop, a popped block), never reaching the edge of
  the watched box (its box + `--margin`, default 3). Builds that disagree on period, output waveform
  or family make the clock `position-dependent` or `rotation-dependent`: recorded, flagged.
* The study's product: for every period P, every family that reaches it, its distinct types and the
  smallest of each, and which periods a family cannot reach (with the reason, or an exhaustive
  census as the proof): `designs/clocks/CATALOG.md` (generated) and the board's
  `minecraft-tas/clocks/coverage.md`.

## Design format

The door design text format ([tools.md](tools.md#design-files), `crates/mctas/src/design.rs`), with
no door and no lever:

```text
name: cal-observer-pair
port output: 0 0 0          # optional: where the signal is read
port start: 1 0 0 last      # optional: the one-time start kick
palette:
  E minecraft:observer[facing=east]
  W minecraft:observer[facing=west]
layer 0:
  EW
```

* No `port start` = **self-starting**: placed exactly like the door judge places a door (raw writes
  in file order, a shape update of every wire, a `self` update of every block; no observer pulse and
  no piston fire from the placement itself), then run with nothing else.
* `port start: x y z <kick> [at N]`, the kick (between ticks, at tick 0 = right after placement and
  before the first tick, or after N ticks with `at N`):
  * `last`: the design's own block in that cell is left out of the silent placement and placed with
    updates (a player placing it). The two-observer clock starts this way.
  * `use [T]`: one right-click on the design block there (lever, button, note block, repeater,
    comparator, dust, oak trapdoor/door); with `T`, a second right-click T gt later (a lever pulse).
  * `<block state> <T>`: a temporary block placed with updates in an AIR cell and removed with
    updates (no drop) T gt later: `minecraft:redstone_block 1` is a 1 gt power pulse,
    `minecraft:stone 1` a shape update for an observer.
  * Parity: every redstone component delays by an even number of gt, so a clock of torches,
    repeaters and comparators started on one parity class keeps it; `at N` with an odd N starts a
    second class (an odd period needs one).
* `placement: player` (header): every block placed with updates in file order instead (flagged in
  the start label).
* `nbt: x y z {snbt}` (header, track clk-item): a block entity's data merged right after the raw
  paste, before the wire shapes and self updates (`data merge block`): container contents
  (`{Items:[{Slot:0b,id:"minecraft:stone",count:1}]}`), a hopper's `TransferCooldown`. redsim models
  Items of its item table and TransferCooldown; anything else is UNMODELED for `clock sim`.
* Extra words on any `port` line are kept: `Design.meta["port <name>"]`, written back on the port
  line.
* `entity: <type> <x> <y> <z> [{snbt}]` (header, the door format's; track clk-misc: minecart
  clocks): a design entity, summoned once the blocks stand (before the kick), turned with the
  design (position like the cells, `Rotation` by the game, `Motion` by the judge). `clock verify`
  then fails on any entity that is NOT the design's (a drop, a popped block) and on a design
  entity lost, duplicated or outside the watched box at the end; the period stays the block
  states' (a detector rail, a comparator reading a hopper minecart). Each entity counts as one
  piece in `blocks`, its kind joins the family (`detector_rail+minecart`) and its cell joins the
  type key. `clock sim` runs them on redsim (boats, minecarts, water: redsim's fluids + entities
  port, 2026-10-19) as far as their blocks and kinds are modeled (else UNMODELED with what is
  missing); an entity's exact doubles never repeat, so for designs with entities the period comes
  from the blocks by the real-game judge's own rule (every cell's state repeating for 3 periods
  within 4000 gt, the smallest warmup then period, and the period holding over at least the
  second half of the horizon: a drifting entity can show a short block pattern near the end of
  any window, 2026-10-20): every traced entity clock (the exact traces
  in `designs/clocks/traces/exact`, all tick-identical under `redsim-fuzz trace-compare`) gives
  the same CLOCK line as the game. Example: a minecart on a 3x3
  rail loop with two powered rails on redstone blocks over a detector rail is a robust P476 clock
  (warmup 556; the detector's 20 gt re-check against the lap time).

### Extendable clocks (`extend` headers, vjeux 2026-10-19)

A family of clocks that grow by a repeated step ("add 2 observers and make the tower bigger") is
ONE base design with these headers:

```text
extend family: clk-rt-fade          # the family's id
extend period: 114 + 56n            # P(n) in gt: linear a + b·n, or geometric a·k^n (+ c): 36 * 2^n
extend blocks: 18 + 4n              # blocks(n), the same forms
extend rule: lengthen both rails by one repeater and one dust each   # how to extend, one line
extend n: 0..12                     # the n range judged in the game
extend base: 0                      # the n of this design (default: the low end)
extend generator: mctas clock rt family fade --n {n} --out {out}     # builds any n
```

The generator is one command (split on spaces, no shell; `{n}` and `{out}` are replaced, `mctas`
is the running executable). `mctas clock extend-check <design> [--n a..b | a,b,c] [--record]`
builds and judges instances with `clock verify` (period and blocks must equal the formulas);
`clock archive` of a family design judges n = low, low+1, low+2, the middle and the high end
first, refuses the family when one fails, and records the instances in
`designs/clocks/archive/extend.tsv` (family, n, period, blocks, type key, verdict). A family's
base is archived even when its instance is already archived (the family card is another entry).
`clock catalog` shows one card per family in an "Extendable clocks" section (the period formula as
the heading, the two smallest members pictured, P in gt and in seconds, blocks, the rule, the n
range and the judged n) and leaves the recorded instances out of the per-period gallery, with a
line "Also reachable by: <family> (n = k)" under every period a family reaches.

The periods players ask for (1 s, 2 s, 5 s, 10 s, 30 s, 1 min, 2 min, 5 min, 10 min): `mctas clock
obs at-periods [--periods ...] [--sim] [--verify K --record]` lists every archived family whose
formula gives exactly such a period at an integer n (inside or past its range), builds the member
with the family's generator, moves its output to an exterior air cell where the judge's lamp lights
evenly (the generators predate that rule), runs it on redsim, judges the K smallest per period in
the game and records them in `extend.tsv`. The website lists every n `extend.tsv` records as a
member of its family even past the family's range, so a time search (`1 min`) finds it (2026-10-21:
30 s, 1 min, 2 min and 5 min members judged; 10 min (12000 gt) needs a horizon past the judge's
20000 gt for 3 periods).

Usable and sealed builds (clk-obs, 2026-10-21): `mctas clock obs tap <files> [--list F] [--seal]
[--gap auto|period|row|any|N] [--threads N]` keeps a clock's declared output when it is usable
(clock::usable::judge) and otherwise moves it to the best usable tap (`usable::pick`, hidden clocks
too), with `--seal` puts glass on every open hopper top (clock::intrusion::intakes; a thin block in
the way gets glass on its free sides) and keeps the result only when redsim gives the same period;
a clock with no usable tap keeps its declared output when sealed. `--scan` lists the periods whose
smallest shown clocks all have an open hopper top, and the recorded family members of an
open-topped family that are their period's smallest. A whole family gets a sealed twin:
`mctas clock obs seal-family <bases>` writes `<family>-sealed`'s base (the same period formula and n
range, the blocks formula fitted on three sealed members, the twins of one card in their own card
`<group>-sealed`) whose generator is `mctas clock obs sealgen --family <base> --n {n} --out {out}`
(the original's member n, sealed), and `mctas clock obs seal-archive <twins>` archives each twin and
records the members its original recorded. The compact hopper clock's own sealed form is
`clk-obs-hopclk-sealed` (`clock obs hopclk --sealed --base`).

Families of one mechanism split into several cards (one per residue, per relay block, per item
count) share ONE card (vjeux: "can we not have 300 variations of this one?") when their base designs
carry the same group:

```text
extend group: clk-item-dropring                  # the group's id, on every member
extend group title: dropper ring                 # optional: the card's name
extend group period: 20 + 14 (W - 3) + 7 (depth - 3) + 7 [one dropper] + 2 Σ delays   # optional: the general formula (the heading)
extend group note: W = ring width, depth = 3 + its repeaters   # optional: one line under the name
```

(title, period and note are read from the first member carrying them; set them with `mctas clock
group --id <id> [--title ...] [--period ...] [--note ...] <designs>...`, `--clear` removes them).
The group's card pictures the simplest member (fewest blocks at its low n, then volume, then
archived first), takes the general formula as its heading (else the formula all members share,
else `a + b·n gt, a in {...}`, else each formula), lists the blocks that work in the relay place
when every member differs from the others by one block kind ("Works with any one of these 10
blocks in place of the note block: ..."), the periods the members reach together ("every period
from 40 gt to 460 gt"), and every member in a collapsed table (formula, blocks, n range, judged n,
design link), with no picture rows. Families without a group header that share a period formula
and differ by one block kind only (a relay block, a calibrated or plain sensor) share a card too.

Put your designs in `designs/clocks/tracks/<track>/`; the coordinator archives verified clocks in
`designs/clocks/archive/` with a caption line in `captions.tsv`.

## Commands

| command | what |
|---|---|
| `mctas clock sim <design>... [--horizon 50000] [--margin 4] [--json] [-q] [--trace N]` | the exact period on redsim in milliseconds: place, kick, run until the whole state repeats (W, P), then replay one period for the waveform per changing cell and at the output, the family, the blocks and the swept box. `CLOCK:` line, or `STOPS` (it goes still, or only hidden state changes), `NO-REPEAT` (horizon), `UNMODELED` (a block or case redsim does not model exactly: judge it with `clock verify`), `INVALID` (a drop, or a write beyond the margin). `--trace N` prints every cell change of the first N ticks |
| `mctas clock verify <design>... [--positions 4] [--no-rotations] [--horizon 4000] [--margin 3] [--workers 8] [-v] [--json]` | THE judge, in the real game: every build in a cleared sandbox at the door judge's positions (4 by default, vjeux 2026-10-19) x 4 rotations = 16 builds (the game turns every block state, and a temporary start block), placed and kicked like `clock sim`, every tick of the watched box recorded (block states and block entities); the horizon grows with redsim's period (warmup + 4 periods, up to 20000 gt); exact periodicity of the full cell state for >= 3 periods after warmup, no entity but the design's own (`entity:` lines: still there, same counts, inside the box), never reaching the watched box's edge, the same period/output waveform/family everywhere. Prints `CLOCK: <name> period P gt (T s), on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust\|position-dependent\|rotation-dependent`, the builds that differ or fail, and the redsim agreement (`redsim AGREES (period, warmup, output waveform, family, box)` or what redsim says); exits 1 unless it is a clock |
| `mctas clock shapes <design or dir>... [--write]` | the designs whose neighbour-shaped blocks (a fence's arms, a fence gate's `in_wall`, a tripwire's connections; walls, panes and bars once redsim models them) are stored otherwise than a player's placement shapes them (`sim::shape_mismatches`: placed like the judge, each one's `updateFromNeighbourShapes`): each cell, stored and placed. `--write` rewrites them as placed (the old comments kept, one saying so). vjeux 2026-10-21, "why don't the fences connect?": the judge places a design as stored, so a design stores what placement gives; `clock archive` and `circuit archive` refuse the others (`catalog::refuse_unplaced_shapes`). Dust is shaped by the judge as it places it; a note block's instrument, a repeater's lock and leaves' distance are set by the first update, nothing to see |
| `mctas clock archive <design>... [--caption '...'] [--dir designs/clocks/archive] [--dedupe]` | (coordinator) the judge, then a copy in the archive (header: date, source, the judge's line), its row in `verdicts-v1.tsv` (period, warmup, output duty and edges, family, blocks, box, robustness, start, type key, chirality) and its caption in `captions.tsv` (default: the design's first comment block); a clock already archived (same blocks, ports, kick, block entities) is skipped, and a design of an already archived TYPE at the same period (a rotation, mirror image or move of it) is refused (`SAME TYPE ... same type as <file>`); a name already taken by another structure is archived as `<name>-<6 hex digits of its type hash>.txt` (the same structure gets the same name on every branch). `--dedupe` (no judging): keep one archived design per type (the earliest archived), remove the others (design, pictures, rows) and record them in `aliases.tsv` (the catalog lists them as "also found as") |
| `mctas clock census-report <census dir>... [--out F]` | the MINIMAL clocks of several census runs merged: per (period, family) the fewest blocks, its box and census, the minimal type count, the periods never found below the largest (`designs/clocks/census/README.md`) |
| `mctas clock period-search <seed>... --out DIR [--period 3] [--palette "state;state"] [--grow 2] [--iterations 200000] [--workers 32]` | annealing toward a target period on redsim ([clock::period_search](../crates/clock/src/period_search.rs) on the move set and chains of [clock::anneal](../crates/clock/src/anneal.rs), shared with `circuit compact`): chains from each seed clock inside its box grown by `--grow`, scored by the period and, among multiples of `--period`, by the share of driver cells (pistons, redstone blocks, moving blocks) already repeating every `--period` gt; designs of the target period go to DIR/<seed>/found/, DIR/summary.tsv |
| `mctas clock extend-check <design> [--n a..b \| a,b,c] [--positions 4] [--record] [--dir designs/clocks/archive]` | judges instances of an extendable family (its `extend` headers, above): builds each n with the generator, `clock verify`, period and blocks against the formulas; `OK`/`FAIL` line per n; `--record` adds the passing ones to `extend.tsv` (default n: low, low+1, low+2, middle, high) |
| `mctas clock tables [--fix [--dry-run]] [--root designs]` | the archive tables' duplicate keys and orphan rows (`clock::tables`): a union merge keeps both versions of a row one branch updated, and brings back the rows of a design one branch deleted. Without `--fix`: one `DUPLICATE` line per table with a key held twice and one `ORPHAN` line per table with rows of a design file (or, in extend.tsv, a family) that is gone, exit 1 when any (the quick selftest `archive:tables`). `--fix` (run it after every merge; `clock catalog` and `circuit catalog` run it first): one row per key, the most columns, then the newest `archived` stamp (circuit verdicts) or the row matching the design file's current hash (clock verdicts), then the row the merge changed (not in the merge base), else the last (reported as ambiguous); then every orphan row dropped |
| `mctas clock catalog` | (coordinator) from the archive's verdicts (no game): `designs/clocks/CATALOG.md` = the INDEX (a one-line intro and legend, the "Extendable clocks" cards: one per row, one per family or [group](#extendable-clocks-extend-headers-vjeux-2026-10-19), the period formula as a heading `### P = 8 + 4n gt`, the two smallest members side by side (`img/<base stem>.jpg` and `img/<base stem>.n<k>.jpg`, labelled with n, period and blocks), then the seconds and blocks formulas, the rule and the judged n range; then a table of period ranges with their clock types linking to the gallery pages, and one link to the variants page); `designs/clocks/gallery/p<lo>-<hi>.md` = the pictures by period (ascending; consecutive periods while a page stays under 120 KB and 300 pictures, a period never split; stale pages removed), per period a 3-wide grid of every distinct type's picture (linked to its design) with one caption line (`20 gt (1 s) · observer+piston · 6 blocks`, "also found as" aliases; "(picture pending)" without a picture) and "Also reachable by: <family> (n = k)"; `gallery/variants.md` the variants of the dominance fold; `gallery/composite.md` the composite (beat) clocks; `designs/clocks/coverage.md` (the period x family matrix of the types shown, mirrored on the board's `minecraft-tas/clocks/coverage.md`); pictures of designs no longer archived are pruned (a family member's `<stem>.n<k>.jpg` stays with its base). The periods a family cannot reach stay data in `designs/clocks/impossible.tsv` (family, periods, why). `clock shot` of a family base also pictures its two smallest members, built with the family's generator |
| `mctas clock group --id <id> [--title T] [--period P] [--note N] <designs>... \| --clear` | set (or clear) the `extend group` headers of family base designs (after their `extend family:` line): the catalog renders one card per group |
| `mctas clock census --box AxBxC --k N [--kmin 1] [--palette core] [--max-period 500] [--margin 2] [--no-kick] [--workers W] --out DIR` | the exhaustive census on redsim: every filling of the box with kmin..=k blocks of the palette (every facing of every component: shorthands `core` = stone, glass, rblock, dust, torch (standing + 4 wall), repeater (4 facings x 4 delays), comparator (4 x 2 modes), observer (6), piston (6), sticky (6), lamp, bulb, note, target, slime; also sculk, csculk, hopper, dropper, dispenser, trapdoor, itrapdoor, leaves, scaffold, honey, wool, obsidian, amethyst, or block states), one per class up to translation (it touches the box's low faces) and the 4 rotations (smallest encoding among the rotations that fit), only fillings with a delay element (torch, repeater, comparator, observer, piston, lamp, sculk, hopper, dropper...); judged self-starting, then with each block placed last; a found clock gets the full `clock sim` analysis and a MINIMAL flag (no single block can go with its period kept). Writes `DIR/clocks.tsv` (every clock: period, whole-state period, warmup, family, blocks, box, duty, edges, pulses, start, type key, canonical encoding, minimal), `minimal.tsv`, `designs/` (the smallest clock of each (period, family)), `summary.md`. Resumable: `DIR/parts/` holds one file per finished chunk (a cell set and its first cell's state); a rerun skips them |
| `mctas clock kit <shuttle\|bouncer\|dripleaf> [--max-len 7] [--screen 1000] [--workers W] --out DIR` | structured census of ENTITY clocks on redsim ([kits](#entity-clock-kits-redsim-clock-kit)): one mechanism's design space in its own coordinates, one candidate per mirror class, screened (`sim::entity_screen`: comes to rest, or no block change late in the run) then judged by `clock sim` (the real-game judge's rule for designs with entities); one representative per type (blocks + entity kinds; an entity at rest first, then the shortest warmup); per period and entity kind the smallest clock is shrunk to minimal (blocks removed while period and family stay). Writes `DIR/clocks.tsv` (every type), `summary.md`, `designs/` (`kit-<kit>-<entity>-p<P>-<n>b.txt`, with a caption) |
| `mctas clock misc trace-export <archived design>... [--exact] [--ticks N] [--out-dir D] [--margin 3] [--action T:use:x,y,z] [--seeds A..B] [--circuit [--stimulus S] [--judged-dir D] [--ground grass\|obsidian [--drop N] [--reach 36]]]` | real-game traces (JSON Lines, one file per clock: a header, then every tick's changed block states and every entity) for the in-browser viewer (`designs/clocks/traces/`, rounded) or, with `--exact`, for redsim's gate (`designs/clocks/traces/exact/`: the game's own doubles and floats, each entity's motion and network id, the level random's state after every frame (`rng`), the sandbox's `seed` and forced `chunks` in the header); `--ticks N` records any design for N ticks (a redsim test scene); `--action` adds an input (`T0..T1/STEP:` repeats it), `--seeds` one trace per sandbox seed. `--circuit`: a circuit as its judge builds and drives it (consumer lamps, input drivers; the kind's stimulus or `--stimulus`), for the judge's whole run and in the judge's watched box (a flying machine's corridor, ending where the judge's run ends), the judged build written as `<stem>-judged.txt` and every input in the header's `actions` (`set`, `clear`, `use`, `use_entity`, `shape_set`, `update`, `game_event`); `--ground` puts a TNT duper on the judge's ground at its declared stand (or `--drop`). `redsim-fuzz trace-compare` replays every kind; the sweep of every published pick (2026-10-21) is `site kinds --paths` through it |
| `redsim-fuzz trace-compare <trace>... [--max 6]` | redsim against the real game, tick by tick: builds the trace's design like `clock sim`, applies its inputs, and compares every cell of the watched box and every entity (position, motion, yaw, pitch, bit for bit) after every tick, and the level random's state when the trace records it (redsim mirrors it from the header's `seed` and `chunks`; a drift names its draw distance). One line per trace: `OK` (N ticks identical), `DIVERGES` at the first differing tick (up to `--max` differences), or `UNMODELED` (what redsim lacks) |
| `redsim-fuzz archive-check <archived designs>... [--problems]` | every archived clock / circuit run on redsim (`clock sim` / `circuit sim`) and its line compared with the real-game judge's line in its header (the last field aside): `OK`, `DIFF` with both lines, or redsim's verdict and why; then the totals and the problems grouped by reason |
| `redsim-fuzz recipes-gen [--data DIR]` | regenerates `crates/redsim/src/redstone/recipes_gen.rs` (crafters): every `crafting_shaped` / `crafting_shapeless` recipe of the game's data whose ingredients redsim's items can fill; fails when a code-defined recipe kind names one of redsim's items. Run it after adding items to `container::MORE_ITEMS` |
| `redsim-fuzz loot-gen [--data DIR]` | regenerates `crates/redsim/src/loot_gen.rs`: the block loot tables of every block redsim can build (or its wall variant), for a block broken with no tool and no explosion radius (`dropResources`, TNT with drop decay off): `survives_explosion` passes, tool predicates fail, `match_block` tests the state, constant counts; a table that draws from the loot random (glowstone, gravel, leaves, sea lanterns) is kept as an error redsim reports. Run it after redsim learns new blocks |

| `mctas clock shot <design>... [--out-dir designs/clocks/img] [--tick T] [--strip N] [--width 800] [--hold-ms 1500] [--redo]` | NOT part of the archive flow any more (vjeux 2026-10-19: "we don't need images anymore, we should render them": the website renders every build with redsim; the existing pictures stay). Pictures in the REAL client (uses an X display: vjeux allowed it on clockinfra's dedicated box), one client session for all clocks, ~2 s a clock: each clock alone on a grass floor at noon (two sandboxes used in turn, cleared, never dropped; the camera always enters from the other one so the client loads it fresh), kicked like the judge, at the most active moment of a period after its warmup with nothing in motion (or `--tick`; for clocks redsim does not model, mid-period from the archive's judge line), an orange stained glass block where the output signal is (the output cell when air, else the air cell the output powers or its neighbor most visible; written raw only for the picture), a still camera framing its swept box (center crop; an elongated clock, longest side over 16 blocks and 5x the next, gets a 16-block close-up around its output), saved as `<out-dir>/<design stem>.jpg`. The clock is built with its lowest cell at y = 72 (the camera then stands above the client's horizon height, below which the sky beyond the floor is drawn black); the floor reaches 96 blocks around it and the client gets chunks 6 chunks around the camera. A design with entities (a minecart) stops one tick short of its moment, the camera arrives, and the last tick runs with the viewer there (a frozen level never sends its entities to a viewer). A dark (mean luma < 70), nearly uniform, or dark-topped (mean luma < 70 in the top 12%) frame is taken again after a longer wait (`BAD` after 2 retakes); `--redo-bad` takes again only the pictures that are missing or fail these checks. `--strip N` also tiles N moments of one period (`<stem>.strip.jpg`). `--ticks A..B` pictures every tick from A to B as well (`<stem>.t<tick>.jpg`, tiled as the strip: real-game frames tick by tick); `MCTAS_CLIENT_PARTICLES=0` gives the client all its particles (default 2, minimal: an explosion's). Existing pictures are kept unless `--redo`. `--grid` (experimental: neighbors show) puts many clocks in one world. `--still` pictures each design exactly as written: placed like the judge, no kick, nothing stepped (one tick with the camera there when it has entities), no marker, framing its blocks and its entities' cells (the still frames of `mctas web snapshot`, for `web compare --still`: the viewer and the game on the same blocks). `mctas clock shot-compare DIR [--against designs/clocks/img]` prints the SSIM of each picture against a trusted set |
| `mctas clock video <design> --out f.mp4 [--periods 2] [--min-secs 8] [--tps T] [--card] [--relabel] [--dry-run]` | (coordinator: shared X displays) the clock running in the real client: the kick and warmup stepped unseen, then >= 2 periods and >= 8 s filmed (10 tps, faster for long periods: <= 60 s), a still camera framing the swept box (the clock filmed with its lowest cell at y = 72, above the client's horizon height: no black sky band), a corner label (period; family, blocks, box; duty, start; name), an optional title card; H.264. The clean clip is kept as `f.clip.mp4` (`--relabel` recuts in seconds); `--dry-run` prints the plan and writes the label image |
| `mctas clock montage <design>... --out-dir DIR [--periods 6,8] [--min-secs 6]` | (videos) per period, every given clock's labeled video (each filmed alone; clips already filmed are reused) after a title card (`P gt, N clocks, smallest first`), concatenated into `DIR/montage-p<P>.mp4`; the per-clock videos stay in `DIR/clips/` |

Clock checks in `door selftest`: `clock:sim-calibration` (quick: the 8 calibration clocks' known
period, warmup, output, family, blocks and box), `clock:sim-negative` (quick: the observer pair
placed silently: STOPS), `clock:verify-calibration` (full suite, the real game: observer pair and
subtract comparator robust, redsim agrees), `clock:verify-negative` (full suite),
`clock:verify-entities` (full suite: the observer pair beside a minecart on a rail is robust; with
the cart in the air, falling away, it is rejected).

## Census results (redsim, `core` palette, self-start or one block placed last)

`designs/clocks/census/<box>-k<k>/`: `summary.md`, `minimal.tsv`, `designs/` (the smallest clock of
each (period, family); every one of them verified `robust` in the game by `clock verify`).

| box | k | clocks | minimal types | periods: smallest clock per family |
|---|---|---|---|---|
| 2x1x1 | 2 | 1 | 1 | P6: observer pair (2) |
| 3x1x1 | 3 | 33 | 2 | P6: observer pair (2); P12: observer pair + copper bulb (3: the bulb halves the pair's pulses) |
| 2x2x1 (vertical) | 4 | 3621 | 9 | P6: observer pair (2), sticky piston + slime + redstone block (3: QC power from the block above, dragged away by the slime), + note block / lamp / observer variants (4); P8: 4-observer ring (4); P12: sticky piston + slime + redstone block + copper bulb (4) |
| 2x1x2 (flat) | 4 | 1073 | 5 | P6: observer pair (2), sticky piston + slime + redstone block flat (4); P8: 4-observer ring (4) |

Every other filling of these boxes with that many blocks is not a clock (on redsim, with these
starts): with <= 2 blocks the observer pair is the only clock; with <= 3 blocks in a 3x1x1 row, the
pair alone or with a bulb. The bigger boxes (3x2x1, 3x1x2, 2x2x2, 3x1x3, 3x3x1, 3x2x2 with k 4,
3x3x2 with k 3) are merged by `clock census-report` into `designs/clocks/census/README.md`.

### Periods 2 and 3 (vjeux's question; period 1 is no signal by definition)

`clock census --palette full --max-period 3 --warmup 60` (every modeled component that needs no
item or entity: the `core` palette plus honey, obsidian, oak and iron trapdoors, powered and
activator rails, fence gates, droppers, leaves; ~100 states; self-start or one block placed last;
a candidate whose whole state does not repeat within 12 gt is dropped early) finds NO clock of
period 2 or 3 in: 2x1x1 (k 2), 3x1x1 (k 3), 2x2x1, 2x1x2, 3x2x1, 3x1x2, 2x2x2, 3x3x1, 3x2x2, 3x1x3
(k 4), 3x3x2 (k 3), nor in 2x2x2 at k 5 (every filling of the cube with 5 blocks of the ~100
states: 4984 chunks, 5.6 h on 96 threads, 0 clocks). The same runs with `--max-period 4` find the P4 clocks (24 observer types and 2
observer + piston types in 3x2x1), so the pruning keeps real clocks. The game-source arguments for
the families that cannot reach P2/P3 at any size are in `designs/clocks/impossible.tsv` (clk-rt,
clk-obs).

P3 beyond the census (clk-rt, 2026-10-21, vjeux's "Don't we have 3 tick clocks?"): a component of
a P3 clock repeats every 3 gt, so it changes at least twice in 3 ticks, with a 1-gt gap somewhere:
nothing whose consecutive changes are >= 2 gt apart can change in one (observers: on exactly 2 gt;
torches, repeaters, comparators: ticks scheduled >= 2 gt ahead, one pending per position; lamps 4;
droppers and dispensers 4; crafters; hoppers: an 8 gt transfer cooldown; sculk sensors >= 10;
pressure plates, tripwire and detector rails >= 10 gt; copper bulbs halve their input). What is
left are pistons 0-ticking every 3 gt, the blocks they move, and instant followers (dust, note
blocks, trapdoors, rails), clk-obs's necessary structure above; an entity can only reach the
blocks through plates, rails or tripwire, so item and minecart clocks cannot be P3 either.
`clock period-search --period 3` then made 60 M annealing moves on redsim (each a clock judgment
unless cached) in exactly that palette (pistons and sticky pistons in 6 facings, redstone block,
slime, honey, stone, obsidian, dust; a third run added observers, which clk-obs's notes count as a
power element once moved) from the 16 observer-free piston clocks of period 6, 9, 12, 18, 24 and
the P6 observer + piston clocks, inside their boxes grown by 2-3 cells (up to ~20 blocks): no P3. Every run settles on P6 clocks with at most 12 of 19 driver cells on a
3-gt cycle (the two halves of the cycle stay mirror images: an element pushed out cannot come
back within the cycle, clk-obs's argument that a returning move must be a 0-tick spit placed in
the same block-event phase).

Types: `clock::canon` turns and mirrors block states on their text (facing, axis, the four side
properties, rotation, rail shapes, crafter orientation, hinge) and drops the dynamic properties
(powered, lit, power, extended, open, locked, distance...): the type key is the smallest encoding
over the 4 rotations and their mirror images, with each cell's `nbt:` data attached (a mirror pair
is ONE type; it is `chiral` when the mirror image is not a rotation of it). The census, `clock
archive` (refuses a design of an archived type: `SAME TYPE`) and `clock catalog` (each type once,
gallery included, the other designs of the type listed as "also found as") all use it.

## Entity clock kits (redsim, `clock kit`)

The smallest entity clocks have 8 to 20 pieces, out of reach of a filling census (k <= 4-5), so
each kit enumerates one mechanism's space (`crates/clock/src/kits.rs`; 2026-10-19, coordinator's
P2). Every design written is shrunk to minimal, re-judged `robust` in the game by `clock archive`
(16 builds) and has an exact trace in `designs/clocks/traces/exact/` that redsim replays
tick-identically. Results in `designs/clocks/tracks/clockinfra/kits/<kit>/` (`clocks.tsv`: every
type; `summary.md`; `designs/`), archived as `kit-<kit>-<entity>-p<P>-<n>b`.

| kit | space | candidates, run | clock types | archived (the smallest per period and entity) |
|---|---|---|---|---|
| K1 `shuttle` | straight track of 2-7 rails (plain, powered, detector) on stone or (powered rails) redstone blocks, stone bumper or nothing at each end, a minecart on a rail at rest or at 0.4 b/t either way | 27.4 M, 70 s on 28 threads | 1289 | 28 periods from 25 to 197 gt: 11 pieces (P25, P30: `S P P D P S`), 13 (P28, 29, 33, 40, 66), 15, 17 |
| K2 `bouncer` | an oak boat (2x2 pool), item or armor stand on a 1-2 deep bubble column over soul sand or magma, walled; a tripwire across at height 1-4 (either axis, either row), an optional cap (stone, slabs, trapdoor, lantern) 1-2 above it | 5 k, 60 s | 343 | 12: boats P72, 73, 74, 77, 81 (19 pieces), P82 (18, one less than clk-misc-boat-bouncer), P134; items P26, P133, P621 (11 pieces), P887, P991 (the item "P12", archived 03:57Z, was a horizon artifact: its item never repeats exactly, so a short block pattern near the end of the window looked periodic; un-archived 2026-10-20 when the entity rule began requiring the period over the second half of the horizon, clk-misc's find; its exact trace stays as a redsim test, tick-identical over 4101 gt) |
| K3 `dripleaf` | a big dripleaf on dirt with an entity on its leaf (armor stand, item, oak boat), an observer watching the leaf, and up to 3 dust / stone / repeater or 4 dust / stone in a 5x4x4 box | 15 M, 23 min | 6 | 3: P4, 8 pieces, with each entity (the leaf tilts under any of them) |

The items of the kits carry `Age:-32768s` (else an item despawns after 6000 gt and the clock stops
after 5 minutes) and `PickupDelay:32767s`.

The fluid census (P1, 2026-10-19: `clock census --palette 'core,minecraft:water[level=0],minecraft:soul_sand,minecraft:magma_block'`,
59 states) finds NO clock with water or a bubble column in its family in 3x2x1, 3x1x2, 2x2x2,
3x3x1 and 3x2x2 at k 4 (29055, 21011, 60736, 126130 and 363357 clocks) nor in 2x2x2 at k 5
(3578429 clocks): the core clocks again, some with soul sand or magma as an inert support. A water
clock needs more room (the smallest archived, clk-misc-bubble-flip, has 12 blocks).

## What redsim models exactly

`clock sim` is exact for every block `redsim::Block::parse` accepts ([mechanics.md section
12](mechanics.md#12-redsim-the-exact-model-track-redmodel)): dust, repeaters, comparators, torches,
levers, buttons, observers, lamps, waxed copper bulbs, note blocks, targets, iron/oak trapdoors and
doors, oak fence gates, powered/activator rails, dispensers/droppers (single stack; dispensers place and
pick up powder snow with buckets), crafters (with
their items: the game's crafting recipes for redsim's items, generated by `redsim-fuzz recipes-gen`),
hoppers, jukebox (records, songs, comparator and signal), leaves, scaffolding, pistons and sticky
pistons with moving blocks, slime and
honey, gravity blocks, sculk and calibrated sculk sensors, daylight detectors (the sky light's fixed
point at the judge's frozen day time 0), the plain blocks of its table, and the
constant comparator sources (cake, composter, end portal frame, respawn anchor). Since the fluids +
entities port (2026-10-19, tracks clockinfra and clk-misc) it also runs water and bubble columns,
boats and chest boats, minecarts and hopper minecarts, armor stands and item entities, ported from
the game's code in its operation order (`Entity.move` / collide / step-up, `checkInsideBlocks`,
`AbstractBoat`, `AbstractMinecart` + `OldMinecartBehavior`, `LivingEntity.travel`), and the blocks
they touch: pressure plates, tripwire and hooks, plain and detector rails (`RailState` for every
rail kind), big dripleaf, wall banners, lanterns, stone slabs, chiseled bookshelves, and
container entities as hopper sources and targets. Fences too (2026-10-20, clk-misc, `fence.rs`: the
12 wooden fences and nether brick, `connectsTo` on every shape update, the 1.5-block collision post
and arms, CENTER-sturdy tops; traces `rt-fence-connect`, `rt-fence-bud`, `rt-fence-entity`), and all
16 wool colours. Entities pushed by moving blocks too (2026-10-20,
clk-misc: `PistonMovingBlockEntity.moveCollidedEntities` with the slime bounce, `moveStuckEntities`
for honey, `moveEntityByPiston` with `NOCLIP` and `Entity.limitPistonMovement`, and the collision
shapes of pistons, heads and moving blocks for entities; gated by the `rt-push-*`, `rt-slime-bounce`
and `rt-honey-drag` traces in designs/clocks/tracks/clk-misc/redsim-tests/). Each is gated tick by tick against real-game
traces (`redsim-fuzz trace-compare`, below). The LEVEL RANDOM too (2026-10-20, clk-misc, opt-in:
`World::mirror_level_random(seed, chunks)`): each bridge sandbox is its own level whose
`LegacyRandomSource` the bridge reseeds to the sandbox seed at create and reset; with no player it
draws `randomTickSpeed` (3) x `nextInt(48)` per forced chunk every tick (`ServerLevel.tickChunk`,
between fluid ticks and block events), and redsim draws at every game spot it models: piston
sounds (extend after a successful move, retract), note blocks (`nextLong`), door, trapdoor and
fence gate sounds, sculk clicks (activate, cooldown end), dripleaf tilt sounds, the tripwire hook's
detach sound, dispenser/dropper slots (`getRandomSlot`: multi-stack droppers pick exactly), the
hopper's pick among container entities, `popResource` (3 per drop), the primed TNT kick, bubble
column particles above a column (8 per entity per tick) and jukebox notes (every 20 gt). Exact
traces record the state after every frame (`rng`, read by the bridge's read-only `random_state`
op; the header carries `seed` and `chunks`), and `trace-compare` / `circuit shulker compare`
check it every frame, naming the tick and the draw distance of any drift. Without the mirror
(clock sim, the census, archive-check) nothing changes. RANDOM TICKS too (2026-10-21, clk-misc
on webrender's cane, `redsim::crops`, opt-in: `World::random_ticks`): `ServerLevel.tickChunk` for
each forced chunk in the game's order (`crops::tick_order`, a port of the simulation tracker's
fastutil hash-map iteration; the bridge queues every forced ticket before the tracker runs them,
so the order is the box's alone: `redsim-fuzz chunk-order-check`), its precipitation draws each
stepping `Level.randValue` on a 0, then each randomly ticking section (checked when its turn
comes) getting `randomTickSpeed` cells from `getBlockRandomPos`; the bridge seeds `randValue` to
`(int) (seed ^ 0x72616E64)` (vanilla's was unseeded: no two runs grew alike). Exact traces record
it after every frame (`rv`) and the chunk order in the header (`chunk_order`); `trace-export
--farm` records a harvester as the farm judge builds it. An entity's own random is mirrored where
the bridge seeds it (summons: `0x6d63746173 +` the summon number; popResource drops: past their
draws), so an item pushed into a block (`moveTowardsClosestSpace`, its neighbours' full-block test
taking a moving piston's shape where it stands) is modeled. BAMBOO (`bamboo.rs`: the stalk's
age/leaves/stage, growth with `nextInt(3)`, the raw brightness above at least 9 from the sky light's
bounds, `nextFloat` from height 11, the shoot; the position-seeded XZ offset of its 3 px collision
column, which entities hit) and KELP (`kelp.rs`: a water source inside, growth into water with
`nextDouble() < 0.14`, a top over kelp turning into stalk and a stalk losing its top turning into a
top of age `nextInt(25)`, the water tick on every shape update) too, and STEMS (`stems.rs`,
2026-10-21: pumpkin and melon stems on farmland, `nextInt(int(25 / speed) + 1)` with
`CropBlock.getGrowthSpeed`, the raw brightness at least 9, at age 7 a side drawn with `nextInt(4)`
and the fruit on #supports_stem_fruit, the attached stem turning back into a stem when its fruit
goes; farmland's moisture random tick, its turning into dirt under a solid block, and its
`fallOn`, whose trampling roll draws `nextFloat` from the level random for anything landing on it,
items included). A melon's slices (3-7) draw from the server's random sequence of its loot table
(`RandomSequences`: Xoroshiro128++ seeded from the salt and the MD5 of the table id); vanilla keeps
one per table for the whole JVM, so the bridge clears them and sets their salt to `(int) (seed ^
0x6C6F6F74)` at every sandbox create and reset (`Sandbox.lootSaltOf`), and redsim's mirror starts
there (`World::mirror_level_random` sets `loot_salt`). CACTUS too (`cactus.rs`: growth from the
random tick climbing its column, the age-8 flower roll with `nextDouble`, the block that cannot
stand beside a solid block breaking itself a tick later, 1 damage a tick to an item touching its
cell), with the sculk sensors' entity vibrations it relies on (`sculk::dispatch_at`: HIT_GROUND
and ENTITY_DAMAGE at the entity, wool and wool items dampening; a STEP from an entity standing on a
sensor). Gated by
cane-mc-8b-judged, cane2-mc-13b-judged, bamboo-8b-judged, bamboo-down-9b-judged,
melon-mc-7b-judged and cactus-wool-9b-judged (rts 300, intruders), rt-bamboo-tall (a stalk and a
shoot to 16), rt-kelp-grow (growth, two cuts), rt-melon-stem (a stem's growth, two melons, the
pop, the detach), rt-farmland-fall (items landing on farmland and on mud), rt-cactus-grow-s2
(growth, a flower, a self-break) and rt-cactus-items (an item dying on a cactus, landings heard
at their exact distance or dampened by wool, a STEP) in the trace suite (the slices' count is
not in a trace: `circuit farm-run --compare` checks the delivered counts). THE FALL-DISTANCE
RAYCAST too (clk-misc 2026-10-21, clk-rt's sweeper cane farm: items flung over water by a moving
slime): after a move of at least 1 block by an entity with a fall distance, `Entity.move` clips the
move's segment against FALL_DAMAGE_RESETTING blocks (scaffolding is redsim's only one: a full cube)
and water (`traverseBlocks` + `VoxelShape.clip`: each water cell's box), and a hit resets the fall
distance (`entity::collide::fall_reset_hit`). A water state's box height is the one the game's JVM
first computed for that state (`FlowingFluid.shapes`: 1 with water above that first cell, else its
own amount / 9), which redsim cannot know: the segment is tested at both heights, and when they
disagree the fall distance is left as is. No modeled outcome depends on the reset: redsim reads a
fall distance only as `> 0` at a landing, which the landing tick's own drop re-establishes
(`fallDistance -= ya` before the onGround test), and no modeled entity takes fall damage; a model
that reads its value must turn that unknown into a model note. Gated by rt-fall-water (items
dropped 24 to 39 blocks into 1-deep water over farmland, a bare-farmland control: tick-identical,
also without the reset). OPEN-TOP HOPPERS too (clk-misc 2026-10-22, found by `farm-run --compare
--probe` on clk-rt's sweeper farm at tick 19692): `HopperBlock.entityInside` runs the hopper's
`tryMoveItems` for an item entity whose box meets its suck box (`Hopper.SUCK_AABB`, cell
coordinates), in the entity's tick, before the block entities: a hopper off its cooldown pushes
there (a full one too), takes the entity unless full, and starts its cooldown, which its own tick
then counts down (`container::hopper_entity_inside`). The probe (`farm-run --compare --probe`)
diffs every container's slots and hopper cooldown with the blocks and entities, and shows the
differing containers' last three ticks. RIDDEN MINECARTS too
(clk-misc 2026-10-21, the rail study's twin): passengers (`Entity.vehicle` / `passengers`), `/ride`
(`startRiding` forced, ENTITY_MOUNT), a passenger skipped by the entity loop and ticked right after
its vehicle (`tickPassenger`: `commonTick`, `rideTick`: its motion zeroed, its own tick, then
`positionRider`: the minecart's seat 0.1875 up), the ridden cart's 0.75 move scale and 0.997
slowdown, `/kill` (passengers stop riding, ENTITY_DISMOUNT and ENTITY_DIE; a container minecart's
`dropContents` draws three `nextDouble` per slot, empty ones too), a detector rail's comparator
output (the first container minecart's fill level). Gated by the rail judges' recorded runs
(`circuit station-run | cart-run --trace`: header `raw`, the judge's track and chest, and `script`,
its carts, riders, presses and kills by network id): station-slope-v2-judged,
station-slope2-v4-judged, cart-unloader-t3-judged and cart-loader-t5-judged in the trace suite.
DISPENSED ITEMS too
(clk-misc 2026-10-21, `World::spawn_dispensed`): a dropper shooting an item into the world
(`DefaultDispenseItemBehavior.spawnItem`: center + 0.7 facing, y minus 0.15625 sideways or 0.125
vertically, the motion from the level random, `pow = 0.2 + 0.1 nextDouble()` then three triangles of
spread 0.103, no pickup delay); vanilla leaves the item's own random unseeded, so the bridge reseeds
it to `seed ^ (0x64697370 + n)` (n: the sandbox's n-th dispensed item, its own count) and redoes the
constructor's yaw and drift draws, which redsim mirrors. Gated by rt-dispense-bubble (shots into a
bubble column) in the trace suite and by `circuit lift-run --compare-verify` on the bubble-column
elevators. TNT EXPLOSIONS too (2026-10-20,
clk-misc, `explosion.rs`, with TNT entities and the mirror): `ServerExplosion` in its order: the
1352 rays (16x16x16 surface) with the level random's `0.7 + 0.6 nextFloat()` jitter, blast
resistance (the `Blocks` table; fluids 100, so a TNT in water destroys nothing), the exploded set
in Java `HashSet` order then `Util.shuffle`; entities within 2r hurt and pushed by their exposure
(`getSeenPercent`: collision raycasts, `BlockGetter.clip`) - items lose health, vehicles gain
damage, other TNT and falling blocks fly; each destroyed block's loot (`loot_gen.rs`, generated by
`redsim-fuzz loot-gen`) merged into stacks of 16 and popped as seeded items (`rng::Drop`, like the
bridge), `setBlockAndUpdate(air)`, a TNT block primed with `nextInt(20) + 10`. Every block a
neighbor update or a piston destroys pops its loot the same way (`World::drop_resources`). Gated
by `rt-tnt-floor`, `-obsidian`, `-water`, `-chain`, `-push` and `-items` (exact traces with the
level random checked every frame). For robustness over every random outcome (vjeux): injected
kicks and chain fuses per TNT (`World::tnt_kicks`, `chain_fuses`), `Rolls::Max` (every ray at the
largest roll: surviving it is surviving every roll), and `robust::critical_points` (a kick sweep
with every change bisected). An explosion destroying a moving block drops what it moves
(`MovingPistonBlock.getDrops`) and runs `finalTick` on an air cell; rays stop at the build height
(-64..=319: a TNT that fell into the void destroys nothing); entities only tick in the sandbox's
forced chunks (`World::entity_chunks`, set with the level random: a TNT or item flying out of them
freezes, like `ServerLevel.tick`'s entity-ticking range). The 7 TNT archive designs are exact
traces too (200 gt: blasts breaking moving pistons, void blasts, frozen TNT). Not modeled (a model
error): an explosion near sculk sensors, a
container's spilled contents, a vehicle or armor stand it breaks
(their drops use the item's own random). Anything else (unwaxed copper, other items' dispenser
behaviors, other mobs, a crafter result or a record redsim does not know...)
is `UNMODELED`: the real game (`clock verify`) is the only judge. Every design of the clock and
circuit archives runs on redsim to a line (2026-10-20, `redsim-fuzz archive-check`: 3485 of 3491 print
the judge's own line; the 6 others are listed in
[archive-check.txt](../designs/clocks/tracks/clk-misc/archive-check.txt): the copper-bulb
census clock of the Output note above, and 5 hopper/dropper rings whose warmup the judge counts
differently, their states identical). The browser viewer (native and wasm, `mctas web check
--wasm`) finds the same warmups and periods on the whole archive.
The bridge and redsim emit no `block_place` vibration for a placement (a sculk clock cannot start
from a placement sound; kick it with a moving piston or a lever).

## Calibration clocks

`designs/clocks/calibration/`: classic clocks built from first principles, each measured (never
assumed) by `clock sim` and `clock verify` (8 positions x 4 rotations, before the default became 4):

| design | how it works | P | output on/P, edges | warmup | family | blocks, box | start |
|---|---|---|---|---|---|---|---|
| [observer-pair](../designs/clocks/calibration/observer-pair.txt) | two observers facing each other, each watching the other | 6 | 2/6, 1 | 0 | observer | 2, 2x1x1 | last (the second observer) |
| [comparator-subtract](../designs/clocks/calibration/comparator-subtract.txt) | subtract comparator fed 15 by a redstone block, its output through 3 dust into its own side (15 -> 2 -> 15) | 4 | 2/4, 1 | 2 | comparator+redstone_wire (the comparator's block state never changes: its output strength, in its block entity, does) | 9, 3x2x2 | self-start |
| [torch-repeater](../designs/clocks/calibration/torch-repeater.txt) | wall torch lights a dust ring into a delay-4 repeater pointing into the torch's block | 20 | 10/20, 1 | 0 (whole state: 51, the torch memory fills) | redstone_torch+redstone_wire+repeater | 14, 3x2x3 | self-start |
| [bulb-observer](../designs/clocks/calibration/bulb-observer.txt) | an observer watches a waxed copper bulb; its pulse runs (block, dust, delay-2 repeater) back into the bulb | 28 | 14/28, 1 | 0 | copper_bulb+observer+redstone_wire+repeater | 13, 3x2x3 | last (the bulb) |
| [observer-piston](../designs/clocks/calibration/observer-piston.txt) | a sticky piston lifts a stone in front of an observer whose pulse powers the piston (block, dust under it); the stone is spit on the short pulse | 8 | 4/8, 2 | 3 (whole state: 4) | observer+redstone_wire+sticky_piston | 8, 3x4x1 | last (the stone) |
| [sculk-piston](../designs/clocks/calibration/sculk-piston.txt) | a sculk sensor hears a piston and powers it through two delay-4 repeaters: the retraction comes after the sensor's cooldown | 49 | 30/49, 1 | 5 (whole state: 53) | repeater+sculk_sensor+sticky_piston | 6, 5x2x2 | redstone block 2 gt |
| [hopper-pair](../designs/clocks/calibration/hopper-pair.txt) | two hoppers facing each other pass one item back and forth (`nbt:` line), a comparator reads one | 14 | 7/14, 1 | 3 | comparator+hopper | 4, 3x2x1 | self-start |
| [burnout-torch](../designs/clocks/calibration/burnout-torch.txt) | (clk-rt) torch + delay-1 repeater ring: the torch burns out at its 8th turn-off and relights 160 gt later | 220 | 32/220, 8 | 0 | redstone_torch+redstone_wire+repeater | 10, 3x2x2 | self-start |

All eight: `robust` in the real game (32 builds each), and redsim agrees on period, warmup, output
waveform, family and box. The warmup in the `CLOCK:` line is the block states' (what the real game
shows); `clock sim` also prints when the whole state repeats (hidden state such as the torch
burnout memory or a sensor's last frequency settles later).

## For explorer crates

A track's generator lives in its own crate, already created as a stub for each explorer:
`crates/clock-obs`, `clock-rt`, `clock-sculk`, `clock-item` (each depends on `clock`; its owner
replaces the stub freely). `mctas clock <obs|rt|sculk|item> <args...>` passes every argument after
the track name to that crate's `pub fn run(args: Vec<String>) -> anyhow::Result<()>`, so a track's
commands never touch the shared crates. The in-process API:

```rust
let c = clock::Clock::load(path)?;            // or Clock::from_design(design)
let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 50_000, margin: 4 });
// r.verdict (Clock | Stops | NoRepeat | Unmodeled | Invalid), r.why, r.period, r.warmup,
// r.analysis.family, r.analysis.cells (pos, states, kinds, on, edges, widths), r.output,
// r.blocks, r.swept, r.summary()
let v = clock::verify::verify(&c, &clock::verify::VerifyOptions::default())?; // the judge
```
