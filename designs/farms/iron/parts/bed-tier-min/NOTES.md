# The bed stop (the lead's napper stop: F1 + Q2, one bed, 24 villagers)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-two`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), farmopt (ca87cf44), G1 (8146b8db), day-1
(a75cc97c). Placed under the station, stop 9 down (ironspec 12:23): loop = local + (0,-27,-1).

## Files

- `bed-tier-fold.txt`: the stop, design A6 (ironspec 10:46-12:52). 17.8 build minutes at middle
  prices (`mctas mobfarm bill`), the leaves' shears and the landing's chest included. Not built:
  F and the release from the bed observer (the controller's: day-1's line ends at the hatch's two
  levers and the park's two). The way-in (b8949c96) ends at the entry booster.
- `bed-tier-fold-bench.txt`: the same with a run-out past R and the two-visit bench's queue rail.
- Header lines: `keep-clear:` (the bed's column (-3,1..3,4): the HOME path's target and the walk's
  drop; the leaves over the zombie (-5,3,4)) and `waker-places:` (where a napper has the zombie in
  sight within 8 once F shows it, from the bench's sight log; none before F).
- `mctas mobfarm bedtier` writes either and prints the file checks: placement-order cells 0, no T,
  R's reach set, the park's sloped switched rails, the zombie's post (its eye's sky light 14), the
  waker's sight lines and the zombie's sky column.

## How it works

The arrival heads west along z -4, open above: the entry booster (3,2,-4) (lit by a torch on
glass), the activator (2,2,-4) (always on), two braked powered rails down a slope (the park), a
capped flat, the curve south, seven flats under glass, the pickup cell (-2,0,4) beside the bed's
head, the curve east, a capped booster, a capped flat, the slope down, two flats, R (4,-1,5).

- **No solid-render block in the stop** (ironspec 10:19): rails, levers, torches on glass, so
  IDLE's stroll finds no candidate from any cell a napper stands in.
- **The landing is a chest** (2,2,-3): the activator sets the rider down on it at y 2.875, above
  the cart's box (top 2.7625). A save and load clears Entity.boardingCooldown (not saved): on a
  glass landing the rider stood level with the still-moving cart, 0.01 outside its pickup reach,
  and walked into it (below), so a restart in the eject's first few gt re-seated it and the parked
  cart kept it (160 of 21,600 restart rows at entry + 5, entries 0.4 b/t and up). On the chest the
  boxes never meet. Glass at (2,5,-3): no golem fits on it.
- **The entry and the activator are uncapped.** A riding napper in calm REST paths from its cart,
  and keeps following that path after the eject. Its start is the first cell over the rider that
  is not air or passable: under a cap, the cap's own cell, a 1-node partial path; the rider then
  walked back toward it after the eject and stood 120-240 gt against the cap (the stamp armed).
  Open above, the path from the activator's cell jumps onto the chest and runs down the ledge,
  complete. A path last computed in the entry's cell (entries 0.3 b/t and up, ejected before a
  recompute) is still 1 node: those visits wait 112-137 gt on the chest, clean (stamp at most
  137). The way-in's roll arrives at 0.1946 b/t (b8949c96), inside the fast band.
- **The walk** (all at y 2): the chest, south along x 2, west along z 0 over the approach's glass
  covers, south to (-2,2,2), west to (-3,2,2), onto the bed foot's roof (-3,2,3) (1.80 from the
  bed head's centre: most nappers lie down there), south into the open column over the bed's head:
  a 1.44 drop onto the bed, no way back up. The bed column's first non-solid cell (-3,1,4) is the
  HOME path's target (`findSurfacePosition`), so the walk's path is complete and arms no stamp.
- **Night:** it lies down on the walk (REST). F (the lie-down + 10, the bed observer's edge) shuts
  the hatch's two iron trapdoors: the zombie (persistent, in a boat at its post (-4.5,0,4.5),
  leaves over it) wakes it, it stands up on the pickup cell, the park releases 30 gt after the
  wake edge, the cart seats it under 0.21 b/t and runs on to R.
- **IDLE** (a skip, a stale arrival): it waits where it stands (the chest, the ledge, the bed's
  top, the pickup cell) with no walk target and no stamp, and lies down at REST's first tick.
- **The stand-up:** the hatch cell has no floor, so the pickup cell is findStandUpPosition's answer
  at every wake on both sides (the bed's top comes last, after the surround cells).
- **R's pulse** re-arms the stop (the park braked, the hatch open) and releases the queue.

## What the stop relies on

- **Entry speed 0.02-0.62 b/t** heading west at the entry booster (all seated). From 0.65 the cart
  brakes lower on the park and the released cart stops 0.5 short of the pickup cell.
- **The stamp that leaves with the napper:** the napper's calm REST HOME walk from the cart arms
  CANT_REACH on the way out (wake + 64..103 gt, at the exit cells; age at R 0-38 gt). The loop
  clears it: the S-bend seat about 250 gt after R (onechunk's phase 3a, ironspec 12:44).
- **The way-in's last cells** (4..8,2,-4) open above (no cap), as is b8949c96's roll.
- **The lift's entry keeps (5,-1,5), east of R, at 2 rail sides or fewer** (7b90c3f1).

## Golem spawn flood

- `mctas mobfarm queue-loop-spots bed-tier-fold.txt --rails --positions <the walk's cells, the
  bed, the pickup cell, the entry and the activator>`: 0 golem spawn cells off the pad from 36
  positions; `mctas mobfarm exit bed-tier-fold.txt --part-flood --positions <the same>`: no golem
  can spawn anywhere in the build. (On the first A6 lay both failed: a spawn cell over the bed
  foot's roof, now a floor stone button at (-3,2,3); golems on the leaves over the zombie, now
  glass at (-5,4,4).)

## Bench rows (8 seeds; bench, tool unreviewed)

On A6 (main 93d67fdae). Entries 0.1/0.1946/0.2 b/t unless stated. Every row: HOME 0 lost in the
stop, findStandUpPosition's answer the pickup cell at every wake (both sides), 0 hurt, 0 boat
seats, 0 north paths, 0 riderless carts, the waker in sight within 8 only after F.

| row | visits, clean | notes |
|---|---|---|
| restarts +1..+250 every 3 gt x 10 orders | 20,160 / 20,160 | lie-down entry + 85..93, R 188..252; stamp at most 38 at a calm REST tick, 0..38 at R |
| re-board: restarts +1..+30 x 10 orders, entries 0.02..0.62 | 24,000 / 24,000 | 0 re-seated (the glass landing: 160 of 21,600 at entry + 5) |
| skip at every gt +1..+300 | 7,200 / 7,200 | outcome a 5,392, b 1,808; stamp at most 15 |
| thunder: arrival 11,593, skip +1..+420 | 10,080 / 10,080 | all b, waiting on the chest or the bed's top |
| stale arrivals, day time 0..80 | 1,944 / 1,944 | all b |
| early arrivals 11,500..12,000 every 5 | 2,424 / 2,424 | wait on the chest up to 500 gt |
| window end 23,000..23,990 every 10 | 2,400 / 2,400 | |
| panic tail at entry 0..40 gt | 504 / 504 | lie-down by entry + 141 |
| skip x restart 1..200 gt after it x 10 orders | 13,200 / 13,200 | stamp at R at most 105 |
| stale x restarts at 50..11,500 x 10 orders | 4,800 / 4,800 | |
| entries 0.3..0.62 (the night row) | all clean | the riding path last computed in the entry's cell: lie-down entry + 197..229, stamp at most 144 |
| two visits, restarts every 16 gt over +2..+502 x 10 orders | 7,704 / 7,704 | calm-REST ticks within 16 of an occupied bed: A 0, C 0; A out of the ball to C's lie-down 97..336 gt (bar 20); R to A out of the ball 26 gt (the bench's lift; bound 152); departures 227..496 gt apart (bar 177) |
| two visits, A at day time 0..80, C queued | 192 / 216 | the 24 fail at day time 0: C loses HOME at 12,013 at the bench's queue cell (no stamp clearer there), before its entry |
| two visits, a skip in A's visit (every 7 gt) | 0 / 1,032 by HOME | every loss at the bench's queue (C, held through the day) or on the bench's lift top (A, after R): the rows' stop visits complete |
| control `--no-presence-gate --empty` | 24 out | the empty cart leaves |
| control `--trapdoor-caps`, restarts +90..+180 every 4 gt x 10 orders | 5,520 / 5,520 | no path off the pickup cell after a restart in A6, so the contact mode no longer shows |

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed. Not bridged; the
  design must not rely on it.
