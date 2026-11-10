# The single-lane carousel loop (being built)

vjeux, Nov 10 05:58: "a single lane where the villagers are riding on back to back with a
mechanism to release them 3 by 3". The brief: designs/briefs/farm-iron-carousel.md, the top
section. This folder holds the loop as it is built.

## What is real (vjeux 07:29: "You can't say it's working when nothing is wired")

| part | in the file | controls | plays on its own | missing |
|---|---|---|---|---|
| queue register (18 slots) | track built | bench-driven (the slope supports switched glass/redstone block by the bench) | no | day-1's controller (being built) |
| release of the front 3 | track built | bench-driven | no | day-1's controller |
| stand (3 slots) | a sketch (farmopt owns the real stand, part 2) | bench-driven, position-triggered brake | no | farmopt's linear stand and pad |
| return to the queue's tail | track built | bench-driven, position-triggered brakes | no | timer circuits for the tail's brakes |
| zombies (PANIC), golem-floor gate | none | | | all |
| bed siding (naps) | none (farmopt designing it) | | | all |

Nothing here is wired redstone yet. "Clean" below always means with bench-driven controls.

## The queue: a sawtooth shift register

Each queue slot is one 3-cell period: a powered rail sloping down (the cart's resting cell, braked
while unpowered), a plain rail sloping up, a plain flat peak (the next slope's sturdy high side).
One power pulse on every slope (7..12 gt) rolls every cart down, up, over the peak and onto the
next braked slope: every cart one slot on, the front one out. A cart starting lower arrives slower
and stops sooner, so the resting offsets converge (0.32 of the cell down the slope): no drift. The
carts sit 3 cells apart, outside the 0.2 push range: no creep, no bounce, no momentum build-up.

Measured on redsim (`mctas mobfarm queue --scene saw` + `queue-run --cycle-start ... --pulse K`):
pulses of 7..12 gt identical, one cart out per pulse; pulses at least 16 gt apart (12..14 slip).
tick-exact bridged the register's one-slot moves and the U-turn against the real server (0
differences; controls still bench-driven).

What did not work (same tool):
- flat brakes, carts 1.0 apart: one release dragged 10 carts out (push momentum);
- flat brakes 1.25 apart: holds and releases exactly 3, but a flat queue cannot advance (a
  stopped cart on a flat powered rail starts only off a solid block at its rail's end);
- a gravity staircase (one cart per slope cell, all released together): lockstep but drifting,
  no reference, and 21 tall;
- a fast train caught by plain sawtooth slots settles deep (0.8..0.95 down the slope) and cannot
  climb out at its next departure;
- a 1-block ledge before a catch (off a rail a cart's speed is clamped to 0.4): lands too slow
  (~0.12 b/t), carts crawl and bump at the edge.

## The loop (`mctas mobfarm queue-loop` + `queue-loop-run`), bridge layout

- The stand (sketch) runs along +x at z 0: 3 catch slots, i.e. sawtooth slots whose preceding
  flat cell (S2's peak, S3's peak, the run's last cell) is a powered rail linked to the slope (a
  powered rail takes its connected neighbour's power): a fast train crosses its braked catch cell
  and stops halfway down its slope.
- The release run comes south along x -2 into the stand's west end; the departure leaves its east
  end north along x 10, jogs west along z 20 and enters the queue's tail; the queue sits north of
  z 20 in two rows (row A, the tail, north along x 0; a boosted U-turn; row B, the head, south
  along x -2 straight into the release run). The return is about as short as the release run.
- The queue's 3 tail slots are V slots (slope down, plain valley, slope up on its own line): a
  fast returning cart stops on the slope down or up; the slopes up go on 12 gt before the advance
  and stay on through it (a cart on one rolls back first, then leaves forward).

Per slot (t0 = the golem pulse; every control bench-driven):
- t0: the release group (the queue's front 3 slopes) on for 30 gt: the 3 leave as a train; the
  stand's batch leaves one slot at a time (S1 at t0, S2 +12, S3 +24);
- the stand braked as the train's front cart is 0.45 into its catch cell (about t0+89..91);
- the queue advances 3 slots: pulses at t0+40, +60, +80, 9 gt each (the tail's slopes up on from
  t0+28);
- the returning train fills the queue's tail front first: slots 1-2 braked as its front cart
  reaches slot 1's peak, slot 0 as its second reaches slot 0's peak (about t0+111..122).

Bench-driven result: 3 slots clean (batch at the stand +97..99, riders at 0.66..0.87 of their
slopes; returning train in the tail +117..122). From the 4th slot the queue's row B degrades:
carts that came round the U-turn fast settle deep and crawl over the next peaks. Next: the U-turn.
