# The return's lift and split (7b90c3f1): the bench's numeric bars

`mctas mobfarm return-bench` (crates/mobfarm/src/returnbench.rs). Every number it enforces, with its
basis: **physical** (a measured failure boundary or a game rule) or **inherited** (a number carried
from an earlier ruling with no measured failure behind it). ironspec's bar audit (Nov 12 PT):
https://artifacts.internalmeta.com/a/1872157077300391

## Barred (a failing row)

| bar | value, flag | basis |
| --- | --- | --- |
| wrong lanes, pushes (box touches where a push acts), the napper off its route, riders hurt, batch carts on the hold's route, extra pulses | 0 | physical: the failure itself |
| the release bar: a pop without a departure, a launch before rest, a creep, a departure off an unfired pulse | 0 (`--pulse-gate` or `--day1-taps`) | rule (ironspec 11:43, 14:41) |
| the shaft-entry (foot) pairs, and the napper's foot gaps off path (a) | >= 16 | physical: the exit-top contact boundary 14 (13 touches: the leader drifts ~8 gt in the top cell, the follower rising at 0.7 b/gt catches it), + 2 (ironspec 19:38); the inherited 21 / 28 / 21 retired |
| path (a): every y-12 pair (lane 3 -> 7, 7 -> the napper, the napper -> the next lane -1, lane 7 -> the next lane -1 with no napper between) | >= 16 (`--y12-min`, `--y12-nap-min`) | physical: the column bar 14 + 2 (carts rise at the same 0.7 b/gt, so a y-12 gap is the gap at the top; lane 7's side entry clean at 13); final after tick-exact's foot-entry row and the bench's run of its lift-pair configuration (ironspec 20:14 PT); the inherited 23 (21 + 2) and window >= 46 retired |
| path (a): a cart missing y 12, or the column order wrong at y 12 | 0 | rule |
| path (a): lane 7's mouth clearance (its column entry after the preceding cart's box has cleared the mouth cell at y 12) | >= 2 (`--mouth-min`); unmeasured fails as "unmeasured" | physical: the out-of-order mouth contact (ironspec 20:01 PT (2)) |
| heads under the roof's underside | < 28 | physical: geometry |
| each lane's delivery entering its merge cell | 0.30..0.48 | inherited here: ironfarm's part spec |
| the settle lock k | 42 (`--pulse-gate 42`) | physical: the official approach-detector -> rest 36 (restart at the top exit) + 3 + 2 (ironspec 14:36) |

## Printed only

- The junction and K settle / clear margins, per switch cause with its phase: cc767b78's judge bars
  them (tick-exact's official boundary row: block phase 1 gt, entity phase 2, + 2). The arithmetic
  is `settle_margin` / `clear_margin` (its convention and a scripted-timeline unit test in the
  source): a cart recorded in a junction's cell for ticks e..=l reads it in e..=l, a switch written
  at s is read from s; settle = e - s, clear = s - (l + 1); a D press timed from its press tick adds
  + 1 / - 1. The bench's `--driver-latency` switches the detector-triggered ones at press + 4 (set)
  / + 6 (reset), day-1's driver.
- The single-occupant margin: 312 against the hold's longest wait + the pulse + clear (the + 20
  retired: ironspec 19:07 PT (5); the occupancy is measured directly).
- The window (lane 7 -> the next lane -1), the least distance in the column, the K window, the same
  pairs at the exit top (the contact point).
- The junction 8 was never barred here: it came from 1bd9b1ba1's sweep step, no mechanism (retired
  19:14).

## Stand-ins (G1's guard, ironspec 18:34)

The `# emulated:` header line of each file lists the bench's writes outside the file. The two
toggled inputs: the hold's (7,-1,8) a floor lever on glass (7,-2,8); lane 7's drive a ceiling lever
(-4,12,6) hung from the sign's glass (-4,13,6), as onechunk's record7 lays it (no new support: no
glass where the build has air). Both non-solid, no top face, no T in their update reach, powering
only their own rail or repeater. The slope rail's printed rises equal `--lane7-rel` in every row
(the positive check on the toggle convention).
