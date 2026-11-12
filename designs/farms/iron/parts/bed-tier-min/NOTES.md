# The minimal bed tier (the napper stop)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-strand`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), day-1 (a75cc97c), onechunk (7d3d2afb),
farmopt (ca87cf44); the lead (F1 + Q2, 24 villagers, one bed) places it under the station with
9fc3155f's budget (loop frame = this file's local frame + (0,-18,-1)).

## Files

- `bed-tier-fold.txt`, `-bench.txt`: the stop as a family-agnostic part (47bbde753): 167 blocks,
  25.1 build minutes at middle prices, the kick clock not built. The bed's head at local (-3,0,4);
  blocks x -6..4, y -2..3, z -5..8; riders reach y 4.26 over the entry.
- `bed-tier-fold-trip*.txt`: C′'s legs (divert spiral, return helix) from a5fb7eafb, laid against
  the old exit; stale since R moved to (4,-1,5).
- `bed-tier-min*.txt`: the linear tier (37099474d), superseded.

## How it works

The arrival heads west along z -4: a booster entry (3,2,-4), an always-on activator (the rider lands
on the ledge south of it), two braked powered rails down a slope (the park), a flat, the curve south,
a 2-level hump (slopes up, two raised capped flats at y 2, slopes down), the north T (-2,0,3), the
pickup cell (-2,0,4) beside the bed's head, a capped flat, the curve east, the slope down (-1,-1,6),
a flat, the south T (1,-1,6), the exit north and east to R (4,-1,5).

- Normal operation only (vjeux 2026-11-12): no handling of empty arrivals, dead nappers or the like.
- The napper walks the ledge onto the bed's roof and sleeps; F (the hatch shut: the zombie seen) 10
  live gt after the lie-down, the release (the park's supports powered) 30 live gt after F (the bench
  emulates both; on the build F is the bed observer's lie-down edge, a one-shot, never inside the
  observer's ~4-gt blind window).
- Woken, it stands only on the pickup cell; the released cart reaches it at 0.10-0.15 b/t and seats
  it (a cart into a standing villager at ~0.2 b/t or more stops dead).
- The reload strand: a save while the released cart rolls and the napper stands awake puts the napper
  back to bed on load (SleepInBed's cooldown is not saved), and the cart passes it. Fixed by two
  dead-end spurs and a plain kick clock, no bed-state memory (an observer ignores changes while its
  pulse is pending, so no latch on its pulses survives a load; beds give no comparator output):
  - the north T, unpowered for good (south-east): southbound carts pass to the pickup cell (the hump
    gives back the ~30% a straight pass through its curve costs), any northbound cart turns into the
    north spur (dead end (0,0,3));
  - the south T, powered (north-west, the exit) only by the rider strings (A at y 1 over the curve
    and the slope, B at y 0 over the T: at y -1 a seated rider's box tops out at y 1.0; dust
    (2,0,6..7) beside it): ridden carts turn north to R, unridden ones into the south spur (dead end
    (1,-1,8)), so R never sees a strand;
  - the slope: a braked powered rail on string OR kick, a second hold for a strand cart;
  - the kick clock (80 gt, 40 on) powers both spur rails and the slope: a cart kicked off the stone
    runs back through the pickup cell into the north spur and back until the string sees the napper.

## Bench rows (8 seeds x arrivals 0.1/0.2/0.3, no give-up)

| row | result |
|---|---|
| night | 24/24 seated; the south T's lead over a ridden cart 17-27 gt; entry to R 218-236 gt; lie-down from the entry 80+ gt |
| reloads +100..+220 every 2 gt, freezes +100..+220 every 4 gt (2,208) | all seated; 0 strands to R, 0 never out, 0 ridden in the south spur, 0 HOME lost; 37 heals, extra 143-156 gt; least pickup 0.106 |
| reloads at 6 phases, a freeze at load+0..40 every 2 gt (3,168) | all seated, same zeros; 11 heals 143-156 gt |
| give-up only (bedtier-strand, rejected) | 65 of 216 strands, every one losing the next napper's HOME |

Open: 74 thaw rows show the south T re-powered in the tick a cart, frozen on the flat before it,
entered it (lead 0-1 gt, routed right): a second flat with B over it would give the lead back.

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged; the design must not rely on it.
