# The bed stop (the lead's napper stop: F1 + Q2, one bed, 24 villagers)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-two`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), farmopt (ca87cf44), G1 (8146b8db), day-1
(a75cc97c). Placed under the station, stop 9 down (ironspec 12:23): loop = local + (0,-27,-1).

## Files

- `bed-tier-fold.txt`: the stop, design A6 (ironspec 10:46-16:29). 17.2 build minutes at middle
  prices (`mctas mobfarm bill`), the leaves' shears, the two landing chests and the trapdoor's iron
  included. Not built:
  F and the release from the bed observer (the controller's: day-1's line ends at the hatch's two
  levers and the park's two; the bench's levers are stand-ins, so onechunk's verify re-runs on
  day-1's real line ends when laid). The way-in (b8949c96) ends at the entry booster.
- `bed-tier-fold-bench.txt`: the same with a run-out past R and the two-visit bench's queue rail.
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
  levers on) and hide the zombie; F (the lie-down + 10, the bed observer's edge) shuts them flat
  (levers off): the zombie (persistent, in a boat at its post (-4.5,0,4.5), leaves over it) wakes
  it, it stands up on the pickup cell, the park releases 30 gt after the wake edge, the cart seats
  it (0.143..0.207 b/t in every bench row) and runs on to R, whose pulse opens the hatch again.
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

- **Entry speed 0.02-0.62 b/t** heading west at the entry booster (all seated). From 0.65 the cart
  brakes lower on the park and the released cart stops 0.5 short of the pickup cell.
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
| two visits, restarts every gt over +1..+321 x 10 orders | 77,064 / 77,064 | A within 17 at C's lie-down 0; A's R to C's lie-down 122..361 gt |
| two visits, A at day time 0..80, C queued | 192 / 216 | HOME losses by place: 0 rows in the stop's box, 24 only outside it (C at the bench's queue cell, at 12,013, before its entry) |
| two visits, a skip in A's visit (every 10 gt) | 178 / 1,032 by HOME | 653 rows lose HOME only outside the stop (C on the bench's queue cell through the day, at 12,013..12,034, before its entry); 201 in the stop's box: C, teleported from the bench's queue with a stamp armed there (no post). Void (ironspec 15:17): the bench queue's; the real-queue rows (next section) replace it |
| control `--no-presence-gate --empty` | 24 out | the empty cart leaves |
| control `--trapdoor-caps`, restarts +90..+180 every 4 gt x 10 orders | 5,520 / 5,520 | no path off the pickup cell after a restart in A6, so the contact mode no longer shows |

The two-visit rows still run on the old corner queue's cell and transit; the real queue's rows are
the next section's.

## On the real approach (ironspec 14:58-16:29, the boundary contract)

`mctas mobfarm bedtier-bench <the module> --module-offset 0,-27,-1 --queue-seat 9.5,-13.4375,-3.5
--queue-release 8,-14,-4 --queue-waits 20..59 [--restop OLD,NEW] [--skip-phases ..] [--forced-stamp]`:
the module moved into the stop's frame, its lanes' and seat's villager riders removed (the posts'
mobs stay), the napper spawned at the queue seat at night and released by a redstone block in the
release's input once the hold has passed and day-1's gate sees night (REST_l and NOT DAY, from day
time 12,250: a napper picked before a skip is held until dusk); `--restop` brings the module's stop
(b36302af8's) up to this file (12 cells). Every row prints the stamp at A6's entry, at every calm
REST tick and at dusk, and the escape row. On b8949c96's drop-and-roll WIP (e9f0f4cbf, 8 seeds):

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

The contract run on (C) (the rail descent at 0.171 b/t, ironspec 16:20) with the second landing
follows its lay: 0 HOME lost over every skip tick and release phase, the forced control biting.

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed. Not bridged; the
  design must not rely on it.
