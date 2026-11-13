# The return's lift and split (7b90c3f1): the bench's numeric bars

`mctas mobfarm return-bench` (crates/mobfarm/src/returnbench.rs). Every number it enforces, with its
basis: **physical** (a measured failure boundary or a game rule) or **inherited** (a number carried
from an earlier ruling with no measured failure behind it). ironspec's bar audit, the maintained
table: https://artifacts.internalmeta.com/a/2276791626224129 (1872157077300391 is superseded).

## Barred (a failing row)

| bar | value, flag | basis |
| --- | --- | --- |
| wrong lanes, pushes (box touches where a push acts), the napper off its route, riders hurt, batch carts on the hold's route, extra pulses | 0 | physical: the failure itself |
| the release bar: a pop without a departure, a launch before rest, a creep, a departure off an unfired pulse | 0 (`--pulse-gate` or `--day1-taps`) | rule (ironspec 11:43, 14:41) |
| the shaft-entry (foot) pairs, and the napper's foot gaps off path (a) | >= 16 | physical: tick-exact's official exit-top row (side entry: 11 touches, 12 pushes, 13 clean), which redsim matches tick for tick under the adopted push-box convention (ironspec 21:58 / 22:07 PT), + 2 = 15; 16 stands until tick-exact's foot-entry row (lanes -1 / 3), then 15 for the side entry; the inherited 21 / 28 / 21 retired |
| path (a): every y-12 pair (lane 3 -> 7, 7 -> the napper, the napper -> the next lane -1, lane 7 -> the next lane -1 with no napper between) | >= 16 (`--y12-min`, `--y12-nap-min`) | physical: the column bar above (carts rise at the same 0.695 b/gt, so a y-12 gap is the gap at the top); the inherited 23 (21 + 2) and window >= 46 retired |
| path (a): a cart missing y 12, or the column order wrong at y 12 | 0 | rule |
| path (a): lane 7's mouth clearance (its column entry after the preceding cart's box has cleared the mouth cell at y 12) | >= 2 (`--mouth-min`); unmeasured fails as "unmeasured" | physical: the out-of-order mouth contact (ironspec 20:01 PT (2)) |
| block contacts: a cell the carts' moves touch that the file's `# contacts:` line doesn't declare | 0 | rule (ironspec 21:25 PT (4) / 22:05 PT): the lift's walls and caps are declared containment with their evidence rows; a bench-only contact prints PROVISIONAL with its speed change (G1 on 8ed397b2e); read under the face filter (ironspec 02:41 PT: a face the cart moved toward, at its axis step in the game's order, within 0.011, overlapping > 0.01 on both other axes) |
| D-on-J: a J's T off its power's settle after its branch detector's change, or re-shaped by that update (stale before it) | 0 | rule: the `# update-only:` declarations' evidence (push-scan (b), ironspec 19:58 (2b); 9fc3155f 23:22: the D detectors are update-only, the J / K levers and the exit detector (7,-3,4) `# switches:`) |
| the napper's return after its climb top: the exit detector (7,-3,4) FIRES for the napper (a rise, unpowered then powered, within 10 gt before its cell arrival: the rail's entity box meets its 0.98-wide box ~0.3 b early); the napper seated on the hold before the hold rail's first rise (both in bench ticks) | the detector fires; margin >= 2 | rule (ironspec 03:37 PT (2); G1 H3 / H4): carrying a world open's dry first move (37 % at (7,-4,4)); a detector already powered (a cart parked on it) doesn't fire; the siding-R arrival printed for day-1's lane model |
| `--set`: an override that changes nothing | fails the run | rule (ironspec 04:07 PT); each override prints its cell before -> after, and its state on the settled world (07:26 PT C8; G1 on 9a213943f (d): the cell must hold the FULL state it set, a rail shape or a power state the settle changed failing); air over air is a no-op too |
| record-match: a row it doesn't count (an undeclared cell, an unguarded stand-in) | fails the row | rule (ironspec 07:26 PT C1); the sweeps print the rows failing on it alone, so the other bars stay readable while the Nether pad's portal cells wait for record-match's `# out-of-scope:` |
| rider air per lift transit: a rider drowning (air -19, the last value before the game's -20 damage-and-reset, which redsim does within the tick; or a hurt rider whose air ran negative); a rider that rode the column with no air sample, or a run with no transit (NOT JUDGED) | 0 | rule (G1 / ironspec 06:24 PT; 07:26 PT C1 / C2); no air is lost in a bubble cell (26.3's baseTick), the worst point is the top current cell; each row prints the least and the least with a one-period stall there |
| the napper's return in scope (`--napper-lift` with `--napper-delay`): the exit detector, the hold margin (and with `--record` the siding-R arrival) absent | NOT JUDGED fails | rule (07:26 PT C2) |
| junction margins: each cause's settle / clear, where sampled | >= 3 | rule (ironspec 17:20 (b), bar + 2); 07:26 PT C1: now through clean(); a cause with no sample in a row (an event past the run's end) prints NOT JUDGED with the rows that sampled it; G1 on 9a213943f (c): a margin in the run's design (the K causes with the K relay; J(-1)'s re-arm on path (a) unless the napper takes lane -1; the D presses on path (a); the J margins with the split's J's) that NO row of a sweep sampled fails the sweep |
| the exit-top pairs (the contact point) | the y-12 bars | 07:26 PT C6: judged at the top too, y 12 the proxy (one rise speed); G1 on 9a213943f (b): every pair in scope, the foot pair too, and a pair in scope with no measurement is NOT JUDGED and fails |
| STALE: a declared contact its class's scope requires that no row of a sweep hit | fails the sweep (one row: printed, not judged) | ironspec 07:26 PT / 07:47 PT; classes: the bench's own (every sweep), world-open (the bench's own world opens' cells, (-5,22,8), (7,-4,4), (-3,24,9): required in the restart sweeps; allowed in a row with a world open or a login's freeze, undeclared in any other row: G1 H13), external (another run's evidence: printed, not required), and fidelity-gap (G1 10:38: tick-exact's game LOAD rows meet (-6,22,7) and (-6,22,8), redsim's world open never does, 0 of 33,000 restart rows on 9a213943f: their declaration reads "(game LOAD rows: met; redsim: never met over 33,000 restart rows: fidelity gap open for cc767b78)", printed and not required in redsim rows; neither dropped nor silently scoped). Under the face filter the riser's vertical moves never meet the shaft's side walls at the foot and the side entry, the foot wall's upper cell, the top's west wall, the side entry's sign glass or the napper foot wall's upper cell (0 of 829 jitter and late rows): those twelve declarations are dropped |
| the merge row's join Ts (-1,0,7) and (3,0,7) (form (C), ironspec 10:12 / 10:23 PT: each T's support (x,-1,7) slime, strongly powered by a lit torch (x,-2,7) on glass (x,-3,7), so the T is powered and every re-settle lands north_west, its powered preference) | 0 ticks off north_west | rule (9fc3155f / onechunk's junction-check, ironspec 09:44 PT): read at every tick's end on the K-relay line; a plain T (no source) re-settles north_east at a signal-source update reaching it (RailBlock.updateState: the changed neighbour a signal source, 3 potential connections), which `--toggle`'s planted lever shows; the `# shape:` line declares each "held: powered via the slime by the torch; build: the torch last; repair: re-place the torch"; (x,-4,7) and the cells below the torch's glass are keep-out (non-solid-render: 9fc3155f's buried positive) |
| the world open (`--restart`) | its game-time delta = the load's own entity-free ticks; a moving cart holds its position across it; either measurement absent in a world-open row is NOT JUDGED and fails | G1 on 84cd993fd (1); on 9a213943f |
| lightning (`--thunder SEED`, forced seeded rolls): a rider struck (converted, its cart destroyed); `--rod X,Y,Z`: a rider in the rod's bolt box at any tick, the rod not its column's top, a flammable block in its box, a component its struck pulse reaches | 0 | rule (ironspec 07:52 / 07:59 PT: one rod per copy at the farm's highest point; the no-rod run is the positive) |
| positive controls | each must bite | ironspec 07:26 PT C3: the tool runs them after the rows on the run's own options, each judged on its bar's own count AND on the row failing clean() through it (G1 on 9a213943f (i)): a declared contact undeclared; a real block in the column; `--set`'s no-op; a stale T; the hold's lever thrown at the napper's detector; the exit detector unpowered; the exit held 400 gt for the air bar; a record cell changed after laying; lane -1's join without its source and a lever beside it toggled (`--toggle`: the per-tick T bar); one that doesn't bite fails the command; `--no-controls` claims no verdict |
| heads under the roof's underside | < 28 | physical: geometry |
| each lane's delivery entering its merge cell | 0.30..0.48 | inherited here: ironfarm's part spec |
| the settle lock k | 42 (`--pulse-gate 42`) | physical: the bar is the official R_max + 2 = 38 at R_max 36 (ironspec 20:53 PT; no F term); 42 stays, 4 over; R_max awaits the position-checked restart re-run (37 / 38 would make k 44) |

Every bar reaches the exit code (G1 H1): each sweep counts its rows failing a bar or erroring (fault rows excluded), its STALE declarations and no row judged; the command prints `exit: N failure(s), M control(s) not biting` and exits non-zero on either.

Fault rows (`--block-exit T,N`: the exit booster replaced by glass for N gt) are out of scope (ironspec 07:05 PT): each prints its before -> after, its verdict is FAULT ROW, and the sweeps exclude it from every verdict and print it apart.

## Printed only

- Sample counts (ironspec 23:56 PT, no vacuous passes): each barred quantity prints how many
  samples it had (the y-12 pairs measured per pair, the D-on-J detector changes per J, the cart-ticks
  the contact check read, the ticks holding both carts per `--lift-pair` gap, the rows behind each
  sweep least); a barred quantity with none prints NOT JUDGED (0 samples) and fails the row, unless
  the run's scope excludes it (one batch has no next-batch pairs; the napper after the last batch
  has no next lane -1).
- Fluid ticks (ironspec 00:27 PT): every run schedules each water cell's fluid tick at the start
  (`LiquidBlock.onPlace`, as placing the cell does) and prints the count; water with none scheduled is
  an error.

- The junction and K(x) settle / clear margins, per switch cause with its phase: cc767b78's judge
  bars them (tick-exact's official boundary row: block phase 1 gt, entity phase 2, + 2). The
  arithmetic is `settle_margin` / `clear_margin` (its convention and a scripted-timeline unit test in
  the source): the controller's writes before tick t are read in tick t, and each cart is recorded
  after the tick, so a cart recorded in a junction's cell at e..=l reads it in ticks e + 1..=l + 1;
  a switch written at s is read from s: settle = (e + 1) - s, clear = s - (l + 1) - 1; a D press
  timed from its press tick adds + 1 / - 1. The bench's `--driver-latency` switches the
  detector-triggered ones at press + 4 (set) / + 6 (reset), day-1's driver.
- K(x) names the split's K T at (x-2,25,9) for the napper's lane x (ironspec 22:03 PT), apart from
  k_sw (the siding, line 14) and k_lock = 42 (the (c') lock).
- The single-occupant margin: 312 against the hold's longest wait + the pulse + clear (the + 20
  retired: ironspec 19:07 PT (5); the occupancy is measured directly).
- The window (lane 7 -> the next lane -1), the least distance in the column, the K window, the same
  pairs at the exit top (the contact point).
- A restart's load: the game-time delta across the helper's call and whether the moving carts held
  (the official load is one block-only tick: delta 1, carts held); barred once cc767b78's helper
  runs that tick.
- The junction 8 was never barred here: it came from 1bd9b1ba1's sweep step, no mechanism (retired
  19:14).

## `--lift-pair`'s push-box convention (ironspec 21:58 / 22:07 PT)

A contact counts between two carts, between a cart and an unmounted entity, and when a seated rider
is in a riderless cart's push box (that cart is pushed) or its box overlaps a ridden cart; a seated
rider only in a ridden cart's 0.2-inflated push box is printed, not barred (`Entity.push` moves
neither: `rideTick` zeroes the rider's motion and a cart with a passenger isn't pushed), and so are
two seated riders. `contact_counts` and its unit test. The riders are NoAI villagers, as in
tick-exact's official row. Least clean gap 13, the official value.

## Frames (ironspec 22:07 PT (2b), 22:03 PT (8))

- The record lift file's run line says `--offsets 0,12,24 --station-lag 11`: the emulated station, a
  calibration. It reproduces dfb89499's real station at its offsets 0 / 14 / 24 (lane 3's y-12 at
  REL + 76 both ways). Rows that count toward a bar (the 21:43 bench-cells rule) run with
  `--station-part` (dfb89499's f1q2-station.txt, 3939d2012) at `--offsets 0,14,24`; the
  emulated-station rows (release 0.40 / lag 11 and 0.34 / lag 9) are labelled calibration rows.
- L7 = REL + 95 at a tick's end (the game tick the slope rail rises) = REL + 96 in day-1's frame
  (read at the next tick's start); the napper's hold rail REL + 63 (64).

## Light and lightning (the file's `# light:` and `# lightning:` lines)

- Water (rule C, tick-exact's official water rows; ironspec 01:07 PT (2)): the file's two sources
  spread one cell each into their lift's exit cell and stop: the top-drift currents (-5,23,7) and
  (6,-3,4), designed and load-bearing, saved settled (laid as water level 1) and annotated in the
  file's `# water:` line; the bench prints the settle's new water per run, which must be 0.
- Water mobs (ironspec 22:34 / 22:54 PT): the drowned is the one class (oceans, rivers, dripstone
  caves; block light 0). Every lift water cell has block light 0 in redsim's light engine (sky 14
  under the glass caps), so it is (b) a site rule, or (c) `--light-fix`: two glowstone blocks in place
  of the wall glass (-6,8,7) and (5,-16,4) (9fc3155f's cells, its statics on record7 clean: (7,-16,4) made a new unreached stroll target, (5,-16,4) none), block light >= 1 in every lift water cell; glowstone,
  like glass, is refused by LEGACY_IRON_GOLEM spawning (26.3's SpawnUtil) and conducts no redstone;
  it is opaque where glass is clear. ironspec's rule: (c) if <= 2 blocks with every static clean.
- Lightning (26.3's findLightningTargetAround and LightningBolt, redsim's model; ironspec 07:52 /
  07:59 PT): the target is the nearest rod within 128, else a sky-seeing living entity in the roll's
  column box inflated by 3, else the column top, and a bolt falls only where it rains at the target.
  The lift's top cells (risers' feet >= 20.05, in water: never picked) are exposed to a bolt on a
  column top within 3 sideways at y <= 28.25; the split row's riders see the sky through the glass, so
  they are the pick and the bolt falls dry (and shields a lift-top riser in the same box); everything
  below y 20 is unstrikable. Without a rod: ~0.003..0.015 rider strikes per thunderstorm (arithmetic
  on the model). The cover: one rod per copy at the farm's highest point (ADOPTED 07:52), its bolt box
  free of riders, no flammable in it, its pulse reaching no component; `--thunder SEED` / `--rod`
  bench it (the no-rod run is the positive). Tiled copies: the rod search's 128 blocks cover each
  copy's chunk from its own rod (any point of a 16 x 16 chunk is within ~23 of a rod inside it), and the
  closest rod takes the roll, so a neighbour's rod covers it too.

## Stand-ins (G1's guard, ironspec 18:34)

The `# emulated:` header line of each file lists the bench's writes outside the file. The two
toggled inputs: the hold's (7,-1,8) a floor lever on glass (7,-2,8); lane 7's drive a ceiling lever
(-4,12,6) hung from the sign's glass (-4,13,6), as onechunk's record7 lays it (no new support: no
glass where the build has air), written before tick R. Both non-solid, no top face, no T in their
update reach, powering only their own rail or repeater. The slope rail's printed rises equal
`--lane7-rel` in every row (the positive check on the toggle convention). `--station-part`'s pocket
levers at (x-2,0,0) on glass are declared stand-ins for day-1's line-13 cells; each prints its reach
(the powered components among its and its support's neighbours: day-1's repeater (x-1,0,0) only).
The split row's six J / K levers are declared `# stand-in:` for day-1's latch output and timed taps
(ironspec 03:22 / 03:26 / 04:34 PT: no lever ships in the built farm), one header line each, in
record-match's header format; (9,25,9)'s seven verdicts on `# stand-in-guard:`.

## Record-match (`--record`, ironspec 21:43 PT's bench-cells rule; cc767b78's (a))

The design of record as run (its cells, its own `emulated:` stand-ins read as its cells) is laid over
the bench's box + 3 under this file's own cells and compared over the box + 2: the bench's emulated
cells give way to the record's, its drive stand-ins stay (each pocket's lever, the line dust it
re-shapes, (9,25,9)), each guarded; `--set` cells are the candidate `--set`. Each run prints
`record-match: …` and its block verdict; the sweeps count the rows it leaves uncounted (not failures).
The Nether pad's portal cells (not modelled by redsim) read as stand-ins without verdicts until the
helper's `# out-of-scope:` lands, so no row counts yet for that reason alone.

## World opens (cc767b78's `liveness::restart`, train 2)

`--restart T,ORDER` / `--restart-sweep FROM,TO` run the official load at a world open (N whole-chunk
idle ticks; the sweeps' 20 rows: 10 orders x N = 1, 2), each row printing the helper's note. A loaded
cart's first move is dry in 26.3 (tick-exact's official rows): the riser's landing cells at the top
((-6,22,7), (-6,22,8), (-5,22,8)) and the napper's (7,-4,4) are declared with those rows; the napper's
37 % loss there is carried through its return (the detector, the hold margin, the siding-R arrival).

## Bystander stamps (ironspec 02:18 / 02:37 PT)

Each run prints the return path's possible stamps: the napper's seated box inside a pocket summoner's
10-box at an S tick (REL - 5), with tick, position and summoners (on the hold; on the merge row or the
lift's foot). Their margins at the napper's next own-pocket S are the composed run's (downstream).
