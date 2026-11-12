# S3 size files (the double stand's whole-assembly box)

On main (Nov 11, the S3 runner's merge): c24pad.txt, s3v27upw.txt and s3-wiring-v27p.txt below (and
the scratch s3-wired-pair-c-m21.txt and s3-wired-pair-c-mirror-x5.txt) stay on explore/farmopt-s3
(60adb9e8c1). The files kept here were re-saved with `mctas mobfarm paste <file> <file> --box
9999,9999,9999,9999,9999,9999 --overworld-only --out <file>`, which settles each rail junction to
the shape its as-placed power gives (router S stored north_south, settled south_west; c24pad-
understand's (-6,3,31) and (-6,3,34) south_east), so they pass main's junction check. Their runs
read the same as before (identical drive logs, meter rows on s3-one-h0-a and the wired pairs, a
wired run on c24pad-understand).

Priced with `mctas circuit memory price <files>` built from 9db2fcda6 (origin/explore/day1-sched-wip),
defaults (`--ground -1 --stage middle`, no terrain sample). One tool, one commit, every row.

| file | what | blocks | box | volume | build+dig min |
|---|---|---|---|---|---|
| c24pad.txt | today's stand, far-end siding: day-1's generator at 9db2fcda6 (below) | 3,252 | 21x30x79 | 49,770 | 638.3 |
| c24pad-understand.txt | the same with the far-end siding removed and siding (b) under the stand | 3,330 | 21x35x64 | 47,040 | 698.6 |
| s3v27upw.txt | S3-V27: stand B under the queue, plus the port-routed wiring, under-stand siding | 4,317 | 21x37x64 | 49,728 | 819.2 |
| s3-wiring-v27p.txt | the wiring part alone (straight runs, `mobfarm lines`, not a built design) | 569 | | | |

## How each was made

- c24pad.txt, from a 9db2fcda6 worktree: `env MCTAS_FAR_B_ONLY=1 MCTAS_SIDING_PROBE_MORE=1
  MCTAS_STAND_REL_SHIFT_RT=0 MCTAS_MODULE_REL_SHIFT_RT=0 MCTAS_CHAIN_HEAD_RT=2 MCTAS_REL_EXIT=1
  MCTAS_C24=1 mctas circuit memory release-gen --loop
  designs/farms/iron/parts/carousel-release/wip/pocket-loop-8481d3789-stand-wired.txt --siding
  designs/farms/iron/parts/carousel-siding/carousel-siding-wired-alone.txt --siding-ctl
  designs/farms/iron/parts/carousel-scheduler/carousel-loop-scheduler-v0.txt --pad
  designs/farms/iron/parts/carousel-release/wip/pad-a247e2fc2.txt --out c24pad.txt`.
  It reproduces day-1's row exactly (the positive control).
- c24pad-understand.txt: `mctas mobfarm paste c24pad.txt siding-b.txt --remove-box -9,-19,58,11,10,80
  --at 2,-21,0 --overworld-only --set-port home=2,-21,2`, where siding-b.txt is
  designs/farms/iron/parts/carousel-siding/carousel-siding-b-wired-alone.txt at 034d85af9
  (explore/farmopt-station). The bed head lands at (2,-21,2); the siding keeps its local z >= -4.
- s3v27upw.txt: `mctas mobfarm paste c24pad-understand.txt c24pad.txt --box -4,-3,-6,11,6,16 --at
  0,-27,32 --overworld-only` (stand B: c24pad's stand area, its circuit, approach and departure
  stubs, pad and zombie post), then `mctas mobfarm paste <that> s3-wiring-v27p.txt --overworld-only
  --keep-base` (62 route cells kept as the base's). The wiring ends on day-1's head ports
  (input.golem (-2,-5,20), output.go (-2,-5,26), stand_rel (6,-5,26), expire (-1,-5,22)).

## Not in these files

- Siding (b)'s track junctions to day-1's loop (an estimate, about 140 blocks, in every
  under-stand row).
- B's return re-laid to R1's take-off at y 13 (the 57-cell route) and the per-stand REL delay
  lines to R1's G0 (about 2 x 40 blocks plus B's line to R1).
- The four frozen-safe edge lines per stand to day-1's head block (pad, SE, Darr, Dh1, Dh3).

## The S3 bench

The stand rows (H's band, the T bar, the virtual stand B) run on C, not on these files:
`mctas mobfarm queue-loop-run <C> --release-stagger 4 --skew 2,2,2 --pocket-release 13 --cycles
8 --verbose --meter-hold 14,14,0,0,20,0,0,-1,1,13`, where <C> is
designs/farms/iron/parts/carousel-station/wip/approach-c-border-flat.txt at 9fb706996.

## The wired second stand (s3-wired-pair-c.txt)

C with a real stand B in place of the virtual one, for every S3 row from now on (ironspec 11-11:
the virtual B is retired):
- stand B: C's stand area (loop x -5..11, y -6..8, z -2..17: hold, climb, flat run, stand, its
  circuit, departure) pasted at A + (0,-27,+32), its three pocket carts removed (21 carts in all);
  B's flat run cut at loop z 44;
- the router S: C's flat rail (-3,1,16) made a T (north, south, west): south_west unpowered (to B),
  north_west powered (to A), through the stone under it from a repeater at (-2,0,16) facing east;
- B's branch: the curve (-4,1,16) (south_east), 26 slopes down x -4 (z 17..42), a flat at
  (-4,-26,43), the U-turn (-4,-26,44) north_east and (-3,-26,44) north_west into B's flat run
  heading north; glass under every rail;
- not yet: B's return to the tail (B's departing carts leave the loop), the tripwire interlock,
  DEP_X, B's pad and posts.

Generation (scratch files, world = loop + (6,0,6)): `mobfarm paste C C --box 1,-6,4,17,8,23 --at
0,-27,32 --overworld-only`, the removals `--remove-box 3,-27,50,3,-26,55` and `3,0,22,3,0,22`,
then `mobfarm lines --rail '2,0,23>2,-26,49' --block '...'` for the branch, the curves and the
router cells, pasted on; B's three pocket carts' entity lines deleted.

Run: `--meter-hold D12,D23,CLOSE,DEP,HOLD_ON,CHAIN,FALLBACK,MISS_D1,2,OPEN_LEN` (VB 2: the wired
pair). The bench sets the repeater's input each GO (a redstone block on even cycles: A; glass on
odd: B), runs B's meter at its offset on the odd cycles' REL, and prints B's lines with `B: `.
Each stand's summoning-moment line gives each train's Dh3 against the run's own summon tick (world
time a multiple of 100) before that stand's REL, and the carts at rest in its pockets there.
