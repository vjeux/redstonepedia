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
| the napper's return after its climb top: the exit detector (7,-3,4) powered with the napper on it; the napper seated on the hold before the hold rail's rise | the detector fires; margin >= 2 | rule (ironspec 03:37 PT (2)): carrying a world open's dry first move (37 % at (7,-4,4)); the siding-R arrival printed for day-1's lane model |
| `--set`: an override that changes nothing | fails the run | rule (ironspec 04:07 PT); each override prints its cell before -> after |
| heads under the roof's underside | < 28 | physical: geometry |
| each lane's delivery entering its merge cell | 0.30..0.48 | inherited here: ironfarm's part spec |
| the settle lock k | 42 (`--pulse-gate 42`) | physical: the bar is the official R_max + 2 = 38 at R_max 36 (ironspec 20:53 PT; no F term); 42 stays, 4 over; R_max awaits the position-checked restart re-run (37 / 38 would make k 44) |

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
- Lightning (26.3's LightningBolt: every living entity within x +- 3, y - 3 .. y + 9, z +- 3 of the
  strike at the MOTION_BLOCKING heightmap): the riders' heads reach 25.25 at the lift top (cap top
  y 25) and 27.01 on the split row (glass top y 28), so this file's own glass doesn't make them
  unstrikable; the cover is one lightning rod at the farm's highest point (ironspec 23:00 PT).

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
