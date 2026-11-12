# The minimal bed tier (the one-chunk farm's napper stop)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`). Owner: session
58f770cc. Rulings: ironspec (3883f332), day-1 (a75cc97c), onechunk (7d3d2afb), farmopt (ca87cf44).

## Files

- `bed-tier-min.txt`, `-bench.txt`: the linear tier (accepted 37099474d: 24/24 night, 312/312
  freeze, both controls), 191 blocks, superseded by the fold.
- `bed-tier-fold.txt`, `-bench.txt`: the tier folded into C′'s envelope (accepted ca933ae87), 158
  blocks, 25.9 build minutes at middle prices. The design is the loop frame minus (0,50,0); the
  bed's head at loop (-3,50,4).
- `bed-tier-fold-trip.txt`, `-trip-bench.txt`: the fold with the napper's legs (the divert spiral
  from the switch flat (9,36,-1), the return helix to the re-entry's T (7,34,9)), 265 blocks, 38.7
  build minutes (accepted a5fb7eafb).

## How it works

The arrival heads west along z -4: a booster entry, an always-on activator (the rider lands on the
ledge south of it), two braked powered rails down a slope (every arrival stops on the first), a
capped flat, a curve, seven capped flats, the pickup cell beside the bed's head, a capped booster, a
curve east, two flats, the exit slope, a flat, the hold under the rider check's string, R.

- The napper walks the ledge onto the bed's roof: SleepInBed starts within 2 of the head. A villager
  off a rail cannot path onto a rail cell (UNPASSABLE_RAIL), so the pickup cell is no way in; the
  open column over it lets a napper fall in (it then stands on a rail and paths to the bed).
- F (the hatch shut: the waker seen) comes 10 live gt after the lie-down (the bed's own evidence),
  or at the give-up; the release (the park's supports powered) 30 live gt after F. Live time is the
  farm's cart-driven ring, so a level-32 freeze pauses both.
- The woken napper stands up on the pickup cell only and keeps seeing the waker there (one
  two-trapdoor hatch, the bed roofed by top-half trapdoors: the standing eye line runs under them).
- The pickup: a moving minecart collides with a pushable entity in its own move and stops dead
  (speed zeroed), so it seats a standing villager only from a tick that ends with its box grown 0.2
  over the villager and still short of contact: under about 0.2 b/t at the napper. The two-cell park
  and the seven flats bring it to 0.143-0.168.
- The exit's rider check: a tripwire at rider height over the hold itself (a seated rider's box
  crosses its band, an empty cart's 0.7 does not). The hold is a braked rail powered while a rider is
  on the string; R's detector is past it, so a held cart never reads as the tier cleared.
- Day-1's final rule for empties (emulated on the bench, to be wired): an entry rider check tells
  "arrived empty" (F at once when parked; the hold releases it) from "arrived ridden, leaves empty"
  (a stranded napper: released and RIDER_ALARM set if the alarm was clear, held if set).

## Bench rows (fold, 8 seeds x arrivals 0.1/0.2/0.3)

| row | result |
|---|---|
| night | 24/24 seated at 0.143-0.165 b/t; the napper only on the pickup cell; lie-down 80-81, R 192-200 |
| freeze 400 gt at 10 phases (+10..+180) | 240/240 seated |
| a cart sent empty (no entry check) | 24/24 held |
| arrived empty (entry check) | 24/24 released, R 117-129, out clean |
| HOME lost | 24/24 seated (it re-claims); R up to 333 (day-1's one-off consequence row) |
| won't sleep (by day), alarm clear | 23/24 released, RIDER_ALARM set; 1 seated (wandered into the pit) |
| won't sleep (by day), alarm set | 23/24 held, 1 seated |
| waker visible, release at F+150 | 24/24 seated, confined to the pickup cell |
| waker hidden after the wake, release at F+150 | 24/24 back in bed 100 gt after the wake |

Trip (8 seeds x switch speeds 0.1-0.4): 32/32 seated; the switch to the entry 63-70 gt, to R
257-269, to the re-entry's waiting cell 320-332; a freeze of 400 gt mid-spiral: 128/128 seated,
no stall. The legs share no cell.

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged on the real server; the design must
  not rely on it (the waker-visible row is the design).

## Open

- The divert cannot lift an EMPTY cart (drag 0.96 a tick against 0.997 ridden) with the booster
  count that keeps a ridden arrival in the park's range (entry 0.42-0.44 stored); options (a) a
  stalled empty rolls back to the tail, (b) a detector-released stop at the divert's top, (c) a
  three-cell park are with ironspec.
- Two tail posts within 6.05 of the legs ((9,44,0), (8,42,9)): onechunk moves them.
