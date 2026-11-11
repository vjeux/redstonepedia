# Golem-lock corners on the OLD TAIL — not counted for acceptance (ironspec 2026-11-11 08:35)

The 24-gt core's corners count only on R1's loop; these runs are on today's 18-slot loop with the
old tail (tail_desc), kept as the record of the late-corner failure and its cause.

Runner: main 24ee681d6 (tick-exact's golem-lock line) + `timeline --seed-to` (day-1 main-wt):
`mctas mobfarm timeline <file> --days 1 --seed 1 --seed-to 8 --tag "region station -3 0 -6 10 3 1" --to 2300`.
Bar: 0 station slots lost to outside golem locks.

Generator: explore/day1-sched-wip (this commit); base knobs `MCTAS_FAR_B_ONLY=1
MCTAS_SIDING_PROBE_MORE=1 MCTAS_REL_EXIT=1 MCTAS_C24=1 MCTAS_MODULE_REL_SHIFT_RT=0
MCTAS_CHAIN_HEAD_RT=2`, `release-gen --loop wip/pocket-loop-8481d3789-stand-wired.txt --siding
carousel-siding/carousel-siding-wired-alone.txt --siding-ctl
carousel-scheduler/carousel-loop-scheduler-v0.txt --pad wip/pad-a247e2fc2.txt`.

| file | corner | knobs | spawned of 23 (8 seeds) | slots lost | arrival margin min |
|---|---|---|---|---|---|
| ../c24-core-pad-wip.txt | new core nominal | — | 23 all | 0 | 13 |
| c24pad-late.txt | superset: every output +2 after the golem | MCTAS_GOLEM_EXTRA_RT=1 | 23 (seed 5: 22, no spot) | 0 | 11 (seed 5: 7) |
| c24pad-srl.txt | late: stand REL +2 alone | MCTAS_STAND_REL_SHIFT_RT=1 | 11/10/10/11/10/10/10/10 | 2/0/1/0/0/2/0/0 | -97 |
| c24pad-early.txt | early (b) = re-centered nominal: launch, advances, module +2 | MCTAS_RG_RT=2 MCTAS_CHAIN_HEAD_RT=3 MCTAS_MODULE_REL_SHIFT_RT=1 | 23 all | 0 | 11 |
| c24pad-rc-early.txt | re-centered early: launch, advances, module +4 | MCTAS_RG_RT=3 MCTAS_CHAIN_HEAD_RT=4 MCTAS_MODULE_REL_SHIFT_RT=2 | 23 all | 0 | 9 |
| mainpad-late.txt | MAIN's farm, stand REL +2 alone | main's release-gen, MCTAS_STAND_REL_TAP=4 (main nominal regenerates byte-identical to wip/carousel-loop-pocket-siding-wired-stand-pad-wip.txt) | 23 (seed 4: 22, no spot) | 0 | 13 (seed 4: 7) |

Outside locks (the judge's "10.6", a summon's 10-box) run ~33-41 per seed on every passing file,
main's own pad farm included (298 vs the new core's 300 over seeds 1..8): inherent to the farm.

## The late failure: the old tail at 24 gt (ironspec 08:37: the record)
Main passes the same corner, so it is the new core's. On the old tail the returning train comes to
rest at +133 against the new A1 at +132 (main's A1 +142: 9 gt to spare); a later departure pushes it
further behind. Runner on c24pad-srl.txt (`queue-loop-run --wired=-2,-5,20 --visits-observe
--cycles 30 --seed 1 --day-only`): the stand's margins hold for the first 7 slots (gap12 11, in
5..6, clear 1..2, arrive 5..6); the first problem is the register's tail: at slot 2 queue slots 3..5
hold 38/36/none instead of 40/38/36 (the advance moved slots 0..2 while the returning train still
settled); 1 of 29 slots clean (nominal: 59/59); the stand jams follow (touching departures at slot
10, cart 13 back through G0, carts 26/28 merged at (8.59,-2.53,0.5)).
