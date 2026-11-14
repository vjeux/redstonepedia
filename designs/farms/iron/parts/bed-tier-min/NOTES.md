# The bed stop (the lead's napper stop: F1 + Q2, one bed, 24 villagers)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-two`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), farmopt (ca87cf44), G1 (8146b8db), day-1
(a75cc97c). Placed under the station, stop 9 down (ironspec 12:23): loop = local + (0,-27,-1).

## Files

- `bed-tier-fold.txt`: the stop of record for stack (b+), (A) with the run-up (G1's Q17, regenerated
  Nov 13): `mctas mobfarm bedtier --out bed-tier-fold.txt --eject-at-bed --runup --no-loop-booster
  --exit-south --waker-south --exit-boost --hold-slope --wake-sensor --rs-ready --park-sense` (local
  frame; loop = local + (0,-27,-1)): the rider-keyed RS_ready string over R, the sensed park's
  detector rail (0,0,3) with the activator rail (-1,0,3), and the four golem-floor guards (lone
  strings (tripwire, no hooks) in the feet cells over (-1,0,2) and (-1,0,5), glass over the cap
  (0,1,6) and Z1's shutter: 9fc3155f 17:21, ironspec 17:33 / 18:36; glass in both feet cells turned
  the woken napper's flee onto the bed top and armed CANT_REACH after the wake on 363 of 408
  visits, a glass floor at (-1,0,2) met the cart turning at (-2,0,3) at 0.22). No lever and
  no stand-in in it: (-5,1,8) (the cell DAY-1's SHUT drives) and (3,0,4) (the park
  drive's input) are empty for DAY-1's cells, Z1's shutter (-5,1,9) is stored unpowered (its stable
  state with no drive), the slope hold's tap repeater (1,-2,4) is DAY-1's. The design A6 it replaced
  is in git history (cc17d2a8f) and the sections below.
- `bed-tier-fold-bench.txt`: the same with `--bench`: a run-out past R, the queue cell's rail, and the
  stand-ins as declared bench overrides: the shutter's drive lever (-5,1,8), the park drive's lever
  (3,0,4), the slope hold's tap repeater (1,-2,4) with its input lever (1,-2,3) (`--tap`).
- A6's own lines below describe the files as they were before Nov 13.
- Header lines:
  - `keep-clear:` the bed's column (-3,1..3,4): the HOME path's target and the walk's drop (a block
    there moves the target or blocks the drop);
  - `keep-block:` the leaves over the zombie (-5,3,4), a block that must stay: the zombie's cover,
    its eye's sky light 14 or less (G1 on b36302af8: keep-clear means the cell holds no block);
  - `unreachable:` the top hatch's lever cell (-4,1,5): standable (glass under it) but no napper
    reaches it (glass east and west, the hatch's trapdoor north with no floor under the hatch; the
    bed top touches it only diagonally, both orthogonals blocked); never a walk target (flee-check:
    no draw on it from 43 places), never a path node (every path of every bench row, below);
  - `waker-places:` where a napper has the zombie in sight within 8 once F shows it, from the
    bench's sight log (none before F).
- `mctas mobfarm bedtier` writes either and prints the file checks: placement-order cells 0, no T,
  R's reach set, the park's sloped switched rails, the zombie's post (its eye's sky light 14), the
  waker's sight lines and the zombie's sky column (glass, the leaves, then open).

## How it works

The arrival heads west along z -4, open above: the entry booster (3,2,-4) (lit by a torch on
glass), the activator (2,2,-4) (always on), two braked powered rails down a slope (the park), a
capped flat, the curve south, seven flats (glass over the last four), the pickup cell (-2,0,4)
beside the bed's head, the curve east, a capped booster, a capped flat, the slope down, two flats,
R (4,-1,5).

- **No solid-render block in the stop** (ironspec 10:19): rails, levers, torches on glass, so
  IDLE's stroll finds no candidate from any cell a napper stands in.
- **The landing is a chest** (2,2,-3): the activator sets the rider down on it at y 2.875, above
  the cart's box (top 2.7625). A save and load clears Entity.boardingCooldown (not saved): on a
  glass landing the rider stood level with the still-moving cart, 0.01 outside its pickup reach,
  and walked into it (below), so a restart in the eject's first few gt re-seated it and the parked
  cart kept it (160 of 21,600 restart rows at entry + 5, entries 0.4 b/t and up). On the chest the
  boxes never meet. Glass at (2,5,-3): no golem fits on it.
- **The second landing** (ironspec 15:58/16:29, with the way-in's rail descent): a chest at
  (3,2,-3), south of the entry, and an iron trapdoor held open in the cell over it (3,3,-3), facing
  south, its plate on that cell's north side; its lever on the ceiling over it (3,4,-3) on glass
  (3,5,-3); glass east of the chests (4,2..4,-3). The activator's cell is 1 high (the landing's north
  wall (2,3,-4) keeps a napper off the arrival rails and the parked cart), so a calm REST HOME path
  started on an approach rail could not pass it and ended partial, arming a stamp on the way in
  (56 of 72 with the panic held to b8949c96's roll; opening (3,4,-4) alone changed nothing). The
  pathfinder takes the open trapdoor's cell for passable, so the path now runs from the entry onto
  the chests and down the ledge, complete (72 of 72 unstamped); the plate stops a napper on the
  chests walking north onto the entry and back up the approach (the escape row below: 0 escapes in
  every row; the positive control, the lever first tried east of the chest on glass at (4,3,-3)
  with no glass over it, walked 205 of 320 nappers out). The lever's state is not load-bearing:
  with the trapdoor closed (lever off) the path still runs complete (72 of 72 unstamped; the
  pathfinder takes any trapdoor's cell for a TRAPDOOR node, G1) and the stop still contains (the
  closed trapdoor over the second chest leaves 1.81 under the glass (3,5,-3), no standing room: 0
  escapes in 320 rides). The partial control is glass in (3,3,-3): 56 of 72 stamped. A napper
  ejected onto the first chest can follow its riding path east onto the second chest before turning
  back to the ledge (79 of 320 waited there, up to 156 gt, on e9f0f4cbf's ride); the walk timing
  rows include that detour.
- **The entry and the activator are uncapped**, the entry open above. A riding napper in calm REST
  paths from its cart, and keeps following that path after the eject. Its start is the first cell
  over the rider that is not air or passable: under a cap, the cap's own cell, a 1-node partial
  path; the rider then walked back toward it after the eject and stood 120-240 gt against the cap
  (the stamp armed). Open above, the path from the activator's cell jumps onto the chest and runs
  down the ledge, and the path from the entry's cell goes through the trapdoor's: both complete.
  Entries 0.25 b/t and up still wait about 144 gt on the chest before walking (their path from the
  entry's cell), with no stamp (at most 5, was 144).
- **The walk** (all at y 2): the chest, south along x 2, west along z 0 over the approach's glass
  covers, south to (-2,2,2), west to (-3,2,2), onto the bed foot's roof (-3,2,3) (1.80 from the
  bed head's centre: most nappers lie down there), south into the open column over the bed's head:
  a 1.44 drop onto the bed, no way back up. The bed column's first non-solid cell (-3,1,4) is the
  HOME path's target (`findSurfacePosition`), so the walk's path is complete and arms no stamp.
- **The covers** (farmopt 13:44, the cheapest set): (-2,1,0..2) carry the walk, (-2,1,3) keeps the
  flat north of the pickup cell 1 high (no stand-up cell on the approach). The flats (-2,0,-3..-1)
  are uncovered: 2 high under the walk's north wall, reached by no napper (the pickup cell's way
  north is 1 high, the curve (-2,0,-4) 1 high under its glass), never a path node in any row.
- **Night:** it lies down on the walk (REST). The hatch's two iron trapdoors stand open (their
  levers on) and hide the zombie; F (the lie-down + 2, the bed observer's edge; + 10 before the
  stop cuts) shuts them flat (levers off): the zombie (persistent, in a boat at its post
  (-4.5,0,4.5), leaves over it) wakes it, it stands up on the pickup cell, the park releases at the
  wake edge (+ 0; + 30 before the cuts), the cart seats it (0.143..0.207 b/t in every bench row) and
  runs on to R, whose pulse opens the hatch again.
- **Declared for push-scan** (the file's `# switch:` (main's T-reach declaration, which push-scan
  joins to its switch list) and `# always-on:` header lines, local frame):
  the hatch's, the activator's and the park's levers, the second landing's lever and the four
  golem-proofing stone buttons as switches; the entry and exit boosters' torches on glass always-on.
  The buttons are inert sources (never pressed), not T controls: they move to 9fc3155f's `# inert:`
  key once push-scan has it (ironspec 22:46 (2c)); until then `# switch:` keeps them declared.
- **IDLE** (a skip, a stale arrival): it waits where it stands (the chest, the ledge, the bed's
  top, the pickup cell) with no walk target and no stamp, and lies down at REST's first tick.
  The stop has no clearer for a stamp armed before it: a napper arriving in IDLE with one keeps it
  all day and loses HOME at dusk (the positive control below). The way-in must deliver it with no
  armed stamp (ironspec 14:58/15:17, the boundary contract: (ii), 0 arming from the queue's release
  to the first calm REST tick on the ledge).
- **The stand-up:** the hatch cell has no floor, so the pickup cell is findStandUpPosition's answer
  at every wake on both sides (the bed's top comes last, after the surround cells).
- **R's pulse** re-arms the stop (the park braked, the hatch open) and releases the queue.

## What the stop relies on

- **Entry speed [0.02, 0.401] and [0.438, 0.641] b/t** heading west at the entry booster (all
  seated; night, every 0.001 from 0.020 to 0.650). Between them, and from 0.642, the cart parks
  lower on the park's upper slope rail (x 1.04..1.12 against 1.14..1.46) and the released cart,
  topping out near 0.42 instead of 0.46, stops on the flats short of the pickup cell (z 3.71): no
  pickup. The earlier "0.02-0.62" came from coarse steps (0.4, 0.5) that stepped over the hole.
  The timing band is narrower still (below).
- **The pickup speed** (G1's note 1): every bench row seats at 0.143..0.207 b/t. The margin sweep
  (`--pickup-injects`, the released cart set two cells north at a forced speed) seats at measured
  0.100..0.272 and from 0.311; below 0.1 vanilla never picks up (its speed squared must pass 0.01),
  and at about 0.274..0.31 the cart stops dead against the standing villager (a tick-phase window).
- **No armed stamp at the entry** (the boundary contract above), measured on b8949c96's real
  approach by `bedtier-bench --module-offset` (the section below): the way-in's rail descent (C)
  and the second landing together.
- **The stamp that leaves with the napper:** the napper's calm REST HOME walk from the cart arms
  CANT_REACH on the way out (wake + 64..103 gt, at the exit cells; age at R 0-38 gt). The loop
  clears it: the S-bend seat about 250 gt after R (onechunk's phase 3a, ironspec 12:44).
- **The way-in's last cells** (4..8,2,-4) open above (no cap), as is b8949c96's roll.
- **The lift's entry keeps (5,-1,5), east of R, at 2 rail sides or fewer** (7b90c3f1).

## Golem spawn flood

- `mctas mobfarm queue-loop-spots bed-tier-fold.txt --rails --positions <the walk's cells, both
  landings, the bed, the pickup cell, the entry and the activator>`: 0 golem spawn cells off the pad
  from 37 positions; `mctas mobfarm exit bed-tier-fold.txt --part-flood --positions <the same>`: no golem
  can spawn anywhere in the build. (On the first A6 lay both failed: the spawn cell over the bed
  foot's roof, now a floor stone button at (-3,2,3). The leaves are no golem floor: LEGACY_IRON_GOLEM
  excludes LeavesBlock, redsim's `golem_excluded` too; the flood is clean with them open above.)

## Statics

- `mctas mobfarm flee-check bed-tier-fold.txt --home=-3,0,4 --foot=<43 places: the walk, both
  landings, the bed top, the pickup cell, the entry, the activator, the uncovered flats>`: FLEE,
  PANIC-STROLL and IDLE's STROLL draw no cell a path can't reach from any of them (the chest and the
  open trapdoor are not solid-render: no new candidate).
- `mctas mobfarm push-scan bed-tier-fold.txt --switch=<the levers, the buttons, the trapdoor's
  lever (3,4,-3)>`: 0 pairs within push range, 0 Ts, placement-order cells 0; the one switched
  cell left undeclared is the bed foot's roof (-3,1,3) beside the golem-proofing stone button
  (-3,2,3), which nothing presses.
- The waker's lines (`mctas mobfarm bedtier`): with the hatch open, the zombie is out of sight of the
  sleeper, the pickup cell, the bed top, every walk cell and the second landing; with it shut (F),
  in sight of the sleeper and the pickup cell.

## Bench rows (8 seeds; bench, tool unreviewed)

On A6 with the second landing (main d04f8b7d9). Entries 0.1/0.1946/0.2 b/t unless stated. Every
row: HOME 0 lost in the stop, 0 escapes (the escape row, ironspec 16:29: the napper on foot on the
entry or the approach, out of the stop's box, or gone), findStandUpPosition's answer the pickup
cell at every wake (both sides), 0 hurt, 0 boat seats, 0 north paths, 0 riderless carts, the waker
in sight within 8 only after F, and every node of every path the nappers held among at most 25
cells (the walk's, the chest tops, the bed top, the pickup cell and its two rail neighbours, the
exit's rails, the entry): never the declared-unreachable (-4,1,5), never the uncovered flats.

| row | visits, clean | notes |
|---|---|---|
| restarts +1..+250 every 3 gt x 10 orders | 20,160 / 20,160 | lie-down entry + 85..92, R 188..252; stamp at most 38 at a calm REST tick, 0..38 at R |
| re-board: restarts +1..+30 x 10 orders, entries 0.02..0.62 | 24,000 / 24,000 | 0 re-seated (the glass landing: 160 of 21,600 at entry + 5); stamp at most 16 |
| skip at every gt +1..+300 | 7,200 / 7,200 | stamp at most 15 |
| thunder: arrival 11,593, skip +1..+420 | 10,080 / 10,080 | all b, waiting on the chest or the bed's top |
| stale arrivals, day time 0..80 | 1,944 / 1,944 | all b |
| early arrivals 11,500..12,000 every 5 | 2,424 / 2,424 | wait on the chest up to 507 gt |
| window end 23,000..23,990 every 10 | 2,400 / 2,400 | |
| panic tail at entry 0..40 gt | 504 / 504 | lie-down by entry + 141 |
| skip x restart 1..200 gt after it x 10 orders | 13,200 / 13,200 | |
| stale x restarts at 50..11,500 x 10 orders | 4,800 / 4,800 | |
| entries 0.25..0.62 (the night row) | 40 / 40 | lie-down entry + 87..222, waiting about 144 gt on the chest first; stamp at most 5 |
| entry 0.171 b/t ((C)'s descent, ironspec 16:20): night, restarts +1..+250 every 3 gt x 10 orders, skip at every gt +1..+300, panic tail 0..40 | 8 + 6,720 + 2,400 + 168, all clean | lie-down entry + 85..91 (night 87), R 199..252; stamp at most 38 |
| pickup margin: the released cart at a forced speed | 64 / 88 | seated at measured 0.100..0.354 but the injected 0.143 (measured below 0.100) and 0.322..0.35 (the dead-stop window, above) |
| two visits, restarts every gt over +1..+321 x 10 orders | 77,064 / 77,064 | (a1) 0 calm-REST ticks within 16 of the occupied bed; A within 17 at C's lie-down 0 (an inherited margin); A's R to C's lie-down 122..361 gt |
| two visits, A at day time 0..80, C queued | 192 / 216 | HOME losses by place: 0 rows in the stop's box, 24 only outside it (C at the bench's queue cell, at 12,013, before its entry) |
| two visits, a skip in A's visit (every 10 gt) | 178 / 1,032 by HOME | 653 rows lose HOME only outside the stop (C on the bench's queue cell through the day, at 12,013..12,034, before its entry); 201 in the stop's box: C, teleported from the bench's queue with a stamp armed there (no post). Void (ironspec 15:17): the bench queue's; the real-queue rows (next section) replace it |
| control `--no-presence-gate --empty` | 24 out | the empty cart leaves |
| control `--trapdoor-caps`, restarts +90..+180 every 4 gt x 10 orders | 5,520 / 5,520 | no path off the pickup cell after a restart in A6, so the contact mode no longer shows |

The two-visit rows still run on the old corner queue's cell and transit; the real queue's rows are
the next section's. The restart rows above sampled every 3 or 4 gt stepped over the restart trap's
ticks (entry + 2 and + 3) and are pre-official-load (ironspec 22:46 (2b)).

## With both stop cuts (ironspec 17:35-19:33: day-1's controller line lays them)

The park released at the wake edge + 0 (was + 30) and F at the lie-down + 1 or + 2 (was + 10):
`--release-after 0 --after-sleep 1` (or 2) on both benches (the two-visit bench took the flags
too). 0 blocks; entry to R 188 / 207 / 252 -> 150 / 169 / 209 at entries 0.171..0.2 with restarts.
In vanilla LAST_SLEPT is set at the lie-down (Villager.startSleeping, which also erases
CANT_REACH), so the 1-gt nap counts (tick-exact's official row). The release waits for the wake
edge (the presence gate), so the released cart can't meet a napper still in bed: the new bar (0 in
every row): no pickup before the wake, none off the bed while asleep, the released cart never in
the pickup cell before the wake. Its positive control (G1's CHANGES 4 (3)): `--early-release
--after-sleep 100` (the park released at the lie-down edge, the napper held asleep) fires in 24 of
24 visits ("seated off the bed while asleep (the unbridged pickup)") and the run exits 1. The
benches' defaults are the accepted design (F at the lie-down + 2, the release at the wake edge +
0; 10 / 30 the pre-cut control), and every cart's contact with a block is printed per row (ironspec
21:25; A6's file declares none, so any contact fails its row). The full set with `--after-sleep 1` (8 seeds; bench, tool
unreviewed): every row clean, HOME 0 lost, 0 escapes, the early-pickup bar 0, the stamp at a calm
REST tick at most 39, (-4,1,5) never a path node:

| row | visits | entry to R (min / median / max) |
|---|---|---|
| night | 24 | 164 / 179 / 306 |
| re-board, entries 0.02..0.62 | 24,000 | 150 / 176 / 321 |
| restarts +1..+250 every 3 gt x 10 orders | 20,160 | 150 / 169 / 209 |
| the same with F at lie-down + 2 | 20,160 | 151 / 170 / 209 |
| skip at every gt; thunder; stale; early; window end; panic tail | 7,200; 10,080; 1,944; 2,424; 2,400; 504 | |
| skip x restart; stale x restarts | 13,200; 4,800 | |
| entries 0.25..0.62 | 40 | |
| entry 0.171: restarts x 10 orders; skips; panic tail | 6,720; 2,400; 168 | 161 / 174 / 209 (restarts) |
| pickup margin | 64 of 88 | the same three injected failures (below 0.100; 0.322, 0.35) |
| two visits: restarts every gt +1..+321 x 10 orders | 77,064 / 77,064 | (a1) 0 calm-REST ticks within 16 of the occupied bed; A never within 17 at C's lie-down (an inherited margin); A's R to C's lie-down 122..361 |
| two visits: stale; skip | 192 / 216; 238 / 1,032 by HOME | the losses C's on the bench's queue (void, 15:17) |

The maxima of 306..321 are the fast entries' chest wait (below) and, in the re-board rows (restarts
at every gt +1..+30), the restart trap. **Labels (ironspec 22:46 (2b)):** the restart rows in this
table sampled every 3 gt stepped over the trap ticks (entry + 2 and + 3; the restart trap below) and
ran before the official load (cc767b78's helper: one block-only tick after the load): their maxima
(209) are pre-official-load and say nothing about + 2 / + 3; they are re-run on the helper at every
tick.

## The timing band (ironspec 19:18-19:16)

A6 catches entries in [0.02, 0.401] and [0.438, 0.641] (the correctness band, above), but its
timing band is narrower: from 0.28 a riding path last computed in the entry's cell makes the napper
wait about 144 gt on the chest before walking (no stamp), and entry to R reaches 275..335. Lane C's
cap holds only with entries in [0.02, 0.27] ((C)'s entry bar; the water-drag descent delivers
0.12..0.20). The per-speed sweep, both cuts (F at lie-down + 2), 8 seeds (`--speeds` at 78 speeds
0.02..0.62, dense at 0.05..0.07 and 0.13..0.14; `--r-bar 209` prints the v_slow line):

| rows | entry to R over [0.02, 0.27] | worst speeds |
|---|---|---|
| night, 32 seeds per speed | at most 186 | 0.0592..0.068: 167 / 175 / 186 |
| restarts +1..+250 every 6 gt x 10 orders (262,080 visits; sampled every 6 gt, so the trap ticks + 2 / + 3 stepped over; pre-official-load) | at most 211 | 0.0592..0.068 and 0.25..0.26: 211; over C7's band [0.128, 0.176] (b8949c96's lead (C), ironspec 22:09): at most 210 (0.13..0.136), day-1's cap input (the maximum over the band (C) delivers, not over the operating band) |

From 0.28 the maximum is 328..335 with restarts. The only failures in the restart sweep are the
catch band's hole (0.41, 0.42, 0.43: not out); HOME 0 lost, 0 escapes, 0 early pickups.

**The restart trap (found Nov 12 20:47; pre-official-load, every-tick restarts):** the every-6-gt
sweep above stepped over entry + 2 and + 3. A restart there drops the rider's path; its brain
recomputes it at + 4 with the cart in the activator's cell (1 high: the start falls back to the
AABB corner in the entry cell), so the path's first node is the entry, behind the one-way plate.
Ejected onto the chest, the napper presses toward it (vanilla's shouldTargetNextNodeInDirection,
which redsim follows, won't skip it from the chest) until the stuck detection drops the path, about
145 gt: 160 of 240 rows per tick (3 speeds x 10 orders x 8 seeds), entry to R up to 318 / 322;
every other restart tick +1, +4..+30 is clean. HOME, stamps (at most 39) and escapes are unchanged.
ironspec 20:53 (1): A6's cheapest fix (b) is owed (the ejected napper never targets a node it can't
reach, the plate stays), shown at every restart tick +1..+250, and the mechanism goes official
first (tick-exact). Lane C's cap stays on the no-restart maximum 210 meanwhile. A first candidate,
an open iron trapdoor over the activator (plate south), leaves 240 of 7,200 stalled: not enough.

## The operating entry band [0.11, 0.24] (ironspec 19:31; widened 21:25)

A6 is capped, not fixed: (C)'s entry bar is the measured [0.11, 0.24] in every row (leg speeds,
restarts, freezes, the rest +- 0.05, the release +- 1, contacts counted); an entry outside it is
(C)'s failure, and before such an entry counts the park sweep below is extended to cover it. Inside
the band A6 meets every bar at 0 blocks but the restart trap above. The known limits outside it:
- below x 1.12 the park loses a boost tick and the release stops short of the pickup (entries below
  about 0.11 park as low as 1.14: no margin);
- the catch hole 0.402..0.437 (and from 0.642);
- the chest wait from 0.28 (entry to R 328..335).

Park-north stays in reserve (a realistic -15..-20 gt, not -25; ironspec 19:31 (4)).

## The release from every park position (ironspec 20:31 (2))

The released cart starts from rest on the park's slope; the powered rails push only once it passes
0.01, so its top speed is quantized by boost ticks (0.06 each) and the pickup speed steps with the
park position: 0.143 -> 0.155 -> 0.204 as the park rises; below x 1.12 a park loses a tick and the
cart comes in at about 0.10, vanilla's pickup floor, and stops against the napper (the catch band's
hole). `--park-shifts` moves the parked cart along its rail before the release and prints the
seating and the pickup speed by park position against the band, 0.02 inside each edge ([0.120,
0.252] or from 0.331):

| rows | park positions | result |
|---|---|---|
| entries 0.11..0.21 every 0.005, restarts at every gt +1..+60 (through the parking) x 10 orders (100,800 visits) | the parks produced: x 1.28..1.64 | all seated at 0.144..0.206 |
| entries 0.11..0.24 every 0.005, restarts at every gt +1..+30 (through the parking) x 10 orders x the park moved -0.05 / 0 / +0.05 (194,400 visits; ironspec 20:16 (1), pre-official-load) | the parks produced +- 0.05: x 1.23..1.69 (48 positions) | all seated at 0.143..0.207: met (the band widened to [0.11, 0.24], ironspec 21:25) |
| entries 0.066 / 0.12 / 0.15 / 0.17 / 0.2, the parked cart moved -0.15..+0.45 every 0.01 | x 1.19..2.00 (139 positions) | all seated at 0.143..0.249; over 1.23..1.69 (the produced range +- 0.05) 0.143..0.206: met |

A booster before the pickup fails: it brings the cart in at 0.30..0.38, where it stops dead against
the standing napper (z 3.71) at some tick phases.

## On the real approach (ironspec 14:58-18:10, the boundary contract)

`mctas mobfarm bedtier-bench <the module> --module-offset 0,-27,-1 --queue-seat 9.5,-13.4375,-3.5
--queue-release 8,-14,-4 --declare-emulated 8,-15,-4 --declare-emulated 7,-15,-4 --declare-emulated
6,-15,-4 --declare-emulated 8,-14,-4 --declare-emulated 7,-14,-4 --declare-emulated 6,-14,-4
--queue-waits 20..59 [--restop OLD,NEW] [--skip-phases ..] [--forced-stamp] [--arrival-stamp
AGE --arrival-run --arrival-hold H]`: the module moved into the stop's frame, its lanes' and seat's
villager riders removed (the posts' mobs stay), the napper spawned at the queue seat at night and
released once the hold has passed and day-1's gate sees night (REST_l and NOT DAY, from day time
12,250: a napper picked before a skip is held until dusk); `--restop` brings the module's stop
(b36302af8's) up to this file (12 cells). The release is day-1's R1 (ironspec 18:10, b8949c96
17:58): repeaters at (8,-14,-4) and (7,-14,-4) on glass, their output toward the queue's rail,
driven by a lever on glass at (6,-14,-4) thrown at the release (the stand-in for day-1's gate
output); never a redstone block in (8,-14,-4) (it made (8,-13,-4) a second PANIC candidate at the
seat, the "walk targets taken while panicking -> [8,14,-3]" of the first rows below, and an off-pad
golem cell). The dwell rows (ironspec 17:16/17:52) inject the leg's state at the seat: a stamp armed
AGE gt before the spawn, HOME as the walk target (a held run, partial from the seat) and that run's
remaining hold H. Every row prints the stamp at the queue's release, at A6's entry, at every calm
REST tick and at dusk, the release-to-entry time, the escape row and the early-pickup bar.

The first rows below (on b8949c96's drop-and-roll WIP e9f0f4cbf, 8 seeds) ran with the redstone
block release and the old timing: provisional (ironspec 18:10), kept for the record. **Every
real-approach row in this section ran on e9f0f4cbf, the superseded (C): all PROVISIONAL (ironspec
22:46 (2b)); the approach rows that count run on C7's package (explore/farmopt-waycell4,
`f1q2-module-wayin-a6.txt`) with record7d's built R1 cells, once the water batch's waterlogged rails
are in redsim.**

| row | result |
|---|---|
| night, release phases 20..59, A6 as composed | 320 / 320 napped; 81 enter stamped (81..148 gt, armed on the second fall and the roll) |
| skips at every gt 20..360 from the spawn, phases 20/33/46/59, A6 as composed (the failing baseline) | 835 of 2,728 lose HOME at dusk (skips from release - 26 to release + 229; 80 of 160 per 20-gt bin up to + 159, 0 from + 240); before the night gate, so the pre-release ones are void |
| the forced-stamp control (a stamp armed at the entry, a skip at once) | 22 / 22 lose HOME at dusk |
| night, the panic held until the cart is on the roll (it ends at release + 70..110), A6 as composed | 56 of 72 enter stamped (the path from the roll can't pass the activator); with (3,4,-4) opened: 56 of 72 |
| the same with the second landing | 72 / 72 unstamped (stamp at most 4) |
| night, release phases 20..59, with the second landing | 320 / 320, HOME 0 lost, 0 escapes; 27 enter stamped (the airborne second fall: a path computed in the air is partial whatever A6 does; (C) replaces the fall) |
| escape control: the lever east of the chest on glass at (4,3,-3), no glass over it | 205 of 320 escape (walk east off the chests along stale paths and fall) |
| restarts +30..+300 every 5 gt x 10 orders, with the second landing | 1,100 / 1,100, HOME 0 lost, 0 escapes |
| skips every 2 gt 20..360, phases 20/40/59, behind the night gate: A6 as composed / with the second landing (the fall still in) | 204 / 99 of 1,026 lose HOME (the fall's stamps), 0 escapes |

With day-1's R1 release and both stop cuts (the release at the wake edge + 0, F at lie-down + 1),
A6 with the second landing (e9f0f4cbf, 8 seeds; release to A6's entry 114 gt):

| row | result |
|---|---|
| night, release phases 20..59 | 320 / 320, HOME 0 lost, 0 escapes, 0 early pickups; 46 enter stamped (the airborne fall) |
| restarts +30..+300 every 5 gt x 10 orders | 1,100 / 1,100 HOME, 0 escapes |
| skips every 2 gt 20..360, phases 20/40/59 (the fall still in) | 164 of 1,026 lose HOME (the way-in's stamps), 0 escapes |
| escape control (the lever east on glass at (4,3,-3), nothing over it) | 223 of 320 escape |
| forced-stamp control | 22 / 22 lose HOME at dusk |

The contract run on (C) (the rail descent, ironspec 16:20) with the second landing follows its lay:
0 HOME lost over every skip tick and release phase, the forced control biting; and the dwell rows
(ironspec 17:16/17:52): the queue releasing at dwell 0, 34 and 50 with the leg's state injected
(b8949c96's measures: the stamp 78..387 gt old at the arrival, the held run dropped 88..160 gt after
it), a skip at every tick from the arrival to the first calm REST tick on the ledge, 0 HOME lost,
entry to R inside its budget, cross-checked against b8949c96's whole-visit chain.

## The stand-ins the benches write (ironspec 17:46/18:34/20:31)

Every emulated write is checked before the run against the built cell (`emulated_write_check`,
`standin_reach_check`, `survival_check`): golem and path properties through redsim's own predicates
(legacy-solid, a full top face, full-block collision, solid-render, golem-excluded, air, signal
source, rail, dangerous, any collider, fluid: G1's CHANGES 4) equal, unless the cell is declared
(`--declare-emulated`); update reach (the face neighbours a state change notifies, a lever's
attached block's, a repeater's front block's) with no undeclared rail T; power reach (a lever's six
neighbours and attached block, a repeater's front, through a conductor (redsim's
`isRedstoneConductor`) its neighbours) inside the built input's reach, any other stand-in kind
refused; survival (every laid block and every stand-in `can_survive`s where it stands, no rail
`would_pop`), or the run fails. Late-path is left to 9fc3155f's statics on the declared cells. The
stand-ins:

| stand-in | cells | golem / path | update reach | power reach |
|---|---|---|---|---|
| the controller's levers (hatch, activator, park): `ctl_block` | (-4,0,3), (-4,1,5), (2,2,-5), (1,1,-5), (0,0,-5) | the built levers themselves, only their powered state toggled: equal | the built lever's; no rail T (the stop has none) | the built lever's own (identical by construction) |
| the queue's release, day-1's R1 (with --module-offset) | repeaters (8,-14,-4), (7,-14,-4); the driver lever (6,-14,-4); glass under the three | all six declared until day-1's R1 lands in the module (the repeaters and the lever differ from the air they replace in air and signal source; the glass in legacy-solid, full top, collision; golem-excluded and not solid-render: no golem cell, no PANIC candidate) | the repeaters' fronts (the next repeater, the queue's rail) and their neighbours; no rail T | the lever: the back repeater only (its other neighbours air); each repeater: the next one, the front one the queue's rail |

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed. Not bridged; the
  design must not rely on it.

## The curve contact's fix (ironspec 23:19 (4b); adopted 00:32)

A cart picking up near 0.154 b/t caught the glass east of the pickup cell, (-1,0,4), with its box's
east face as it turned the curve (-2,0,5) east (0.148 -> 0.104 b/t). The band sweep with contacts
counted (entries 0.11..0.24 every 0.005 x restarts +1..+30 every gt x 10 orders x park -0.05 / 0 /
+0.05 x 8 seeds, 194,400 visits; bench, tool unreviewed, pre-official-load): the glass, 14,400 visits
with the contact (entries 0.14..0.165); the fix, a closed top-half iron trapdoor there
(`iron_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false]`, collision
0.8125..1.0, above the cart's top at 0.7625; nothing powers it), 194,400 clean, 0 contacts, the park
bar met, pickups 0.143..0.207, HOME lost 0. The napper on the pickup cell still can't step east (its
box 0..1.95 meets the slab, glass over it). The statics' summaries (golem flood, push-scan,
flee-check, sight) are the same as the glass's. It is A6's design now; `--curve-glass` is the
contact's positive control. record7e (onechunk, 1336ad6c3) carries it at (-1,-27,3).

## The shared stand-in guard and record-match (ironspec 22:37 / 23:13 / 23:46)

The bench's emulated writes now go through `recordmatch::guard_intrinsics` (the one shared
definition: golem in both roles, the full path classification, late-path, sight), its update reach,
power reach and survival staying this bench's own checks. `--record FILE` prints record-match's line
for the bench's world (the visit `--from` at the first speed, after every bench write, the declared
levers' night states aside) and its verdicts fail the run; `--record-declare` adds declaration lines;
on a module, its `emulated:` cells are laid and declared as a candidate. record7's line-bench levers:
`--lever-on` (REST_l), `--built-release` (NOT-ready: off at the release, on again at the departure;
no R1 stand-ins), `--day-lever` (DAY written only at game time % 20). Every fluid cell's tick is
scheduled at load (the count printed); every bar prints its samples (0: NOT JUDGED, a failure unless
`--scope-excludes`); riderless carts out > 0 fails; the PATH right after every load is printed.

## (A), the eject within SleepInBed's reach: parked (ironspec 00:07 (4A) -> 02:18)

`mctas mobfarm bedtier --eject-at-bed`: the flats along x -2 raised to y 1, the arming detector
(-2,1,1), the activator E (-2,1,2) over the bed foot's north-east corner, the park one braked powered
rail ascending north (-2,0,3), the pickup cell P (-2,0,4) as A6's; the landing chests, the ledge and
the drop column gone. Findings (bench, tool unreviewed, A6 alone):

- **The dismount.** Height 0's SW (the cell over the bed foot) is no floor: the fallback from the
  cell below counts only a full-height block (the bed's top is 0.5625). So every place at E's level
  and the one above is made invalid (five glass cells round E at y 1..2, glass over the park's
  trapdoor instead of A6-style's golem-proofing button, which it replaces in that role too), and the
  first valid place is height -1's SW, the bed foot: 28/28 riders set on it, asleep the same tick
  (SleepInBed: 1.00 from the bed head's centre; startSleeping erases WALK_TARGET and CANT_REACH).
- **The boarding cooldown.** A rider dismounted at E can't board any cart for 60 gt
  (`Entity.boardingCooldown`, set on stopRiding; A6's 85+ gt walk never met it). So the gate is F =
  the lie-down + 2 AND the arming edge + 70 (the cooldown less the park -> P travel, plus the
  detector -> E time and a margin), the release the wake edge (no release without a wake: a stranded
  napper keeps the cart parked; the napper sleeps through the delay, awake at P 3..5 gt). With delay
  0 (the positive control) the cart is moving on the release tick or reaches P inside the cooldown and
  stops against the napper: 8/8 not out. With 70: the cart at rest on every release tick, the pickup
  >= dismount + 67, 28/28 seated. **Entry -> R 146..185** against A6's 154..183: the cooldown
  replaces the walk, so no cap gain.
- **The 0.2-step clip.** A minecart's own move collides with a pushable entity's box (vanilla
  `canVehicleCollide`): a cart stepping more than ~0.2 per gt can carry its front past the standing
  napper's back face in one tick, the move clips it there and zeroes its velocity, and the pickup
  that follows needs > 0.1 b/t: stopped dead, the napper stranded. A6's seven flats bring the cart
  under 0.2 before P. (A)'s one-cell park gives pickups of 0.145 / 0.214 / 0.284 by park position, and
  the park sweep (every 0.01, z 2.91..3.71) has a failure gap at z 3.23..3.25 between nominal parks at
  3.22 and 3.27: a robust FAIL. Fixes (a run-up of 3..4 flats after the park, a saturated uniform
  park, a non-boosting release) are not priced: (A) is parked behind B1.
- **B2's IDLE clearer.** LandRandomPos can't pick the bed foot (the bed is legacy-solid, its raw
  sample fails `isNotStable`); a reachable stable own cell beside the bed (P with a solid-render
  block under it) clears by dusk with P ~33% only (10 samples of 6,615 per attempt, ~260 attempts in
  a 12,000-gt day); a block-only yard needs K >= ~41 reachable stable cells for P(no clear) <= 1e-7.
  Not priced (02:18).

## (A) with the run-up for stack (b+): Z1, the exit boost and the slope hold (Nov 13)

The design of record for (b+) (ironspec 08:53 / 09:38): `mctas mobfarm bedtier --eject-at-bed --runup
--no-loop-booster --exit-south --waker-south --exit-boost --hold-slope` (local frame; loop = local +
(0,-27,-1)). Bench, tool review pending (G1); contact figures "(shared contact rule on ironfarm's
branch, unlanded)" until `cart_contacts` (329b5acc3) is on main.

- **The run-up's leftovers fixed.** E's floor glass with a top-half iron-trapdoor cap (the stand-up
  competitor gone); top-half trapdoors at (-1,0,2) (K's turn met glass) and (-1,0,5) (the run-up's
  water leaked out through it); the park drive's conductor (2,0,4) SLIME, not smooth stone (a
  solid-render floor made the U-turn's rail cell a stable flee landing: 2 of 20 wakes armed).
- **Z1, the waker moved beyond the flee's 6.** A zombie in a boat at (-5,0,10) (feet 6.73 from P's, 6.43
  from the sleeper's) under glass and persistent leaves (eye-cell sky light 14), behind a top-half
  iron-trapdoor shutter (-5,1,9): powered (the night state) it cuts every line; F unpowers it and the
  sleeper's eye (-2.50,0.76,4.48) sees the zombie's eye (-4.50,1.23,10.50), 6.36 apart, through the
  channel (-3,0,5), (-4,0..1,5..8); the window shuts again on the wake edge. (-4,0,5), A6's old hatch
  wall, cut that line at its corner (no wake on 6 of 6) and is air. Screens (-3,1,6), (-3,1,7),
  (-2,1,7), (-1,1,7) keep the exit out of sight. (Superseded the same afternoon: the record leaves
  (-5,1,8) empty for F's built line and stores the shutter unpowered; see the next section.)
- **The exit boost.** A booster on (-2,0,5) right after P, powered through a slime support by a torch
  at (-2,-2,5): the exit's crawl (carts leaving the hand-off at ~0.07) was the whole slow tail of
  entry -> R (166..203 before, 141..170 after).
- **The hand-off contact,** declared on the block it touches: the bed head's east face (-3,0,4), flush
  (depth 0.0000), one tick, 0.103 -> 0.069..0.075, the bed unchanged across it.
- **The slope hold.** P is a curve and a powered rail is straight only; a flat braking rail can't
  restart a cart at rest without a conductor at an end. The exit slope (1,-1,5) can: an unpowered
  powered rail brakes the loaded cart to rest, the lane's tap powers it and the slope's gravity
  starts it (no kick block). Its support (1,-2,5) is slime, DAY-1's built tap cell (fed by its
  repeater at (1,-2,4)); 7.8 from Z1 (beyond the flee's 6), the rider screened from it in both
  shutter states. A passenger can't start SleepInBed, so the wait aboard is safe from the re-sleep
  lock (LAST_WOKEN's 100 gt; P is 1.09 from HOME's centre, inside SleepInBed's 2.0).
- **Judged runs (b38/b39, every per-visit problem failing the run):** 0.05..0.55 x 8 seeds, 408/408
  clean, exit 0 (contacts only the declared one; waker 0 ticks within 6 with sight or memory;
  NEAREST_HOSTILE's clear 19 gt after the window's close on every wake; re-sleeps 0, the longest wake
  -> boarding 20 gt); the dense 0.38..0.44 (0.005) + 0.4075 x 8, 112/112 (the cart's speed at the
  napper contact >= 0.1638 against the pickup's 0.1; park x 1.614..1.698); the hold, with delta = 5 at
  the park and the tap at one shared tick, 0.05..0.55 x 8, 408/408 (every cart at rest on the slope,
  creep <= 0.0083, held 93..122 gt, departure -> R 13..15 gt, 0 NEAREST_HOSTILE ticks or stamps over
  the hold). "(free-running tap: waits >= 100 launch early: not yet judged)" on the hold until the
  periodic-pulse row runs.
- **A known failure class outside the operating range:** entry 0.56 parks the cart at x 1.432, the
  release reaches the napper at 0.0955 (< 0.1: no pickup), the cart stalls against it and the napper
  re-sleeps at the wake + 100 (8/8 seeds; 0.55 and 0.57..0.60 pass). Standing condition (ironspec
  08:53): any change that could lift the entry above 0.50 (the landing booster's power, the slope, the
  cart's speed into the landing) widens the park's catch first and re-sweeps to 0.60.

## The rebased tip: main's contact rule, the wake sensor, the tap's built path (Nov 13 afternoon)

Rebased onto main f5dd4cf87 (train 10; first onto 1ea7bf39a, train 8: the shared contact rule). Rows
cited "on 2af606b9d" ran before the second rebase (2af606b9d is not an ancestor of the tip): they
are re-run on the tip before they're cited; the contact figures below come from e25e3b01c (an
ancestor). The controller-dependent rows before G1's B1 ran on the bench's named stand-ins (and the
pre-tick writes stamped one tick late, B2): they carry that label until re-run on the adopted
emulation.

- **Contacts judged by main's ONE bounded judge** (G1's Q1 / Q6; ironspec Nov 13 10:42 (3) / 10:58;
  the 84dff165a copy and my per-cell parsers gone): every visit's hits (`contacts::cart_hits` +
  `cart_vertical_hits`, the cell in the design frame, each visit's cart its own sample) and motion into
  `contacts::judge` (undeclared by cell and face; ticks, events, depth, speed bounds; NO STALL by
  IRONSPEC_1042; STALE for a declaration never met); declarations by
  `declared_contacts_bounded`; `--reference-check` makes `contacts::reference_check` fail the run. The
  bed-head declaration in the bounded grammar (`face=east | ticks<=1 | events<=1 | depth | speed_out
  | ref=`), its values from the reference run on main's judge (ref e25e3b01c,
  774111605's code) at 21 build positions (the design frame, ironspec's P1-P4 as written and on the
  column convention, P2 at loop + (6,0,6) and + (6,24,6), every copy A-D at loop + 0 and + 80): the
  same extremes at each, 296 events over 296 of 408 visits (37 of 51 entry speeds, every seed of
  each), 1 tick, at most 1 per cart, the deepest 0.0440 (main's depth is the clipped part of the
  cart's westward request at P's turn; the boxes stay flush), speed_out 0.0336..0.0808 (the contact
  tick's returned move). Each eps is one unit of the stated fourth decimal: the extremes' spread is 0
  over the 21 positions and 2,040 visits of seeds 1-40. The carry threshold 0.0101 (`--boost-probe`:
  the cart's velocity set on (-2,0,5)) is another measure, a floor the speed_out bound doesn't lean
  on. Controls: `--reference-check` passes on the band; editing the depth bound to 0.0400 fails 24
  events. **Judged per entry speed** (G1's B3, ironspec 16:38): the contact is decided by the entry
  speed (the park's rest position), not by chance, so no binomial rate: `# contact-speeds:` names the
  37 meeting speeds of the 0.05..0.55 / 0.01 band; each listed speed must meet the face on every seed,
  each other speed of the band never; a speed off the band prints, unjudged.
- **The wake sensor** (`--wake-sensor`, G1 12:04 / day-1 12:03): an observer at (-3,-1,4) facing up at
  the bed head, in the glass's place; OCCUPIED's set and clear each give a pulse into (-3,-2,4) 2 gt
  after the edge (tick-exact confirmed in 26.3). On every one of 408 lie-downs and 408 wakes it rose at
  +2, none missed, none spurious. DAY-1's F line keys on it: SLEPT (toggled by its pulses, reset at the
  release), OPEN on (ARM + 70) and SLEPT, the shutter = NOT OPEN, the release behind OPEN's fall.
- **The record holds no stand-in:** (-5,1,8) (F's line output) and (3,0,4) (the park drive's input)
  empty for DAY-1's cells; Z1's shutter stored unpowered, its one stable state with no drive (Q16: the
  generator fails on a trapdoor that changes at its next update). The bench's file lays the stand-ins
  as declared overrides.
- **The slope hold's tap on its built path** (`--tap T,W[,PERIOD]`, Q2): the bench file's stand-in
  lever (1,-2,3) into DAY-1's repeater (1,-2,4) facing north, into the slime support (1,-2,5), through
  the stand-in guard (its reach: the repeater alone). A PERIOD makes it a free-running train at phase
  T. Bars: an arrival during a pulse, a wait of a full period, a CLEAR line to Z1 in either shutter
  state, departure -> R over the budget's 15 gt (Q3). The retired `--pulse-cell 1,-1,6` is refused.
- **The design point:** the window shut at the wake + 4 (DAY-1's observer 2 + torch 2), the release
  at the wake + 8 (`--close-after 4 --release-after 8`). 0.05..0.55 x 8 seeds, 408 / 408 clean: ARM
  -> lie-down 4..5 gt (every lie-down after ARM), ARM -> wake 71..90, the window's opening -> wake
  1..20, release -> boarding 13..19, NEAREST_HOSTILE's clear 16 gt after the close on every wake, the
  first (-2,0,5) entry clears the close + 27 by 1 gt at least (entity-to-entity, ironspec 14:08), 0
  re-sleeps, the longest wake -> boarding 27. close-after 8 / 10 with delta = close + 4 also 408 / 408.
- **The slope hold at the design's waits** (W 10, period 100, three phases: held 16..45 / 36..65 /
  56..85; single late pulses, the RETURN gate's held launches: held 166..195 / 186..215): every bar
  passes (0 arrivals during a pulse, creep 0.0083, 0 NEAREST_HOSTILE ticks, 0 stamps, 0 CLEAR lines,
  departure -> R 13..15). Controls fail: W = 4 (carts stuck), a pulse over the arrivals, a planted
  re-shape of (-1,0,3) (integrity), `--release-after-arm 1` (at rest).
- **Frame independence** (`--offset`, the whole stop built at design + offset): the dense band at six
  generic positions (+80 / -40 in y, (16384,0,16384), the chunk phase (+6,0,+6), their sum) and at
  every module copy of onechunk's N = 4 (A..D, each also +80): every judged outcome identical, the
  least speed out 0.04678174225627934 at 0.395 at all 14 (the old after-tick speed, before main's
  judge); offset 0 byte-identical to the code before.
- **The mutual hold (-1,0,3) <-> (-1,0,4):** both `holds ...=east_west` in `integrity:`, judged every
  tick (Q13); tick-exact's both-orders build replay keeps both east_west in the game.
- **(superseded latch, before 15:37) DAY-1's latch emulated** (`--latch T_MAX,LOGIC_LAT,REL_AFTER` on the real sensor) and the merge
  row (`--hurt-after-lie k`): a hurt within 3 gt of the lie-down merges into one pulse; the napper
  stands awake on P, re-sleeps in the bed at the wake + ~100 (LAST_WOKEN), its pulse resets OPEN and
  the superseded latch released with it asleep (12..16 of 16 forced visits; 0 / 408 in normal
  operation; the adopted controller's merged row replaces this). The
  P-eye static: from 25 eye positions on P, Z1 is in view from 0 with the window open (the screens
  (-3,1,5..6)), so an awake napper on P never re-acquires it. Tick-exact: in 26.3 a cart at P does
  NOT seat a sleeper in this bed (9 / 9), so the awake guard is bought (DAY-1 places it: a string at P
  + 1). A never-slept napper (by day) stays on the bed foot (-3,0,3) the whole visit, never on P.
  (Tick-exact 15:46, later: the game DOES seat a sleeper that drifted after a walked re-sleep, the
  merged path's; ironspec 15:48: a seated sleeper is a normal departure, a centred one refused is the
  permanent halt the STALE guard prevents.)

## The controller's terms, RS_ready and the 16-ball (Nov 13 evening)

- **The stop's controller in the bench** (G1's B1): by default DAY-1's gate list rev 20:00 (artifact
  1608254923947368, its stop block G16-G22) emulated on the real cells (`--controller adopted`,
  `--adopted-terms PARKED,A70,D,W,OPEN_SET,OPEN_RESET,TORCH,RLS,PARK,LAG,ONSET`, default
  52,0,12,54,6,6,2,6,2,10,1): SO set by any observer pulse, reset at RLS; SINCE set by a pulse with
  P_STRING on at its head change and (G1's B2 onset, day-1 17:35) the press begun within 1 gt of that
  change, reset on NOT P_STRING; WIN from the pulse + 12 (RLS's sample) to a new pulse or the W line
  48 (G17: the latest RLS at the pulse + 59; the term 54 = 59 - RLS's 6 + 1); PARKED = SENSED (the
  detector rail at (0,0,3) pressed after ARM, through d = 28; ironspec 17:35 / 17:39 / 18:36) AND
  COOL (ARM + 52, ironspec 17:53); OPEN on PARKED and SO and not P_STRING (ironspec 17:32 (4)), reset
  on RLS or P_STRING; RLS = P_STRING and SINCE and WIN and PARKED, 6 gt of logic after; the park at
  RLS + p, p = 2 (ironspec 17:53); the bench's release drive stays on (G21's >= 8-gt stretch is
  covered). P_STRING is a NAMED STAND-IN (the awake napper's box meeting the band at (-2,1,4)), read
  at each iteration's start, before any planted write (G1's C2). `--controller stand-in` keeps the
  old terms, labelled; its own flags (`--release-after-arm`, `--early-release`,
  `--no-presence-gate`) are refused under the adopted controller (G1's C3); `--latch` is the
  superseded 12:38 latch. Every run prints its command line.
- **The stop's bars on the adopted controller** (ironspec 17:32-17:53 / 20:55, G1 17:33-17:49 and
  CHANGES 7), each printed per run: the first (-2,0,5) entry - (C + 19) >= 1 after k + j (C = the
  closing write's stamp, t - 1 for a pre-tick write (ironspec 20:55 on G1's B2); the shutter's first
  shut end-of-tick printed beside it; tick-exact 17:52: NEAREST_HOSTILE's worst clear; "close + 27"
  retired and printed beside it); (the cart's first tick in P) - (the napper's boarding-cooldown
  end, 60 gt from the eject's dismount) >= 2 after k + j; COOL's measured inputs; the park's rest
  against COOL's timer (a rest after it fails, G1's C4); the sensed park's pass -> rest per speed
  (SENSED's d >= the worst + 2, judged; a cell without a detector rail is read by the labelled
  stand-in) and its presses (only the arriving empty cart; none after RLS); the window's opening -
  the lie-down >= 4 (G1 17:40); the window open on a napper awake on P <= 9 gt (rev 20:00 G20); no
  ridden cart on the activator (-1,0,3) (G1 19:26; the planted `--plant-e-off` row fails it by name,
  with the detector press, the activator's power edge and the passenger change printed by tick). k =
  1 on the convention placement, 3 off it; j = 0 (the stop in one chunk). The merged path is a hurt
  inside the merge window (`--hurt-after-lie k`, k <= 3); a later hurt keeps the stranded bound;
  only the blocked-P row may end in bed; a release with the napper asleep is never waived.
- **The sensed park** (`bedtier --park-sense`): a detector rail at (0,0,3), the last straight cell the
  cart passes into the park that touches no powered rail ((1,0,3), the climb, sits beside the park's
  powered rail). Its power reach: the inert glass under it, the activator rail (-1,0,3) (only the
  empty cart rides it, after E's eject: the detector fires on the cart's box while it may still be on
  the activator, which ejects nobody from an empty cart), the plain rails (1,0,3) and (0,0,4), the
  air (0,1,3) and SENSED's read (0,0,2). Its updates reach the T (1,0,3), which re-settles to its own
  shape (update-only; declared in `switches:` with both states ascending_east, ironspec 20:33); the
  integrity line keys one entry per arm (P, (-1,0,3), (1,0,4)); the old mutual hold (-1,0,3) <->
  (-1,0,4) (9fc3155f 17:49) is broken by (-1,0,3) being an activator rail (straight-only; G1 19:19:
  not at (-1,0,4), where the loaded cart sits at the pickup). (-1,0,3) is not a T.
- **The fault notes from the stop's rows:** E-off (`--plant-e-off`): with the eject's torch lost the
  napper rides on and the park-entry activator (-1,0,3) throws it out beside the park; it wanders off
  (16 / 16). The empty departure (ironspec 20:34, adopted): the stepped-off napper keeps its bed and
  re-sleeps at its next rest; the held next napper loses HOME (cc767b78's composed night gives N).
- **The power-up row** (ironspec 21:10 (3) / 21:11; `--park-initial-on reset|keep`): P_HOLD's latch
  come up SET, the park drive on from the first tick; `reset` clears it at ARM's edge (DAY-1 21:10's
  reset, emulated), `keep` is the control; the first arrival's brake and ARM's edge against the
  stretch are printed.
- **The hold's launch** (G1 20:27): with `--tap T,W,PERIOD` each held cart is judged against its own
  next pulse; `--tap-skip K` leaves one pulse out (N13's planted missed launch: the full-period bar
  must fail); the single-tap long-wait row is its own row (the hold's stability). The cart's speed
  entering R is printed, ridden and for empty departures (ironspec 20:45).
- **The rider-keyed RS_ready** (`bedtier --rs-ready`; ironspec 15:23, day-1 15:29): a tripwire string
  at (4,0,5) one cell over R's detector, hooks (4,0,4) facing south / (4,0,6) facing north on glass
  (4,0,3) / (4,0,7), off the rail line; a seated rider's box presses its band (y 0.0625..0.15625, the
  attached string's shape), an empty cart's top (y -0.24) never reaches it. DAY-1 reads the north
  hook by a repeater at (5,0,4) on (5,-1,4) (its cells). `bedtier-bench` judges it per visit: the
  riding napper presses it on every seated visit, nothing else ever does, the rider's band overlap
  and the cart's gap under the band, its press - R's detector rise (d_RS's new reference).
- **The stale press** (ironspec 15:46 (2)): every run prints the napper's least gap to P_STRING's band
  (-2,1,4) over the 12 gt before each lie-down; the eject's set-down lies down in its own tick, the
  ridden cart north of P at K.
- **Check 4's 16-ball** (G1 16:00: an awake HOME-holder within 16 of an occupied bed loses HOME): the
  stop's own cells all sit within 11.1 of the bed head but are occupied only while the bed is free
  (RS_ready / R1 hold the next napper until the previous one boards); the held next napper waits at
  onechunk's queue seat (9,13,-3) local, 18.9 away. Every run prints the boarded rider's distance at R
  and at its last bench tick (the route beyond R is composed).
- **R4's planter** (G1 15:29 / b8949c96): `--cart-states FILE` writes the cart's states near the
  top-half trapdoors (`x,y,z,vx,vy,vz,rider`, the design frame).

## The re-laid stop (Nov 13-14, ironspec 21:42 -> 00:08)

`mctas mobfarm bedtier [--bench] --relaid 1 --seal-p ...` on the flags of record. The files:
`relaid/bed-tier-relaid.txt` (+ `-bench.txt`), and `relaid/bed-tier-relaid-latch.txt` (+ `-bench.txt`)
with day-1's latch laid (`--latch-b`); the bench rows below are redsim, the 408 band (0.05..0.55/0.01 x 8
seeds), terms p 2 / W term 54 / PARK 24 gt (the defaults), path search 48 / 768 (Villager's requiredPathLength
48; tick-exact's 00:21 16 / 256 pin withdrawn).

- **The layout:** P (-2,0,4) straight north_south, dry, reached from the north through K (-2,0,3), a
  driven T (north_east for the arrival from E, south_east for the return; its drive's conductor the
  slime (-2,-1,3), the bench's stand-in a ceiling lever (-2,-2,3); K's own latch is DAY-1's: SET by
  ARM's edge, RESET by the park string's rise). The empty cart's out-and-back siding along z 3: K,
  (-1,0,3), (0,0,3) plain and dry, the park (1,0,3) a braked powered rail ascending east, its drive
  the plain repeater (2,0,2) (FACING north: input (2,0,1)) into the slime (2,0,3). SENSED: the
  rest-keyed tripwire over the park (1,1,3), hooks (1,1,2) / (1,1,4) on glass (1,1,1) / (1,1,5).
  P_STRING east-west at (-2,1,4), hooks (-3,1,4) on (-4,1,4) and (-1,1,4) on new glass (0,1,4).
  (-1,0,4) and (-1,2,4) glass. No head-top guard (ironspec 23:40 (3)(a): it takes the bed head
  node's headroom, so no HOME path could complete).
- **Water: none** (ironspec 23:40 (1)). (-1,0,3) wet seated 32 / 408 (first contacts 0.044-0.122);
  P wet stalled 144 loaded carts in P's water; the px 2 booster gave 0.234-0.272 and a 19-22 gt ride.
  All dry: 408 / 408 seated, first contact 0.1285-0.2712 (the [0.13, 0.2] band retired as a proxy;
  the mechanism bars: least first contact >= 0.12 (pickup gate >= 0.1 inclusive at the cart's tick
  end, tick-exact 23:46); pushes of the napper before seating 0 / 408), the park -> P ride 14-18 gt,
  the arrival rests in the park cell on every visit (x 1.18-1.61, the climb's top at 0.62 of the
  cell), K's bars pass (north_east >= 27 gt before the arrival; south_east >= 7 after its last tick in
  K, >= 74 before the return).
- **The boarding cooldown** (tick-exact 23:52: every dismount, the ARM eject included, sets 60):
  eject -> pickup least 93 gt (margin 33; bar >= 65).
- **The departing cart re-presses the park string** on the out-and-back siding: the emulation's
  SENSED counts a press only between ARM and the release (SET gated by ARMED); day-1's netlist
  carries the gate or ARM's edge as SENSED's reset (ironspec 23:40 (2)).
- **The west bed sensor** (`--bed-sensor-west`, ironspec 23:34): the observer at (-4,0,4) facing east,
  glass under the head: both bed halves on glass (no LandRandomPos isNotStable draw). Its condition:
  the observer pulse ticks per visit identical to the sensor under the head: 816 pulses and every
  lie-down / wake line, an empty diff. It doesn't change the stamps (HOME paths) and buys nothing on
  9fc3155f's stroll / stamp classes: it LEFT (ironspec 01:05): the record keeps the observer under the
  head and day-1's wake line unchanged; the flag stays in the tool as an option.
- **The stamp class** (CANT_REACH_WALK_TARGET_SINCE; ironspec 23:53 (3)): HOME paths have accuracy 0
  (they complete only ending ON the bed head's node). Today's fold completes them through a 13-node
  walk-up via (-1,0,4) / (-1,0,5) (vcompose's route, the walk-off itself); the re-laid stop seals it, so
  the exit riders ((-2,0,5) on) arm: the waker's stamp after the wake 366 / 408, every held rider at
  the slope's departure. The siding riders complete on every visit by an entry-side route that was
  there already: (-1,0,3) -> (-1,0,2) -> E -> ARM -> (-3,1,1) -> (-3,2,2) -> the foot cap (-3,2,3) ->
  a drop onto the bed head's node (8 nodes): R1 (`--stamp-route`, three glass steps over the roof)
  buys nothing and leaves. Its first step crosses E's east trapdoor top under the lone guard string
  (-1,1,2): no hooks, so nothing reads a press, and no walker treads it (the riders stay seated): the
  `unreachable:` declaration is reworded to say so (9fc3155f 01:01, 58f770cc 01:02). The west sensor
  buys nothing either (9fc3155f 01:02: the stamp classes identical, arms-and-clears 28,
  arms-never-clears 2 at (1,-1,5) / (1,1,-4), never-arms 4) and leaves. The bar is arms-and-clears with
  age < 1,200 judged on the composed night (cc767b78), farm-wide (9fc3155f's place static).
- **Seal-P** (`--seal-p`, ironspec 00:05 / 00:08; REQUIRED by 9fc3155f's closure, 01:01): the exit's
  first cap (-2,1,5) glass instead of the iron trapdoor, because the pathfinder routes through the
  trapdoor cap (26.3's pathfinder treats every trapdoor as passable, malus 0, though a villager may
  physically only press into a top-half cap): with the trapdoor P's closure includes the exit channel,
  with the glass it is {P}. On the 408 band: 408 / 408 seated, no hurt, pushes 0, eject -> pickup least
  93. Its riders-eye pin (isInWall) is tick-exact's. The riders' HOME route over the foot's cap
  (-3,2,3) clears stamps on the pathfinder's completion; "no climb-back" rests on the bed head proved
  unreachable from the exit cells and from P (9fc3155f 01:01).
- **The no-HOME rows** (`--erase-home-on-p K`, `--erase-home-ticket free|taken`,
  `--erase-home-day-time`): night, HOME erased on P: the napper stays on P (today's fold 44 / 44;
  re-laid with K stuck, 24 / 24 over ~2,300 gt). AK (tick-exact 00:07: day, IDLE, ticket free, a
  stroll toward an unreachable target whose partial path ends on the bed tops) isn't reproduced:
  the class waits for cc767b78's replay of AK's recordings. A HOME-less napper arriving with the
  ticket free claims the bed and naps (44 / 44; its lie-down ARM + 4..82).
- **Day-1's form-(b) latch** (`--latch-b`, ironspec 23:45): rig (x,0,z) -> stop (x+2,-2,z-2) on glass
  at y -3; the OR line climbs (0,-2,1) -> (1,-1,1) -> (2,0,1) into the drive repeater; Q's tap line
  along x 8 and z 2 into the slope's tap repeater (1,-2,4); RLS_IN (-1,-2,-2) and ARM_IN (6,-2,-5)
  the logic block's inputs (the bench's floor levers: RLS held `--latch-rls-width`, ARM 3 gt).
