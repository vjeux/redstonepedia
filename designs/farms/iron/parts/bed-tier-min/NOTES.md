# The bed stop (the lead's napper stop: F1 + Q2, one bed, 24 villagers)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-two`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), farmopt (ca87cf44), G1 (8146b8db), day-1
(a75cc97c). Placed under the station in 9fc3155f's budget: loop = local + (0,-18,-1).

## Files

- `bed-tier-fold.txt`: the stop, 142 placed blocks, 24.0 build minutes at middle prices. Not
  built: F and the release from the bed observer (~12 blocks), the queue's latch, trapdoor and wire
  from R (~30), the queue's DAY gate (~4).
- `bed-tier-fold-bench.txt`: the same with a run-out past R and the two-visit bench's queue rail.

## How it works

The arrival heads west along z -4: the entry (3,2,-4), a booster, always on; the activator (2,2,-4),
always on (the rider lands on the ledge south of it); two braked powered rails down a slope (the
park: every arrival stops on the first); a flat; the curve south; a capped 2-level hump (two slopes
up, a raised flat, two slopes down); two capped flats; the pickup cell (-2,0,4) beside the bed's
head; the curve east; a capped booster, always on; a flat; the slope down; two flats; R (4,-1,5),
loop (4,-19,4), heading east.

- Normal operation only (vjeux 2026-11-12). Under L1‴ a restart cannot put a woken napper back to
  bed before the cart reaches it: wake to pickup is 55-75 gt, under SleepInBed's 100-gt LAST_WOKEN
  guard (a saved memory; tick-exact 00:55). So the stop handles no strand. The spurs-and-kicks fix
  (explore/farmopt-station 47bbde753..f8147b303) is the pearl alternative's, for chunks unloaded
  while game time runs.
- The napper walks the ledge onto the bed's roof and lies down. The controller's F (the hatch shut:
  the zombie seen) comes 10 gt after the lie-down: on the build, the bed observer's lie-down edge
  plus a delay. The release (the park's supports powered) comes 30 gt after F. The bench emulates
  both on game ticks.
- Woken, the napper stands only on the pickup cell. The released cart reaches it under 0.2 b/t and
  seats it.
- A napper stepping into the cart can stop it dead and be seated by a cart pushed back north. After
  a restart its brain has no panic and it strolls in the cell. The pushed cart climbs the hump's
  south slopes, stops short of the top, and rolls back through the pickup cell to R.
- R's pulse re-arms the stop (the park braked, the hatch open) and releases the queue: the next
  napper's cart waits at the queue cell, loop (9,-8,-6), 17.8 from the bed's head.

## Bench rows (8 seeds x entry speeds 0.1/0.2/0.3; bench, tool unreviewed)

| row | result |
|---|---|
| cold start (a fresh design, the night) | 24/24 clean: lie-down 80-81 gt after the entry, pickup 0.169-0.184 b/t, R 196-198 |
| a restart (save, stop, start) at +2..+300 every 2 gt | 3,599/3,600; the exit at most 140 gt later than the night's (the +2 restart delays the first lie-down); one failure (+126, seed 5, 0.1): the napper stepped into the cart, the push-back was too weak to reach the hump and the empty cart stopped on the flats |
| two visits, no restart and a restart every 4 gt over +2..+502 | 3,044/3,048; A's distance to the bed's head at C's lie-down 21.38 or more (bar 17); A's R to C's lie-down 116-356 gt; the 4 failures the same contact mode |
| early arrivals, day time 10,500..12,020 | all nap when REST starts (12,009-12,189) and are seated out; 11,940 (60 gt early): trip about 370 gt |
| the window's end, arrivals every 20 gt over 23,400..24,000 | every arrival up to 23,920 naps (lie-down 24,000) and is seated out; 23,940 lies down at 24,020 and is not picked up; later ones never nap: the cutoff is an arrival by 23,920 |
| daylight arrivals (day time 2,000 and 6,000) | none naps: no F, no release, the cart parked until dusk (the queue's DAY gate keeps such a napper at the queue) |

Without the hump (144 blocks, pickup 0.138-0.158, R 193-201) the restart sweep had 2/3,600 failures:
the pushed-back cart seated the napper and rolled north to the activator, which ejected it.

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged; the design must not rely on it.
