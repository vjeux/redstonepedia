# The single-lane carousel loop (being built)

vjeux, Nov 10 05:58: "a single lane where the villagers are riding on back to back with a
mechanism to release them 3 by 3". The brief: designs/briefs/farm-iron-carousel.md, the top
section. This folder holds the loop as it is built; WIP.

## The queue: a sawtooth shift register

Each queue slot is one 3-cell period: a powered rail sloping down (the cart's resting cell, braked
while unpowered), a plain rail sloping up, a plain flat peak (the next slope's sturdy high side).
One power pulse on every slope (7..12 gt) rolls every cart down, up, over the peak and onto the
next braked slope: every cart one slot on, the front one out. A cart starting lower arrives slower
and stops sooner, so the resting offsets converge (0.37..0.56 of the cell): no drift. The carts sit
3 cells apart, outside the 0.2 push range: no creep, no bounce, no momentum build-up.

Measured on redsim (`mctas mobfarm queue --scene saw` + `queue-run --cycle-start ... --pulse K`):
pulses of 7..12 gt identical, one cart out per pulse; pulses at least 16 gt apart (12..14 slip).

What did not work (same tool):
- flat brakes, carts 1.0 apart: one release dragged 10 carts out (push momentum);
- flat brakes 1.25 apart: holds and releases exactly 3, but a flat queue cannot advance (a
  stopped cart on a flat powered rail starts only off a solid block at its rail's end);
- a gravity staircase (one cart per slope cell, all released together): lockstep but drifting,
  no reference, and 21 tall.

## The loop (prototype, `mctas mobfarm queue-loop` + `queue-loop-run`)

Queue row A (z 0, going -x, slots 0..8), a U-turn at the peaks' height (curve, booster, curve;
rows 2 apart), row B (z 2, going +x, slots 9..17; the release group = slots 15..17), the release
run (20 cells along z 2, boosters on its first 3 so a train reaches the speed cap together and
keeps its spacing), the stand (3 sawtooth slots), the departure run, a U-turn to z 0 and the
return run back beside the queue into slot 0.

Per slot (t0 = the golem pulse): the release group powered 20 gt (its 3 carts leave as a train)
and the stand powered (its batch leaves as a train); the stand fills front first (S1 and S2 braked
as the train's front cart reaches S2's peak, S3 as its second reaches S3's peak); the queue
advances 3 slots (3 pulses from t0+40, 20 gt apart); the queue's 3 empty tail slots fill front
first from the returning train the same way.

Open: a cart that stops deep in its slot (arriving fast) cannot climb out when powered; the
returning train's timing against the next advance; the bends' bench; the stand's cells against
farmopt's pad.
