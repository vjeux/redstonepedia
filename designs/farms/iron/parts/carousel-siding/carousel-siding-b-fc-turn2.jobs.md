# Bed stop (option 2, as placed at W): where the 477 blocks go

**Source:** explore/farmopt-station at 8cafab728, `designs/farms/iron/parts/carousel-siding/carousel-siding-b-fc-turn2-alone.txt`.
- The part has **477 blocks** in a 14 × 18 × 16 box.
- The bench file adds 83 bench-only blocks: 31 for the launcher, approach and run-out, and 54 for the 21 riders' hold and its zombies.
- How it's counted: `MCTAS_SIDING_JOBS=1 mctas mobfarm siding --wired --alone --turn 2`. Each block is counted once, under the generator section that first placed it. A support stone counts with the part on top of it.
- **Supports alone are 198 stone and 64 glass** (55% of the part).

## By job

| job | blocks | what it is |
|---|---|---|
| **Forced exit** | **94** | FAST: when F comes and no nap happened, kick at once. F's tap from the force cell, N ("a nap this visit") locked by H, FA = F − N, the two-torch tower, SC before the kick. 47 of the 94 are stone supports. |
| **Pickup kick** | **92** | K (the park's torch), BK, and the delay chain into BK (not-H delayed 5 repeaters, L's line along x 6, not-FC): 72. K's hold and the B-now line into the chain's end: 20. |
| **Visit state H and its outputs** | **65** | The H latch (N1, N2, W1, W2, D1, D2, H out): 30. The H line from the latch to its outputs: 25. Ha, Aa and their feeds into the activator and hatch controls: 10. |
| **Seat sensor and exit switch** | **45** | The seat string, its hooks and its overpass line into the T: 31. The entry, the T, the exit branch and the slot (track): 14. |
| **Visit end (R)** | **37** | L's string over the slot: 4. R's line from L into N2 (H's reset) and DB (V's reset): 33. |
| **Spacing filler** | **29** | Option 2's two new rails: 4. The seam dust where 5 lines cross the 2-cell shift: 20. Glass caps on golem spots: 5. |
| **Waking zombie and hatch** | **27** | The zombie's boat box and roof, the hatch, and its control line from the L network. |
| **Cage** | **26** | Glass walls and roof keeping the awake napper on the free cell, plus the booster's top-half trapdoor cap. |
| **Divert gate V and F's gate** | **24** | ARR, the dropper pair DA/DB (self-recovering from a missed or extra pulse, which is also the reload/freeze resync), not-V, and the gate letting F set H only during a visit. |
| **Bed and drop** | **19** | The bed: 4. The activator slope, low rail, park rail and their caps: 11. The booster and free cell: 4. |
| **Empty-cart hold** | **19** | FC: the string over the free cell and not-FC into the kick chain. The parked cart isn't kicked until the napper stands on the free cell. |
| **Give-up timer** | **0** | F comes from day-1's scheduler; its gate is counted under V. |
| **Freeze and reload resync** | **0** | No separate part. V's droppers recover by themselves, and VR is day-1's. |
| **Bench-only track** | **0** in the part | +83 in the bench file. |
| **Total** | **477** | |

**Where the size comes from:**
- **The forced exit and the kick logic: 186 blocks, 39%.** They exist because the bed line is a dead-end spur. The cart parks beyond the bed, and its kick back toward the napper has to wait on evidence: H, the delay chain, FC, or FAST when there was no nap.
- **H, R and V: 126 blocks, 26%.**
- **The T and its seat switch: 45 blocks.** They exist because the cart leaves the way it came in.

## By generator section (raw)

| section | blocks |
|---|---|
| forced exit: FAST (F with no nap kicks at once): F's tap, N, FA, the torch tower, SC | 94 |
| kick: K, BK and the chain into it (not-H delayed, L, not-FC) | 72 |
| R line (L's rise into N2 and DB) | 33 |
| seat string and its line into the T | 31 |
| H latch (N1, N2, W1, W2, D1, D2, H out) | 30 |
| waking zombie and hatch | 27 |
| cage: glass walls and roof round the free cell and the bed | 26 |
| H line (the visit's state to its outputs) | 25 |
| V divert gate (ARR, DA, DB, not-V) and F's gate into H | 24 |
| K's hold and B-now line (into the chain's end) | 20 |
| option 2: the lines' seam dust | 20 |
| FC string over the free cell and not-FC into the chain | 19 |
| track: entry, T, exit branch | 14 |
| drop and park: activator slope, low line, park, caps over it | 11 |
| H's outputs: torches and feeds into the T, activator and hatch controls | 10 |
| golem-spot caps (glass) | 5 |
| L string over the slot | 4 |
| bed | 4 |
| option 2: the two new rails | 4 |
| track: bed line (booster, free cell) | 4 |

## Cells per section (local frame; the turned file is x,z -> -x,-z)

**blocks (local frame, before the turn) (477):** **forced exit: FAST (F with no nap kicks at once): F's tap, N, FA, the torch tower, SC (94):** (-4,-4,-4) (-3,-4,-4) (-2,-4,-4) (-4,-3,-4) (-3,-3,-4) (-2,-3,-4) (-2,-5,-3) (-2,-4,-3) (-3,-6,-2) (-2,-6,-2) (-3,-5,-2) (-2,-5,-2) (-3,-6,-1) (-3,-5,-1) (-3,-6,0) (-3,-5,0) (-3,-6,3) (1,-6,3) (-3,-5,3) (0,-5,3) (1,-5,3) (0,-4,3) (1,-7,4) (-3,-6,4) (1,-6,4) (-3,-5,4) (1,-7,5) (-3,-6,5) (1,-6,5) (-3,-5,5) (1,-7,6) (-3,-6,6) (1,-6,6) (-3,-5,6) (-3,-7,7) (-2,-7,7) (-1,-7,7) (0,-7,7) (1,-7,7) (-3,-6,7) (-2,-6,7) (-1,-6,7) (0,-6,7) (1,-6,7) (0,-7,8) (1,-7,8) (2,-7,8) (0,-6,8) (1,-6,8) (2,-6,8) (3,-3,8) (4,-3,8) (3,-2,8) (4,-2,8) (2,-7,9) (0,-6,9) (2,-6,9) (-3,-5,9) (-2,-5,9) (-1,-5,9) (0,-5,9) (-3,-4,9) (-2,-4,9) (-1,-4,9) (0,-4,9) (0,-3,9) (1,-3,9) (4,-3,9) (1,-2,9) (4,-2,9) (2,-7,10) (-4,-6,10) (2,-6,10) (-4,-5,10) (-3,-5,10) (-3,-4,10) (-2,-4,10) (-2,-3,10) (-1,-3,10) (-1,-2,10) (-4,-7,11) (-3,-7,11) (-2,-7,11) (-1,-7,11) (0,-7,11) (1,-7,11) (2,-7,11) (-4,-6,11) (-3,-6,11) (-2,-6,11) (-1,-6,11) (0,-6,11) (1,-6,11) (2,-6,11)

**kick: K, BK and the chain into it (not-H delayed, L, not-FC) (72):** (5,-1,1) (6,-1,1) (5,0,1) (6,0,1) (3,-1,4) (6,-1,4) (3,0,4) (6,0,4) (3,-1,5) (6,-1,5) (3,0,5) (6,0,5) (-6,-4,6) (-6,-3,6) (3,-1,6) (4,-1,6) (5,-1,6) (6,-1,6) (3,0,6) (4,0,6) (5,0,6) (6,0,6) (-6,-4,7) (-6,-3,7) (6,-1,7) (6,0,7) (-6,-4,8) (-5,-4,8) (-6,-3,8) (-5,-3,8) (1,-3,8) (2,-3,8) (1,-2,8) (2,-2,8) (6,-1,8) (6,0,8) (-6,-4,9) (-6,-3,9) (2,-3,9) (2,-2,9) (6,-2,9) (6,-1,9) (-6,-4,10) (-6,-3,10) (2,-3,10) (3,-3,10) (4,-3,10) (5,-3,10) (6,-3,10) (2,-2,10) (3,-2,10) (4,-2,10) (5,-2,10) (6,-2,10) (-6,-4,11) (-5,-4,11) (-4,-4,11) (-3,-4,11) (-2,-4,11) (-1,-4,11) (0,-4,11) (1,-4,11) (2,-4,11) (-6,-3,11) (-5,-3,11) (-4,-3,11) (-3,-3,11) (-2,-3,11) (-1,-3,11) (0,-3,11) (1,-3,11) (2,-3,11)

**R line (L's rise into N2 and DB) (33):** (-7,-4,-4) (-7,-3,-4) (-6,-3,-4) (-6,-2,-4) (-5,-2,-4) (-5,-1,-4) (-4,-1,-4) (-3,-1,-4) (-2,-1,-4) (-1,-1,-4) (0,-1,-4) (1,-1,-4) (2,-1,-4) (-4,0,-4) (-3,0,-4) (-2,0,-4) (-1,0,-4) (0,0,-4) (1,0,-4) (2,0,-4) (-7,-4,-3) (-7,-3,-3) (-1,-1,-3) (-1,0,-3) (2,0,-3) (2,1,-3) (-7,-4,-2) (-7,-3,-2) (2,0,-2) (3,0,-2) (2,1,-2) (3,1,-2) (4,1,-2)

**seat string and its line into the T (31):** (0,1,-4) (-1,2,-4) (0,2,-4) (-1,3,-4) (0,0,-3) (-1,1,-3) (0,1,-3) (-1,2,-3) (-1,3,-3) (0,-1,-2) (1,-1,-2) (0,0,-2) (1,0,-2) (1,1,-2) (-1,2,-2) (1,2,-2) (-1,3,-2) (1,-1,-1) (1,0,-1) (-1,2,-1) (-1,3,-1) (-1,2,0) (-1,3,0) (-1,2,1) (-1,3,1) (-1,1,2) (0,1,2) (1,1,2) (2,1,2) (3,1,2) (-1,2,2)

**H latch (N1, N2, W1, W2, D1, D2, H out) (30):** (-7,-4,-1) (-6,-4,-1) (-5,-4,-1) (-7,-3,-1) (-6,-3,-1) (-5,-3,-1) (-7,-4,0) (-5,-4,0) (-4,-4,0) (-3,-4,0) (-2,-4,0) (-7,-3,0) (-5,-3,0) (-4,-3,0) (-3,-3,0) (-2,-3,0) (-7,-4,3) (-5,-4,3) (-7,-3,3) (-5,-3,3) (-7,-4,4) (-5,-4,4) (-7,-3,4) (-5,-3,4) (-7,-4,5) (-6,-4,5) (-5,-4,5) (-7,-3,5) (-6,-3,5) (-5,-3,5)

**waking zombie and hatch (27):** (4,0,2) (2,0,4) (2,1,4) (4,7,4) (4,8,4) (4,6,5) (3,7,5) (5,7,5) (3,8,5) (5,8,5) (4,10,5) (3,6,6) (3,7,6) (4,7,6) (4,8,6) (4,0,7) (4,1,7) (5,1,7) (5,2,7) (4,4,7) (3,5,7) (4,5,7) (3,6,7) (5,2,8) (4,3,8) (5,3,8) (4,4,8)

**cage: glass walls and roof round the free cell and the bed (26):** (0,-1,3) (0,0,3) (2,0,3) (0,1,3) (1,1,3) (2,1,3) (1,2,3) (2,2,3) (-1,0,4) (-1,1,4) (0,2,4) (2,2,4) (-1,0,5) (2,0,5) (-1,1,5) (1,1,5) (2,1,5) (0,2,5) (2,2,5) (0,0,6) (2,0,6) (0,1,6) (1,1,6) (2,1,6) (2,2,6) (1,1,7)

**H line (the visit's state to its outputs) (25):** (-1,-4,0) (-1,-3,0) (-1,-4,3) (-1,-3,3) (-1,-4,4) (-1,-3,4) (-1,-4,5) (-1,-3,5) (-1,-4,6) (-1,-3,6) (2,-3,6) (3,-3,6) (2,-2,6) (3,-2,6) (-1,-4,7) (0,-4,7) (1,-4,7) (2,-4,7) (3,-4,7) (-1,-3,7) (0,-3,7) (1,-3,7) (2,-3,7) (3,-3,7) (2,-2,7)

**V divert gate (ARR, DA, DB, not-V) and F's gate into H (24):** (-5,-4,-3) (-4,-4,-3) (-3,-4,-3) (-5,-3,-3) (-4,-3,-3) (-3,-3,-3) (-5,-4,-2) (-5,-3,-2) (-4,-3,-2) (-3,-3,-2) (-4,-2,-2) (-3,-2,-2) (-1,-1,-2) (-1,0,-2) (-4,-2,-1) (-3,-2,-1) (-2,-2,-1) (-4,-1,-1) (-3,-1,-1) (-2,-1,-1) (-1,-1,-1) (-1,0,-1) (-1,-1,0) (-1,0,0)

**K's hold and B-now line (into the chain's end) (20):** (-2,-2,8) (-1,-2,8) (0,-2,8) (-2,-1,8) (-1,-1,8) (0,-1,8) (0,0,8) (0,1,8) (-4,-5,9) (-4,-4,9) (-1,-3,9) (-2,-2,9) (-1,-2,9) (0,-2,9) (-2,-1,9) (0,-1,9) (1,-3,10) (0,-2,10) (1,-2,10) (0,-1,10)

**option 2: the lines' seam dust (20):** (-3,-6,1) (-3,-5,1) (-7,-4,1) (-5,-4,1) (-1,-4,1) (-7,-3,1) (-5,-3,1) (-1,-3,1) (-3,-6,2) (-3,-5,2) (-7,-4,2) (-5,-4,2) (-1,-4,2) (-7,-3,2) (-5,-3,2) (-1,-3,2) (6,-1,2) (6,0,2) (6,-1,3) (6,0,3)

**FC string over the free cell and not-FC into the chain (19):** (-5,-1,4) (-4,-1,4) (-5,0,4) (-4,0,4) (-3,0,4) (-3,1,4) (-2,1,4) (0,1,4) (1,1,4) (3,1,4) (-1,2,4) (-5,-1,5) (-5,0,5) (-5,-2,6) (-5,-1,6) (-5,-3,7) (-4,-3,7) (-5,-2,7) (-4,-2,7)

**track: entry, T, exit branch (14):** (4,-2,0) (5,-2,0) (6,-2,0) (0,-1,0) (1,-1,0) (2,-1,0) (3,-1,0) (4,-1,0) (5,-1,0) (6,-1,0) (0,0,0) (1,0,0) (2,0,0) (3,0,0)

**drop and park: activator slope, low line, park, caps over it (11):** (1,-2,5) (1,-1,5) (1,0,5) (1,-2,6) (1,-1,6) (1,0,6) (1,-2,7) (1,-1,7) (1,0,7) (1,-1,8) (1,0,8)

**H's outputs: torches and feeds into the T, activator and hatch controls (10):** (0,-4,0) (0,-3,0) (2,-2,4) (2,-1,4) (0,-5,5) (0,-4,5) (1,-4,5) (1,-3,5) (2,-3,5) (2,-2,5)

**golem-spot caps (glass) (5):** (-7,-2,-1) (-5,-2,4) (-4,-4,8) (-4,-3,8) (2,-1,8)

**L string over the slot (4):** (4,0,-2) (4,0,-1) (4,0,0) (4,0,1)

**bed (4):** (0,-1,4) (0,0,4) (0,-1,5) (0,0,5)

**option 2: the two new rails (4):** (1,-1,1) (1,0,1) (1,-1,2) (1,0,2)

**track: bed line (booster, free cell) (4):** (1,-1,3) (1,0,3) (1,-1,4) (1,0,4)

