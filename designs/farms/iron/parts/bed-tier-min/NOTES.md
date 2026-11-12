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

## Bench rows (8 seeds x entry speeds 0.1/0.2/0.3; bench, tool unreviewed)

On the glass-cover stop BEFORE the golem-proofing and the zombie's move (the boat at x -5.0;
explore/bedtier-stop's work tree on 2a5b685ea with the restart helper). The re-runs on this commit's
stop follow in the next one, each against these numbers.

| row | result |
|---|---|
| night | 24/24 seated; pickups 0.138-0.152 (this commit's stop: the same) |
| restarts (`liveness::restart`) +2..+300 every 2 gt x 10 orders | 36,000/36,000 clean, 0 not out, HOME 0 lost; exit at most 140 gt later than the night's; after a restart, the woken napper's paths off the pickup cell: south 6,815 visits, west 9,411, north 0 |
| positive control `--trapdoor-caps`, restarts +90..+180 x 10 orders | 11,040 visits: 30 not seated out (the cart stopped dead), 2,210 with a path north |
| two visits, no restart and restarts every 4 gt over +2..+502 x 10 orders | 30,264/30,264 clean; A at least 21.51 from the bed's head at C's lie-down; HOME 0 lost; A's R to C's lie-down 116-354 gt |
| daylight: A held at the queue from day time 0/100/300/600/6,000, the gate at 12,230/12,002/11,556, C 20 gt after A's drop or at 12,370/12,433 | 1,440 runs: every visit completes (A and C nap and are seated out, A at least 21.56 from the bed's head at C's lie-down, C at least 20 gt after A's drop, C waits 0-647 gt); HOME kept 483/720 per set: the bench's queue rail floats, so the held napper's CANT_REACH stamp arms at once and its first calm REST tick releases the bed (day time 12,004-12,019); re-run on the way-in line's queue cell |
| early arrivals 10,500-12,020 (cbcac083a's stop) | every one naps at REST's start, seated |
| window end (cbcac083a's stop) | arrivals up to day time 23,920 nap and are seated |

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged; the design must not rely on it.
