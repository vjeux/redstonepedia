# The minimal bed tier (the napper stop)

Code: `crates/mobfarm/src/bedtier.rs` (`mctas mobfarm bedtier`, `bedtier-bench`, `bedtier-strand`).
Owner: session 58f770cc. Rulings: ironspec (3883f332), day-1 (a75cc97c), onechunk (7d3d2afb),
farmopt (ca87cf44); the lead (F1 + Q2, 24 villagers, one bed) places it under the station with
9fc3155f's budget (loop frame = this file's local frame + (0,-18,-1)).

## Files

- `bed-tier-fold.txt`, `-bench.txt`: the stop as a family-agnostic part: 167 placed blocks, 25.1
  build minutes at middle prices. Not built (the bench emulates them): the kick clock, the south
  T's latch, the slope's OR, F and the release (see Signals). The bed's head at local (-3,0,4);
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
- Night only: the napper sleeps in the villager REST window (day time 12000 to 10 the next day), so
  the picker sends nappers from day time 12000 to at least ~240 gt before the window ends (entry to
  R 218-236 gt, a heal up to ~160 more); a napper entering by day does not sleep (not handled).
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
    (2,0,6..7) beside it) and by a latch they set and R's pulse resets (its output the T's support
    (1,-2,6)): ridden carts turn north to R, unridden ones into the south spur (dead end (1,-1,8)),
    so R never sees a strand; the latch keeps the T powered for a rider stopped on the flat under
    both strings (without it, 74 thaw rows had the T re-powered in the tick the cart entered it);
  - the slope: a braked powered rail on string A OR the slope's window, a second hold for a strand
    cart;
  - the kick clock (80 gt; the spurs' window 40, the slope's 60 from the same phase, so a cart
    kicked in the spurs' last tick still climbs the slope boosted), on the controller's game ticks
    (L1): a cart kicked off the stone runs back through the pickup cell into the north spur and back
    until the strings see the napper. A 60-gt period (40 on) was tried: a kicked cart then reached
    the standing napper at 0.212 b/t and stopped dead (84 race-window rows).

## Signals (the controller's, on game ticks; loop frame = local + (0,-18,-1))

| signal | source | target |
|---|---|---|
| F | the bed observer's lie-down edge + 10 gt | the hatch controls (-4,0,3), (-4,2,4) |
| release | F + 30 gt | the park supports (1,0,-4), (0,-1,-4) |
| spurs' window | 40 gt in every 80 | the spur supports (-1,-1,3), (1,-2,7) |
| slope | string A's hook (2,1,6) OR the slope's window (60 gt in every 80, same phase) | (-1,-2,6) |
| latch | set: hook (2,1,6) or hook (1,0,7); reset: R (4,-1,5) | the T's support (1,-2,6) |
| queue release | R's pulse sets "stop free"; the queued cart's drop resets it | the trapdoor at loop (9,-8,-6) |

## Bench rows (8 seeds x arrivals 0.1/0.2/0.3, no give-up; bench, tool unreviewed)

The lead's rows (ironspec via farmopt 2026-11-12 00:28: under L1‴ the farm never freezes; the
controller runs on game ticks; a save/stop/start at every phase; the cold start):

| row | result |
|---|---|
| cold start: night at kick phases 0..78 every 2 (960) | all seated; 0 problems; the south T's least lead 17 gt; pickups 0.143-0.206; entry to R 218-236 gt; lie-down from the entry 80+ gt |
| save/stop/start (an immediate restart) at +2..+260 every 2 gt x kick phases 0/27/53 (9,360) | all seated; 0 strands to R, 0 never out, 0 ridden in the south spur, 0 HOME lost, 0 T problems; 48 heals, extra 95-157 gt (median 122, p90 148); least pickup 0.140; least lead 17 gt |

Earlier rows (47bbde753 and before, freezes now for the pearl alternative only):

| row | result |
|---|---|
| reloads +100..+220 every 2 gt (400 gt away), freezes +100..+220 every 4 gt (2,208) | all seated; 0 strands to R, 0 never out, 0 ridden in the south spur, 0 HOME lost; 37 heals, extra 143-156 gt; least pickup 0.106 |
| reloads at 6 phases, a freeze at load+0..40 every 2 gt (3,168) | all seated, same zeros; 11 heals 143-156 gt |
| give-up only (bedtier-strand, rejected) | 65 of 216 strands, every one losing the next napper's HOME |

## Observed, unbridged

- In the hidden-waker control, the cart seated the sleeping napper off its bed: the sleeper drifts
  into the cart's grown box over the pickup cell. Not bridged; the design must not rely on it.
