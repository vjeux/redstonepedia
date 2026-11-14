# loaderlay: the loader layouts for tick-exact's N = 1 / 2 / 4 rows (Nov 13 2026)

Lane loaderlay (replacing onechunk for this item; router farmopt ca87cf44). Branch `explore/farmopt-loaderlay`.

## The defect (tick-exact's 16:41 N = 4 row) and the fix (onechunk's rule)

Both loaders (the brief's loader of record, `wip/onechunk/loader/l1-item-loop-bench.txt` as landed in
59d96295d, and tick-exact's `loader-clock.txt`) fire from overworld column 1 and Nether column 3 of
their 5-wide portals. loader-long's composer puts the twin's west end at floor(anchor / 8). The twin's
firing column ×8 then always lands 24..28 blocks east of the overworld portal's west end, while the
portal is only 5 wide, so no offset puts the return target inside it. In the 16:41 layout the copies'
lit pads at x 36..45 sat nearer, and every return went there (tick-exact: PORTAL tickets at overworld
chunks (2,0) and (2,2), none at (1,1)).

The fix keeps every cell of both sides as bridged. Only `port nether-anchor` and `port nether-axis`
move, 24 west. loader-long's composer gives floor((a - 24) / 8) = floor(a / 8) - 3, so the Nether
part moves 3 west whole. The layout also puts the overworld portal's west x at 1 mod 8 and its z at
3 mod 8. Then:
- the forward leg's exit block is the twin's own column 3 (distance 0);
- the return leg's exit blocks fall inside the overworld portal, within 1 in z over one move and
  within 4 over the two-move allowance;
- each item comes out at the same column / row / depth of its partner as on the bench, so each pocket
  sees what it saw.

Files (this directory):
- `loader-x8.txt` — the loader of record with its anchor moved (md5 of the source 7152e5f02884).
- `loader-clock-x8.txt` — tick-exact's loader-clock with its anchor moved (source md5 f97bad1379c5).
- `control/loader-clock-1641.txt` — the 16:41 control's loader, byte-identical to tick-exact's
  artifact 1485301936870056 (md5 f97bad1379c5).

The module for every row is rev 3: `designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt`
on explore/onechunk 685729e50, md5 3d7ed77b4bf3. Its pads are its overworld `nether_portal` cells: two
10 × 3 portals, axis x, at local x -2..7, y -8..-6, z -5 and -2. Its 16-cell Nether part is the pads'
2 × 3 twin placeholder (nether layers 64..68, y base 64). loaderlay claimed that part for these
layouts (farmopt 22:35).

## The layouts (tick-exact 22:02: the loader of record at N = 1, 2, 4, then loader-clock-x8 at N = 4 beside the control)

Copies at (6,80,6), (38,80,6), (6,80,38), (38,80,38), unmirrored, module Nether y base 64. N = 1 is
the first copy and N = 2 the first two.

| row | loader | loader offset | overworld portal | Nether twin | PORTAL chunks (ow / Nether) | control |
|---|---|---|---|---|---|---|
| N = 1, 2, 4 | loader-x8.txt | (25,80,19), Nether y base 0 | x 25..29 y 90..94 z 19 | x 0..4 y 90..94 z 2 | (1,1) / (0,0) | (3,3) |
| N = 4 pair | loader-clock-x8.txt | (10,85,-14), Nether y base 0 | x 25..29 y 90..92 z 19 | x 0..4 y 90..92 z 2 | (1,1) / (0,0) | (3,3) |
| control | control/loader-clock-1641.txt | (6,80,-10) | x 21..25 y 85..87 z 23 | x 2..6 y 85..87 z 2 | — | — |

loader-clock-x8 sits at y 85, not 80. At y 80 its Nether part (y 83..88) would take the copies'
twins' cells at y 83..84 (their frames are at y 80..84).

The tool prints each row's loader-long command line (`--chunks`, `--nchunks`, `--portal`,
`--nportal`) and the results (see "Verdicts").

## The tool

`mctas mobfarm loader-layout --module M --module-offsets "x,y,z;..." --loader L --loader-offset x,y,z
[--module-nether-ybase 64] [--loader-nether-ybase 0] [--plant DIM:NAME=box]... [--plant-unframed
DIM:NAME=box]...` (crates/mobfarm/src/loaderlayout.rs). It lays the parts as loader-long lays them
(mobai freeze::recipe_part). The portal search follows tick-exact's 26.3 pins (22:07): exit block
BlockPos.containing(x·s, y, z·s); every lit portal cell within 16 into the Nether and 128 into the
overworld, in x/z at any y; least integer distSqr, then the lowest y. It checks and prints:
- every portal and its frame; a frame broken in the built layout is a FAIL, and the search skips that
  portal;
- the cells two parts claim;
- both loader legs, from the firing droppers' code-derived crossing box (one move = the code; the
  verdict searches one and two moves);
- every pad's golem leg (centre over the cells grown 0.7, all rows) against its own twin;
- ties at the same y (FAIL), exit blocks decided by the lower y, and empty searches (a new portal:
  FAIL);
- each leg's exclusion zone (the search square, and the capture box);
- where the items come out (the relative mapping);
- the chunks and their levels from the two PORTAL tickets: a chunk with a rail or an entity must be
  at <= 31, any other at <= 32;
- the control chunk;
- the loader's portal cells and frames in the farm frame;
- the loader-long command line.

Controls:
- the 16:41 layout FAILS by name;
- `--plant` (framed) must move picks, and is read back;
- `--plant-unframed` is skipped, and its framed twin must move picks;
- a plant that moves nothing refuses;
- unit tests for the tie rule, the lowest-y rule, the empty search, a frame-breaking clash, zero
  samples (a leg with no exit block, a copy with no pad, a bad margin), and the chunk bars'
  negatives (a rail or an entity at 32, a cell at 33, a farm cell in the control chunk, picks over
  two chunks, NOT JUDGED with a failing loader leg).

## Verdicts (`mctas mobfarm loader-layout`, this tree; the commands and outputs under "Runs")

`loader-x8.txt` is the candidate loader of record. It cites as the loader of record only after
tick-exact's N = 2 / 4 game rows hold (ironspec, via G1 00:46).

- **The 16:41 control on rev 3** (copies as above, `control/loader-clock-1641.txt` at (6,80,-10)) FAILS by name:
  - the return leg: 22 of its 24 exit blocks pick copy4.pad1 (19) and copy2.pad2 (3); over one move
    all 6 go elsewhere (copy4.pad1 4, copy2.pad2 2). The tickets go where the picks go: overworld
    chunks (1,1), (2,0) and (2,2). tick-exact's run showed (2,2) and (2,0). With a loader leg failing,
    the chunk levels are NOT JUDGED.
  - a clash, a fault of the old layout: the loader's Nether hopper at (2,84,0) replaces copy1's
    twin's top frame obsidian (the loader is laid after the copies). The game breaks that twin, and
    copy1's golems then go to copy3's twin, or to a copy2 / copy3 tie. (The tool treats the broken
    twin as gone: the game drops it at its first neighbour shape update.)
  - the forward leg passes (distance 0, margin 2.83).
- **The loader of record, `loader-x8.txt` at (25,80,19): N = 1, 2 and 4 PASS.**
  - The forward exit block is (3,94,2), the twin's column 3: distance 0, margin 11.22.
  - The return over one move lands at x 27..28 y 94 z 18..20, distance at most 1.00, margin 24.16
    at N = 4. Over two moves (x 26..29 z 18..23) the distance is at most 4.00, margin 19.43 / 24.40
    / 26.50 at N = 4 / 2 / 1.
  - Where the items come out:
    - forward, in loader.nether: column 1.397..1.603 of 5, row 4.400..4.607 (the top row), depth
      0.397..0.703;
    - return, in loader.overworld: column 3.397..3.603, row 4.400..4.607, depth 0.297..0.603.

    This is the bench's relative mapping, unchanged.
  - Every pad lands on its own twin. Least margins: 0.84 (copies 1 and 3) and 0.16 (copies 2 and 4,
    whose next portal is copy1's or copy3's twin).
  - No clash, no broken frame.
  - Chunks: the overworld PORTAL chunk is (1,1) and the Nether's (0,0).
    - Every overworld chunk with a rail or an entity is at 30 or 31.
    - Rev 3's only cells past its chunk are 2 glass at local (-5,-27,10) and (-5,-26,10). For copies
      3 and 4 they sit in chunks (0,3) / (2,3), at level 32 (block-ticking: static glass).
    - The Nether chunks are (-1,-1), (-1,0) and (0,-1) at 31, and (0,0) at 30.
    - The control chunk is (3,3).
- **`loader-clock-x8.txt` at (10,85,-14), N = 4: PASS.** Forward distance 0, margin 7.35. Return:
  over one move distance at most 1.00, margin 21.11; over two moves at most 4.00, margin 16.12.
- **Controls on the N = 4 layout of record:**
  - `--plant overworld:near=27,96,23,28,97,23` reads back 12 of 12 cells, moves 8 picks and FAILS.
  - `--plant-unframed overworld:bare=27,96,23,28,97,23` (the same cells) reads back 4 of 4. It is
    skipped (its frame sides are air), and its framed twin moves 8 picks.
  - `--plant overworld:far=500,100,500,501,101,500` is refused as a no-op.
  - `--plant overworld:lowy=27,88,23,28,90,23` moves 2 picks, both decided by the lowest y: distance
    4 against the portal's 4, y 90 below 94.
  - Refused by name (unit tests): a negative or non-finite `--golem-margin`, two plants of one name,
    an empty `--module-offsets` entry, a dropper firing up or down into a portal. A copy with no pad
    FAILS NOT JUDGED, and a leg with no exit block never passes.

## Runs (the binary built from this commit; the files at these md5s)

- module: rev 3, explore/onechunk 685729e50
  `designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt`, md5
  3d7ed77b4bf3 (the runs read a copy of it at that md5);
- loader-x8.txt md5 15fa75750894, loader-clock-x8.txt md5 c2cd0f8674c2,
  control/loader-clock-1641.txt md5 f97bad1379c5.

`L="mctas mobfarm loader-layout --module $MOD"`:
- `$L --module-offsets "6,80,6" --loader wip/loaderlay/loader-x8.txt --loader-offset 25,80,19`
  (N = 1), the same with "6,80,6;38,80,6" (N = 2) and "6,80,6;38,80,6;6,80,38;38,80,38" (N = 4);
- `$L --module-offsets "<N = 4>" --loader wip/loaderlay/loader-clock-x8.txt --loader-offset 10,85,-14`;
- `$L --module-offsets "<N = 4>" --loader wip/loaderlay/control/loader-clock-1641.txt --loader-offset 6,80,-10`;
- the N = 4 layout of record with each control flag above.

## loader-long command lines (tick-exact's rows; the module from explore/onechunk 685729e50)

`MOD=designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt`; AFK spots as in
tick-exact's 16:38 / 16:41 rows (they clear every part): farm (8.5,113,8.5) at N = 1, (24.5,113,24.5)
at N = 2 and 4; Nether (4.5,85,9.5). Each row's control chunk is (3,3). Add `--loader-only` while the
record's own SHAPE settle differences stand (tick-exact 22:02 (2)).

- N = 1: `--module $MOD --module-offsets "6,80,6" --loader wip/loaderlay/loader-x8.txt --loader-offset 25,80,19 --loader-nether-ybase 0 --chunks "0,0;0,1;1,1" --nchunks "-1,-1;-1,0;0,-1;0,0" --portal 1,1 --nportal 0,0`
- N = 2: `--module-offsets "6,80,6;38,80,6"`, `--chunks "0,0;0,1;1,1;2,0;2,1"`, the rest as N = 1
- N = 4: `--module-offsets "6,80,6;38,80,6;6,80,38;38,80,38"`, `--chunks "0,0;0,1;0,2;1,1;2,0;2,1;2,2"`, the rest as N = 1
- N = 4 pair: `--loader wip/loaderlay/loader-clock-x8.txt --loader-offset 10,85,-14`, the N = 4 chunks
- control: `--loader wip/loaderlay/control/loader-clock-1641.txt --loader-offset 6,80,-10`, the N = 4 copies

## Fault note: a farm chunk unloaded while world time runs (G1 00:16, via farmopt)

This is not normal operation in this design. The loader keeps every route chunk entity-ticking with
nobody nearby: tick-exact's N = 1 row held 48,000 gt through a restart and a login. World time also
stops with the server. So a chunk unloaded while time runs means the loader failed or was mis-built.
What the player sees: the farm stops while those chunks are out. Nothing in the design signals that
the loader stopped: no lamp, no sound, only a farm gone quiet. The relink cost, as G1 states it,
is pending tick-exact's unload-and-reload fault row (night vs day reload, HOME and the bed's ticket
printed), which this note will cite when it reports:
- each rider whose CANT_REACH stamp was armed at the unload comes back with the absence as its age;
- its first HOME check erases HOME;
- under survival linking only one villager re-claims the bed, and the rest need the relink procedure.

## For vcompose's site record and the build guide

- The loader of record in the farm frame (world less copy 1's (6,80,6); copy 1's anchor (-6,0,-6)
  sits at a world chunk corner, and the Nether image depends on world x / z mod 8): offset
  (19,0,13).
  - Overworld portal cells (farm frame): x 19..23, y 10..14, z 13 (world 25..29, 90..94, 19).
  - Obsidian frame (farm): (19..23,9,13), (19..23,15,13), and (18,10..14,13), (24,10..14,13), no
    corners.
  - Nether portal cells: x 0..4, y 90..94, z 2. Frame: (0..4,89,2), (0..4,95,2), (-1,90..94,2),
    (5,90..94,2), in the Nether's own coordinates for the farm placed so.
- Portal exclusion zones (no other lit portal there, at any y):
  - Nether, around the loader's forward exit: x -13..19, z -14..18.
  - Overworld, around the return's exits: x -102..157, z -110..151.
  - Nether, around each copy's pad exits: copy1 x -16..17 z -16..16; copy2 x -12..21 z -16..16;
    copy3 x -16..17 z -12..20; copy4 x -12..21 z -12..20.
  - The capture boxes (where a foreign portal would actually win): return x 22..33 y 90..98
    z 14..27; forward (3,94,2) only.
- Information: something re-entering a copy's twin from the Nether can come out at the loader's
  overworld portal. Of each twin's 1,680 exit blocks, copy1's 149, copy3's 100, copy2's 8 and
  copy4's 1 pick it. Nothing in the design crosses that way. A Nether mob in a twin would.
