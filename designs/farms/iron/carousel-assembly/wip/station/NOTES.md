# The F1 + Q2 station (dfb89499)

Three level pockets under M1 vertical drops, the golem pad under the summoners' overlap, in
9fc3155f's budget (explore/farmopt-c3-borders `wip/c3/f1q2-budget.txt`; the station's box loop
x -3..9, y -11..19, z -6..0). Loop frame; the bench files are written at file = loop + (6,0,6).
Bench rows from `mctas mobfarm drop-bench --layout station` (labelled bench: the controller and the
lanes' feed are emulated until the circuit is laid).

## The lead's flags

- Mode: `--l1 --open-delay 4` (the controller and its timers on game ticks through anything; the
  hold's release at the controller's stage 3, 4 gt after its gate). Rows without `--l1` are L2 (the
  live clock), for the pearl alternative only.
- The build: `--slab-stop --post 8,16,0` (each hold's north stop a bottom slab; one zombie post at
  loop (2,16,-6)).
- Judge line (cc767b78's L1 judge runs it):
  `drop-bench --layout station --pad --hold-y 16 --stagger 0,6,12 --rebrake 16 --rebrake-each --release 3 --exit-gate --open-delay 4 --pulse 12 --refill 2,10 --l1 --post 8,16,0 --slab-stop`
  (the trapdoor pulse 12, cc767b78's edge bar: its lead-row threshold is 5..6 gt, so 8 left 2..3).
- Restarts at every phase (ironspec's reload-order rule): `--restart-sweep 0,130,1` (each tick, in
  the save order, its reverse and 8 seeded orders: cc767b78's `liveness::restart`). The cold start
  is the fresh design's row: `--dep-riders x,x,x` (G1). `--restart` with `--freezes`/`--login` is
  refused (a restart's unload resets the freeze's entity box; freeze rows are the pearl
  alternative's only).
- The committed design (onechunk's module): `f1q2-station.txt` here, in the loop frame, written by
  the judge line with `--write-design` (the build, the start state's carts in the pockets and the
  holds, `positions:`, the runner's `run-cells:`, the event table in its header; the lanes' R_1
  carts and R_2 are the lanes' file's). `mobfarm exit` on it: 48 spawn spots, all on the pad, 432
  golem runs every one through a portal.

## Cells (loop frame)

- Pockets: north_south powered rails at (-1,0,0), (3,0,0), (7,0,0) on switched supports (x,-1,0);
  the launcher stone (x,0,-1) with an unattached string on it (x,1,-1) (no golem spot, passes
  sight).
- Pocket strings (x,1,0), hooks (x+-1,1,0), glass (x+-2,1,0).
- Departure: plain (x,0,1), the exit detector (x,0,2) (plain rails north and south, air east, west
  and above, glass below: no powered rail, gate or junction on a face), plain (x,0,3), boosters
  (x,0,4..6); the merge row at z 7 (7b90c3f1's, shifted from z 8 for the strict pitch 32). The
  merge row's entry speed: 0.83 b/gt.
- Holds: bottom-half trapdoors at (x,16,0), carts at y 16.1875 (riders' feet 16.19); the ring's
  glass at (x+-1,16,0) and (x,16,+-1); the north stop a bottom slab at (x,17,-1); the hold string
  (x,17,0), hooks (x+-1,17,0), glass (x+-2,17,0).
- R_1 (x,17,1): a powered rail ascending south on the ring's south glass, side-powered at
  (x+1,17,1) (glass off, a redstone block on; glass over it at (x+1,18,1)); R_2's rail (x,18,2).
- Pad: x-axis portals x -2..7, y -8..-6 on rows z -5 and -2; obsidian frames at x -3 and 8,
  bottom y -9, top y -5; glass over the frames at y -4; cobblestone floor at y -7 on rows z -6,
  -4, -3, -1 (x -3..8): golem feet -6, 48 spots (P 0.993); every golem through a portal.
- The hold post: its boat at (2,16,-6), glass (2,15,-6), leaves (2,19,-6).

## Events (L1, game ticks; T the summon tick)

1. Pocket k's support powered at REL + offset k (x -1: 0, x 3: 6, x 7: 12: the reverse merge
   order), unpowered (the re-brake) 16 gt later.
2. Pocket k's gate: its exit detector rises after REL + offset k, or its string is off at REL +
   offset k (an empty pocket: the cold start). Its trapdoor opens 4 gt later, not before T+3.
3. Each trapdoor shuts 12 gt after it opens (clean 3..20; cc767b78's lead rows 6..20+).
4. All three shut: +2 gt, every R_1 side-powered for 10 gt (>= 6): the next batch rolls into the
   holds.

## Rows (bench, tool reviewed by G1 through be06f9bd6 and 0f8207674; pulse 12)

- Normal running, REL T+3..7 x 40 seeds (200 runs): at rest at least 34 gt before T+100, the
  landing lead over the re-brake at least 31, launch spacing 6; 0 wrong pockets, contacts, riders
  hurt, stuck carts, trapdoors shut onto a cart. The last trapdoor shuts at T+40..44 (4 gt later
  than at pulse 8), the R line from T+42..46, the next batch at rest in its holds by T+56..60.
- Restarts at every tick T+0..130 x 10 orders x REL 3/5/7 (3,930 runs): the same rows.
- The cold start (pockets empty, the fresh design), REL T+3..7 x 40 seeds: every trapdoor opens on
  the empty-pocket evidence; at rest at least 42 gt before T+100; the R line from T+34..38.
- The re-brake window (L1): clean at 6..47 (<= 5: the departing cart braked before it leaves; >=
  48: the arriving cart lands on a powered pocket); 16 keeps 10 above the low edge, 31 below the
  high.
- Night panic with the post and a pinned pillager: every hold covered at every tick, the pockets
  and the departure too; one 7-gt calm stretch in the fall (for the stamp check). The nearest
  zombie to any villager 6.08 (bar 6.05: a falling rider in the x 3 shaft, feet to the post's
  seated zombie as `fleecheck::zombies_seated` places it); the napper passes the zombie at 7.0 on
  its way down (ironspec's call). Without the slab stop no legal post sees the x 7 hold.
- The fall behind the 6.08 (G1, ironspec): the closest approach is 6.0828 = sqrt(37), the
  horizontal distance from the x 3 shaft's centre line (loop 3.5, z 0.5) to the post's seated
  zombie (loop 2.5, z -5.5), reached at T+27 in the control's worst run as the falling rider's feet
  pass the zombie's seat height (feet 15.52, 0.67 below the hold: in flight, the fall's first
  ticks; the zombie at y 15.49). So it depends on the fall only through the cart staying on the
  shaft's centre line, which tick-exact's scene drop-bridge 131a9b2f2
  (designs/redsim/recorded/mobai/drop-bridge.rec, 26.3, 600 ticks, 0 differences: ridden carts from
  closed bottom-half trapdoors down 1 x 1 glass shafts onto braked flat and slope pockets from 14
  and 17 blocks) shows (0 spread in flight); a 16-block fall's in-flight positions are a prefix of
  the 17-block one's. One cell differs: the station's shaft is open (no wall blocks below the ring
  at y 16; the cart falls with no horizontal speed, its box 0.01 inside the column): tick-exact's
  ruling on whether that needs the exact x 3 scene.

## Open

- The pillager posts: 9fc3155f's search (seated eye, its cell's own blocks in the sight test);
  check the pockets' coverage against its exact cells (`--block`, `--scarers P:`).
