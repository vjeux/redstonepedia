# The bed stop (the lead's napper stop: F1 + Q2, one bed, 24 villagers)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-two`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), farmopt (ca87cf44), G1 (8146b8db), day-1
(a75cc97c). Placed under the station in 9fc3155f's budget: loop = local + (0,-18,-1).

## Files

- `bed-tier-fold.txt`: the stop. 153 placed blocks, 20.2 build minutes at middle prices.
  Not built:
  - F and the release from the bed observer (~12 blocks);
  - the queue's NOT DAY gate (~4).
  The queue's latch, trapdoor and wire from R (~30) are the way-in line's (b8949c96).
- `bed-tier-fold-bench.txt`: the same with a run-out past R and the two-visit bench's queue rail.
- `mctas mobfarm bedtier` writes either and prints the file checks:
  - the placement-order cells (built in order, 20 gt apart, against the file): 0;
  - the Ts (a rail with rails on 3 or more sides, each side's rail at its level, one up or one
    down): none;
  - R's raw connections and G1's full reach set (the 6 face neighbours of R and of the cell below
    it), each cell's block and rail sides: no rail at 3;
  - the Ts face-adjacent to the cell above each sloped switched rail (the park's two): none;
  - the zombie's boat and its rider at bench start and 20 gt later: inside the post cell, the
    zombie's eye cell at sky light 14 (the leaves over the post).

## How it works

The arrival heads west along z -4:

- the entry (3,2,-4), a booster, always on;
- the activator (2,2,-4), always on (the rider lands on the ledge south of it);
- two braked powered rails down a slope (the park: every arrival stops on the first);
- a flat, the curve south;
- seven flats under glass;
- the pickup cell (-2,0,4) beside the bed's head;
- the curve east, a capped booster (always on), a flat, the slope down, two flats;
- R (4,-1,5), loop (4,-19,4), heading east.

Rules and timing:

- **Normal operation only** (vjeux 2026-11-12). Under L1‴ a restart cannot put a woken napper back
  to bed before the cart reaches it: wake to pickup is 55-75 gt, under SleepInBed's 100-gt
  LAST_WOKEN guard (a saved memory; tick-exact 00:55). So the stop handles no strand. The
  spurs-and-kicks fix (explore/farmopt-station 47bbde753..f8147b303) is the pearl alternative's,
  for chunks unloaded while game time runs.
- **The visit.** The napper walks the ledge onto the bed's roof and lies down. The controller's F
  (the hatch shut: the zombie seen) comes 10 gt after the lie-down: on the build, the bed
  observer's lie-down edge and a delay. The release (the park's supports powered) comes 30 gt
  after F. The bench emulates both on game ticks.
- **The pickup.** Woken, the napper stands only on the pickup cell. The released cart reaches it
  at 0.138-0.152 b/t and seats it; a cart at 0.2 b/t or more stops dead against a standing
  villager.
- **Glass over the approach flats.** The covers are glass (full blocks at y 1) over the empty cart,
  not top-half trapdoors. The pathfinder takes a trapdoor cell for passable. A panicking napper on
  the pickup cell is on a rail, so it may path along rails. After a restart its rebuilt brain
  draws a new flee target, and over trapdoors its path could run north up the approach, into the
  arriving cart. The cart then stopped dead: G1's contact mode, the `--trapdoor-caps` control.
- **Golem-proof** (ironspec 2026-11-12, onechunk's flood: 20 off-pad spots on cbcac083a). No cell
  of the stop is a golem floor with air over it:
  - the ledge's floor is glowstone (the same full block for the walk and its path types);
  - the curve's cover after the park is glass;
  - glass fills the cells over the entry's and the activator's caps and over the activator's
    redstone block;
  - a floor stone button (no golem floor, no collision box, unpressed: no power) fills the cell
    over each trapdoor a rider's box or the napper's walk reaches: the flat after the park (the
    cart comes off the park's slope too high for a full block there), the exit's three caps and
    the bed's roof (the napper stands on the roof's top, 1.80 and 1.50 from the bed head's centre,
    inside SleepInBed's 2).
- **The zombie** rides a boat at its post cell's centre (-4.5, 0, 4.5) local; cbcac083a had the
  cell's west edge (-5.0), x written without its + 0.5.
- **R's pulse** re-arms the stop (the park braked, the hatch open) and releases the queue. The
  next napper's cart waits at the queue cell, loop (9,-8,-6), 17.8 from the bed's head; the NOT
  DAY gate (L 6: clear 12,230, rain 12,002, thunder 11,556) holds the queue by day. The held
  napper makes the queue cell an all-day rider position under the brief's stamp rule: the way-in
  line (b8949c96) gives it stamp-clearing surroundings (ironspec 04:06).

## What the stop relies on (cross-part constraints)

- **The NOT DAY gate** (b8949c96's queue). An arrival more than about 1,200 gt before REST waits
  on the ledge with its CANT_REACH stamp armed and gives up the bed at its first calm REST tick:
  the early-arrival rows lose HOME for arrivals up to day time 10,800 and none from 10,820. The
  gate's earliest drop (thunder, 11,556, the entry 37 gt later) keeps every arrival after about
  11,593, the stamp under about 420 gt at REST.
- **The queue latches R until C arrives**, or C is queued before R (G1 on d90745270): the bench
  releases C at the later of A's R and C's arrival, plus the transit.
- **F before dawn, every arrival at day time 23,920 or earlier.** F comes 10 gt after every
  lie-down (game ticks; a night skip doesn't stop it), and with F the hatch is shut, so the hatch
  cell (-4,0,4) has 1.625 of headroom and is no stand-up cell. A wake with the hatch open (a dawn
  wake: arrivals from 23,940) can stand the napper up in the hatch cell, and the zombie's boat
  (1.375 wide, centred in its 1-cell post: it reaches 0.19 into the hatch cell, plus its 0.2
  pickup reach) seats it. day-1's latest arrival is about 20,750 (ironspec 06:50).
- **The lift's entry keeps (5,-1,5), east of R, at 2 rail sides or fewer** (7b90c3f1's; the
  assembly's scan checks it).

## Golem spawn flood

- The stop alone: `mctas mobfarm queue-loop-spots bed-tier-fold.txt --rails --positions <the
  walk's 18 cells: the landing, the ledge, the roof, the column over the pickup cell, the pickup
  cell, the bed>`: 0 golem spawn cells off the pad seen from 38 positions; `mctas mobfarm exit
  bed-tier-fold.txt --part-flood --positions <the same>`: no golem can spawn anywhere in the build.
- In the module: 9fc3155f's flood of the assembled world with d90745270 pasted at (0,-18,-1): 0
  off-pad spots from the stop's 34 positions, 0 in every other region (farmopt 06:00).

## Bench rows (8 seeds x entry speeds 0.1/0.2/0.3; bench, tool unreviewed)

On this stop (main 93d67fdae). "Flee paths": the woken napper's paths off the pickup cell, by
direction, after a restart and after the wake in any row (a path north, up the approach into the
arriving cart, is a problem in every row). "Lie reach": the napper's feet the tick before each
lie-down to the bed head's centre (under 2 by construction: SleepInBed starts the first tick it is
inside 2.0; the walk ends on the roof at 1.80 and 1.50). The column "before" is the glass-cover
stop before the golem-proofing and the zombie's move (the boat at x -5.0; on 2a5b685ea).

| row | result | before |
|---|---|---|
| night | 24/24 seated; pickups 0.138-0.152; flee paths south 17, west 7, north 0; lie reach 1.938 | 24/24, pickups 0.138-0.152 |
| restarts (`liveness::restart`) +2..+300 every 4 gt x 10 orders | 18,000/18,000 clean, 0 not out, HOME 0 lost; exit at most 140 gt later than the night's; flee paths after a restart south 3,379, west 4,773, north 0; after the wake south 11,242, west 7,804, north 0; lie reach 1.9273-1.9990 | every 2 gt: 36,000/36,000; after a restart south 6,815, west 9,411, north 0 |
| positive control `--trapdoor-caps`, restarts +90..+180 every 4 gt x 10 orders | 5,520 visits: 20 not seated out (the cart stopped dead); flee paths north after a restart 1,198, after the wake 3,613 | every 2 gt: 11,040 visits, 30 not seated out, 2,210 with a path north |
| two visits, no restart and restarts every 16 gt over +2..+502 x 10 orders | 7,704/7,704 clean (no path north in any row); A at least 21.51 from the bed's head at C's lie-down; HOME 0 lost; A's R to C's lie-down 116-354 gt; lie reach 1.9234-1.9995 | every 4 gt: 30,264/30,264, A at least 21.51 |
| daylight: A held at the queue from day time 0/100/300/600/6,000, the gate at 12,230/12,002/11,556, C 20 gt after A's drop or at 12,370 | 720 runs: every visit completes (A and C nap and are seated out, no path north, A at least 21.56 from the bed's head at C's lie-down, C at least 20 gt after A's drop, C waits 0-647 gt; lie reach 1.5484-1.9966); HOME kept 483/720: the bench's queue rail floats, so the held napper's CANT_REACH stamp arms at once and its first calm REST tick releases the bed (day time 12,004-12,019); the queue cell's surroundings are b8949c96's, the rows re-run on them | the same |
| early arrivals, day time 10,500-12,020 every 20 | 1,848 visits, all seated, flee paths south 1,042, west 806, north 0; HOME lost 67, all at arrivals up to 10,800 (the stamp rule: see What the stop relies on), none from 10,820 | cbcac083a's stop: all napped at REST's start |
| window end, day time 23,400-23,980 every 20 | arrivals up to 23,920: 648/648 clean; 23,940 wakes at dawn with the hatch open and is seated in the zombie's boat; 23,960 and later never nap | cutoff 23,920 |

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged; the design must not rely on it.
