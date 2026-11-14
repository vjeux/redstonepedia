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
on explore/onechunk 685729e50, md5 3d7ed77b4bf3. NOT FOR BUILD: loaderlay's pinned run input; superseded
when the loader is re-judged on V rev 3.1 (its v-nonrail positions file exists). It is a TEST FIXTURE
(ironspec 10:59a (7), 9fc3155f 11:05): composed from /tmp inputs, it carries the (A) stop's bench
stand-ins, and `designs/settle-scope.txt` lists it as a `fixture:` line. Its pads are its overworld `nether_portal` cells: two
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

`loader-x8.txt` is the loader of record once this lands (G1 01:22, with tick-exact's game rows
below), for the loader itself. The rows ran `--loader-only`; the module's own update-stability is
V's item.

**The game rows (tick-exact 01:20, official 26.3 server, `mobai loader-long` with the lines below,
`--loader-only`; https://artifacts.internalmeta.com/a/3742200425927507):**
- **loader-x8 HELD at N = 1, 2 and 4.** Each row ran these phases:
  - 600 gt with the player at the farm;
  - 600 gt in the Nether at the twin;
  - 24,000 gt logged out, a restart, 24,000 gt more;
  - a login.

  In every phase every route chunk was entity-ticking on every tick (frozen 0), every Nether chunk
  was at <= 31, and both PORTAL tickets were on every tick's line (least ticks left 240 at N = 1 / 2
  while logged out). All arrivals landed in the loader's own chunks: overworld (1,1) and Nether
  (0,0), about 1,190 per side per 24,000 gt. The control chunk (3,3) was caught at 32, block-ticking.
- **loader-clock-x8 HELD at N = 4.** Its hopper clock runs about 196 gt: about 124 arrivals per side
  per 24,000 gt, least ticks left 103.
- **The 16:41 control failed as the tool reads it.** The overworld PORTAL ticket was never at (1,1),
  and arrivals were CAPTURED at (2,0) (1) and (2,2) (2): copy2.pad2 and copy4.pad1. The run then
  stopped on an unloaded Nether position.
- Rev 3 isn't update-stable as pasted: 15 SHAPE cells per copy (tripwires, hooks, 3 rails). The rows
  run `--loader-only`, and the loader bars don't read those cells.
- G1 01:22 cites loader-x8 as the loader of record once it lands. farmopt asked ironspec for the
  ruling at 01:20.

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

- module: rev 3, committed in this tree at onechunk's path
  `designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt` (md5
  3d7ed77b4bf3, blob 88eea3fa1d7c: explore/onechunk 685729e50's, byte for byte); not for build:
  loaderlay's pinned run input, a test fixture (the module section above);
- loader-x8.txt md5 15fa75750894, loader-clock-x8.txt md5 c2cd0f8674c2,
  control/loader-clock-1641.txt md5 f97bad1379c5.

`L="mctas mobfarm loader-layout --module $MOD --positions wip/loaderlay/rev3-nonrail.positions.txt"`
(every layout run carries rev 3's non-rail villager places):
- `$L --module-offsets "6,80,6" --loader wip/loaderlay/loader-x8.txt --loader-offset 25,80,19`
  (N = 1), the same with "6,80,6;38,80,6" (N = 2) and "6,80,6;38,80,6;6,80,38;38,80,38" (N = 4,
  with `--write-layout` for 9fc3155f's check on all four rods);
- `$L --module-offsets "<N = 4>" --loader wip/loaderlay/loader-clock-x8.txt --loader-offset 10,85,-14`;
- `$L --module-offsets "<N = 4>" --loader wip/loaderlay/control/loader-clock-1641.txt --loader-offset 6,80,-10`;
- the N = 4 layout of record with each control flag above;
- the redsim runs: each of loader-x8 at N = 1 / 2 / 4 and loader-clock-x8 at N = 4 with
  `--redsim 24000 --seeds 1,2,3` (the world open at t 12,000);
- the redsim controls: loader-x8 at N = 1, `--redsim 2400` with each `--redsim-control` (exit 1 each;
  `put-out-twin` on the N = 2 layout as well), and a control refused (exit 4).

Every run first passes the shared provenance check in mctas's dispatch (ironspec 01:58 / 02:35;
ironfarm's f99df6a0e, default-deny: `loader-layout` is evidence): the binary's stamp, the worktree
and the cited commit must agree. Every design file a run reads (`--module`, `--loader`,
`--positions`) must lie in the worktree and be the cited commit's blob at its path, else the run
refuses (G1 06:37 (6), ironspec 06:43); each run prints them with their blob ids. The outputs end on
the shared exit-code table's line: 0 PASS, 1 FAIL, 2 NOT MODELED (only loader-clock-x8's redsim row,
for the two named model limits below), 4 REFUSED (every refusal before a run, and a control that
planted nothing). The outputs, with their numbers, are the push's report artifacts. Results run before
the binary rule (the 02:03 / 02:04 lightning and redsim runs) cite nothing, and the e495c3d74 runs read
rev 3 from a copy outside the tree (G1 06:37 (6)): the runs of record are the ones citing the latest
commit.

## The lightning print (ironspec 01:17 (1b), G1 01:22 / 01:34 / 06:37; `loaderrods`)

Every `loader-layout` report reads the settled, built overworld through redsim's own
`findLightningRod`. That is tick-exact's 26.3 pin (01:28):
- a 3-D sphere of 128 around the strike column's MOTION_BLOCKING top;
- only rods topping their column;
- the closest by 3-D distance.

The site's surface is materialized as a ground plane in that world (G1 05:51): stone at the copies'
ground (copy y 80 + the module's ground layer -1 = 79; `--ground-y` sets it) under every column of
the strike area that holds no layout cell at or above it. A rod under the plane tops no column.

THE STRIKE AREA (ironspec 05:25 (iii)) is every chunk holding a layout cell, an entity or a villager
place, plus 4 columns beyond its edges. THE PLACES are:
- every rail cell (a rider seated in a cart there);
- every entity a part declares: rev 3's minecarts and the 20 oak boats that hold its scarers (a
  pillager or a zombie; ironspec 06:43: a bolt destroys a boat as it does a cart); loader-x8 declares
  none, and the redsim runner's pad golems are no part's (G1 08:37 (4));
- every place of `--positions` files in the module's written frame, moved onto every copy. The
  files are 9fc3155f's non-rail villager places (05:27b / 05:28, byte-identical to its artifacts):
  - `rev3-nonrail.positions.txt` for rev 3, the module these layouts use (a/1190241670091420, md5
    08c333129f01, 73 cells: the bed head, P, the set-down, (-2,-27,4), the lift (6,-28..-3,4), the
    way-in drop (9,-23..-15,-1), the line → queue drop (9,-12..21,-5));
  - `v-nonrail.positions.txt` for V (a/2417599305677157, md5 a7aba1db5440, 64 cells), for when V
    replaces rev 3.

The bars (each FAILS by name, each with a single-change twin among the unit tests, G1 06:37 (4)):
- (a) every strike-area column sends its strike to a credited rod (no column: NOT JUDGED, FAIL). So
  no strike lands on a column top, and none is redirected to an entity, anywhere in the area; each
  part's footprint columns are printed with the rods taking them and the worst column, and a part with
  uncovered columns needs its own rod;
- (b) every place is clear of every credited rod's own bolt box (the hit box around rodPos + 1). A
  villager converts whether it sees the sky or not, so there is no exposure test. The per-place
  table prints, by part, kind and level y, each place's columns within 3 with the rods taking them
  (inside the strike area, so covered when (a) passes), its clearance, and IN a box by the bar's own
  intersection test;
- (c) no credited rod's bolt box meets a loader cell (no loader part or cell: NOT JUDGED, FAIL);
- (d) every credited rod's chunk is loaded by the loader's PORTAL ticket (NOT JUDGED, so FAIL,
  without the loader's PORTAL chunk).

So a strike on a villager, a rider or a boat needs a second fault, a rod gone.

Each rev-3 copy carries V's rod at (-6,29,-6) in its own frame, so at any N each copy's strikes go
to its own rod; the loader's go to the nearest copy's.

THE ROD LIST OF RECORD (ironspec 05:27): copy k is the module at its offset, so its rod at the
module's (-6,29,-6) is at offset + (-6,29,-6). For copy1 that is 9fc3155f's rod at (6,80,6) +
(-6,29,-6):

| copy | offset | rod | chunk | level (the loader's PORTAL ticket at (1,1)) |
|---|---|---|---|---|
| copy1 | (6,80,6) | (0,109,0) | (0,0) | 31, loaded |
| copy2 | (38,80,6) | (32,109,0) | (2,0) | 31 |
| copy3 | (6,80,38) | (0,109,32) | (0,2) | 31 |
| copy4 | (38,80,38) | (32,109,32) | (2,2) | 31 |

N = 1 has copy1's rod only, N = 2 copy1's and copy2's. Each copy's places go to its own rod.

9FC3155F's every-place check on this 4-rod list met condition (ii) (06:00). It ran on the N = 4
layout file written by `--write-layout` (a/1711726887220920, md5 bef2972ae0d9; places
a/1425208226270087, md5 c8bc37b7e421), with the declared stand-in ground at y 79 (part 50
`--ground 79`; ironspec 05:52 accepted it as a model layer): 3,906 of 3,906 strike columns go to a
rod, the 4 strike targets are these rods, and its 972 riders + 68 entities had 0 struck, 0 hits, 0
fires: https://artifacts.internalmeta.com/a/4260158720941388 (md5 fc591dee3a67). Before the stand-in,
the file's empty columns sat at the world's bottom, so "every column to a rod" is claimed only with
the ground materialized (G1 05:51); this tool now materializes it itself (the ground plane above).
`--write-layout FILE --lightning-ready` writes the same layout with its three model layers declared in
the file's head: portal cells as air, the carts' and boats' Passengers lists dropped (their other tags
kept; at N = 4, 48 passengers from 48 vehicles), and the ground plane. An entity SNBT the stripper
doesn't read (a single-quoted string, an unclosed string or list) refuses the write by name (exit 4;
no input of record has one). It loads
in mctas without stand-ins and gives every strike column a top.

The two tables join copy by copy: the module check judges one copy alone, and this one adds the
neighbours, the loader and the ground between. `--write-layout FILE` writes the overworld layout as
one design (world coordinates; no Nether part), so 9fc3155f's `lightning-check FILE --rails
--positions=…` can run on all its rods. Each copy's places, moved by its offset, make that run's
positions file.

Thunder needs a player within 128 blocks (`ServerLevel.tickThunder`, docs/mechanics.md), and that
player's own tickets load the farm's chunks. With nobody online no strike falls at all.

What the check takes from the game. These are the 26.3 lines tick-exact quoted (05:26), which it
checked against the values used here item by item at 05:27, all correct:
- THE STRIKE: `ServerLevel.tickThunder` 550–583. While it rains and thunders, `nextInt(100000) == 0`
  per chunk tick picks a column (`getBlockRandomPos`), and the bolt goes to
  `findLightningTargetAround` at `atBottomCenterOf(pos)` (576) when `isRainingAt(pos)`.
- THE ROD SEARCH: `findLightningRod` 619–629 → `PoiManager.findClosest` 167–178 → `getInRange`
  97–102 → `getInSquare` 87–95.
  - `getInRange` keeps distSqr = dx² + dy² + dz² <= 128²: a 3-D sphere, never cut by getInSquare's
    square.
  - The center is the strike column's MOTION_BLOCKING top (`findLightningTargetAround` 632).
  - A rod counts only where `rodPos.getY() == getHeight(WORLD_SURFACE, x, z) - 1`, the topmost
    non-air block of its column. A rod under a roof, glass, a rail or a carpet catches nothing.
  - The nearest rod wins, and the strike goes to `rodPos.above(1)`.

  Redsim's `lightning::rods` / `rod_targets` keep exactly these; the print names each rod "tops /
  does NOT top its column". The credit is equal to the game's test, so stricter than or equal (G1
  05:26). A credited rod's chunk must be loaded (the POI search reads loaded sections): the print
  gives each rod's chunk and level, and a level over 33 fails.
- THE ENTITY TARGET (no rod in range): `encapsulatingFullBlocks(center, center.atY(getMaxY() +
  1)).inflate(3.0)` (ServerLevel 637), filtered to LivingEntity && isAlive && canSeeSky (638), one
  picked at random (640).
- THE HIT BOX: `LightningBolt.tick` 147–151 (143–157): x and z ±3, y −3..+9 around the bolt, on
  every tick while its life is >= 0, plus 1–3 re-strikes. Redsim's `lightning::hit_box_at`.
- THE CONVERSION: `Villager.thunderHit` 746–760. Every villager whose box meets the hit box converts
  to a witch unless the difficulty is PEACEFUL, with no sky or exposure test.

## The redsim runs (farmopt 21:44; `loaderrun`, `--redsim`)

The layout's portal skeleton runs in redsim, both worlds: the loader's cells whole, plus each copy's
portal cells with their frame obsidian.
- The loader's clocks start as the build starts them: an observer pair's O2 out and back, read back.
  A hopper clock starts as built.
- Each world's level random and drop seed are set before the settle.
- 3 golems go into each pad at the first tick at or after t 400 (the runner's own golems, no part's).
- A world open (`liveness::restart`, save order, 2 idle ticks) happens mid-run. The reload model
  keeps the Nether part in memory (unmodelled there). Its idle ticks advance game time 2 past the tick
  count, so the runner fires everything timed (golems, controls) at the first tick at or after its
  time, never on an exact tick that a world open could skip (G1 08:37 (1)).

The bars (each FAILS by name, G1 06:37 (1)-(3)):
- every item arrival at the loader's own portal;
- each side's worst gap within the 300-gt ticket (`TicketType.PORTAL`), the world open included;
- launches on both sides;
- the loop's items kept;
- golems placed (none placed FAILS; `--golem-at` outside the run is refused), every golem at its own
  copy's twin;
- the world open inside the run (`--world-open-at` outside the run is refused);
- no crossing whose copy is in neither world;
- no world error other than the two named model limits below.

Each bar has a failing control through the runner (unit tests; `--redsim-control`; G1 08:37 (2)), read
back:
- `stall-overworld-clock`: both overworld observers out a quarter in: the gap bars;
- `stop-nether-clock`: both Nether observers out before the run: launches;
- `steal-item`: one loop item in flight killed: the items;
- `double-item`: one more loop item in the overworld firing dropper a quarter in: the items, on the
  count (and it prints what a doubled item does to the loading: the crash note below);
- `divert-return`: a lit portal planted on the return's exit blocks one plane off the loader's portal:
  arrivals elsewhere;
- `put-out-twin`: copy1's Nether twin put out before the run: its golems cross to another portal;
- `drop-copy`: the copy of a loop item's crossing taken out of its world: a missing copy;
- `unseed-drops`: the overworld's drop seed unset a quarter in: its next spit is a world error NOTES
  doesn't name;
- golems due past the run (none placed) and the world open past the run (never reached).

A control that planted nothing, or whose own bar didn't fail, is REFUSED by name ("control planted
nothing", exit 4: never a pass; ironspec 08:37a's standing rule). So is every way a control can't
plant, and every run that can't start (no item in the loader's droppers, an O2 that doesn't read back,
two observer pairs on one side, a part or portal missing): exit 4 by name, never the FAIL code (G1
09:47 (1) / (2)). Unit tests may not spawn the binary, so the CLI's exit 4 for a refused run is shown
by the evidence runs (the refused stall control and the refusal runs).

The row says how it differs from the reference timing row (ironfarm's flags: ±2 gt per clocked event
and a world open at every phase): one world open, no event shift. The game's 01:20 restart rows
remain the loader's world-open evidence. loader-clock-x8's redsim verdict is "not modeled", and the
official game row (tick-exact 01:20) is its evidence. Two redsim model limits cause it, and these two
are the only world errors the runner names:
- its self-starting hopper clock fires in the Nether while `Run::start` settles the Nether part,
  before any caller can seed that world's dropper random. The spit is refused and its item lost; the
  runner counts only the spits recorded right after the start, and the items bar matches each such
  spit at a loader dropper to one item of the Nether side's shortfall (a dropper spits one item,
  redsim's redstone/container.rs 1319), read back side by side right after the start. Any loss no
  spit explains, on either side, or any surplus, FAILS by name (G1 09:47 (3), ironspec 09:47a (3)). A
  matched spit whose message the runner can't read (no dropper position in it), or at no loader
  dropper, FAILS by name and explains no item, so its loss surfaces as unexplained too (ironspec 10:35a
  (4) (3); the planted case is a unit test);
- redsim's light model lacks the extended sticky piston and the piston head.

A failed bar always FAILS (exit 1, by name). A run whose bars all pass with only those named limits
met judges nothing: NOT MODELED, exit 2 by name (ironspec 02:45 (3)). Any other world error FAILS by
name. So loader-clock-x8's redsim row (exit 2) is CITED FOR NO LAYOUT (ironspec 08:11a (6)): the
clock layout's evidence is its static layout run (exit 0) and tick-exact's 01:20 game row; the
redsim rows cited are the exit-0 ones (loader-x8 at N = 1 / 2 / 4).

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

The loader's chunks are also the lightning rods' operating condition (ironspec 05:33 (5)). A rod's POI
is found only while its chunk is loaded. The loader's overworld PORTAL ticket at chunk (1,1) sits at
level 30:
- chunks within Chebyshev 1 are entity-ticking (31), which takes in the four rods' chunks (0,0),
  (2,0), (0,2) and (2,2);
- Chebyshev 2 is block-ticking (32);
- Chebyshev 3 is border-loaded (33).

So a loader that stops (this fault) also leaves the rods' chunks to the player's own tickets. Thunder
only falls within 128 blocks of a player, whose tickets load the farm anyway.

THE WITCH CASE IS TWO-FAULT (ironspec 06:04 (3), G1 05:10 (2)). Every strike column goes to a rod,
and every villager place is clear of the rods' bolt boxes (9fc3155f's check, condition (ii) met). So
a struck villager needs TWO faults: a rod removed (or its chunk unloaded), plus lightning. Such a
strike on a seated rider destroys its cart in the same bolt (cc767b78 06:03: 5 × 10 = 50 > 40,
VehicleEntity 47 / 50), so the farm sees a cart lost in transit plus a witch at the strike point, not
a witch riding into the stop. What the player sees: a cart and a rider missing from the loop.

A STRUCK SCARER BOAT IS TWO-FAULT TOO (ironspec 06:43 / 06:59, G1 06:47 (2), tick-exact 06:48).
Every boat's column and the columns within 3 of it go to a rod, and every boat is clear of the rods'
bolt boxes (the bars above: the 20 boats are places). With a rod gone, one bolt destroys an oak boat:
`AbstractBoat extends VehicleEntity` (AbstractBoat 54) and overrides neither `hurtServer` nor
`thunderHit`, so `Entity.thunderHit` (2929–2936) → `VehicleEntity.hurtServer` (35–58) adds 5 × 10 =
50 > 40 and destroys it (the boat item drops). What the farm sees: a lost boat AND a freed scarer. The
pillager or zombie (no `thunderHit` override) is left standing, unseated, taking 5 damage and fire per
bolt tick. Unarmed, it can't hurt a villager, but it may move within its box and change the scarer's
sight rays (the panic classes, G1 06:47 (2)); 9fc3155f confirms each enclosure holds its freed scarer
away from villagers (ironspec 06:59).

A CRASH (an unclean stop, a priced host fault; ironspec 08:07a (5)): every item-carrying component
of the loader, and what ONE item lost or doubled does there. The game saves each dimension, and in it
the chunks' blocks and their entities, as separate records, not one transaction; so a crash can tear
an item's move only where it changes record: a spit (a dropper's inventory to an item entity), a
pickup (an item entity to a hopper's inventory) and the crossing (an entity leaving one dimension and
arriving in the other). Each side's droppers and hoppers sit in one chunk (the overworld's (1,1), the
Nether's (0,0)), so a hopper-to-hopper or hopper-to-dropper move is never torn.

loader-x8 (4 leather, the loop's only items; design frame):
- overworld side: the firing dropper (1,14,1) facing north into the portal (1 leather at the start);
  the lift droppers (1,13,1) (empty) and (1,12,1) (1 leather), facing up, which each clock pulse
  fires up the column into it; the floor hoppers (2..4,12,1) (west, into (1,12,1)) and (2..4,12,2)
  (north, into the first row), which pick up the returns; the items in flight from its spit to the
  Nether's floor hoppers;
- Nether side: the same, in its own frame: the firing dropper (3,14,-1) facing south (1 leather), the
  lift droppers (3,13,-1) (empty) and (3,12,-1) (1 leather), the floor hoppers (0..2,12,-1) (east)
  and (0..2,12,-2) (south), the items in flight to the overworld's floor hoppers;
- the clocks' observers hold no item.

What one item does, wherever it is torn:
- LOST: the loop runs on 3. The redsim control `steal-item` shows it (loader-x8 at N = 1, 2,400 gt,
  the run citing 8ba23e466: https://artifacts.internalmeta.com/a/29314562368156393): 4 -> 3 at t 600,
  then 95 arrivals each way, all at the loader's own portals, worst gap 48 gt (252 left of 300), 95
  launches on each side; the loading holds. Each further loss leaves fewer; at zero the loading lapses
  (class X).
- DOUBLED: the loop runs on 5. The loader's items do nothing but cross: no attempt follows from an
  item, and nothing leaves the loop; leather stacks to 64 in a dropper and a hopper has 5 slots, so 5
  fit without a jam. A doubled item gives only an extra crossing (an extra PORTAL ticket renewal), so
  NO GUARD is required. The redsim control `double-item` shows it (the same run conditions, citing
  8ba23e466: https://artifacts.internalmeta.com/a/1862112958260255): 4 -> 5 at t 600, the items bar
  fails on the count, and 136 arrivals each way (worst gap 48 gt) and 136 launches on each side pass.

loader-clock-x8 (6 cobblestone in the loop, and 13 stone in each side's hopper clock):
- the loop's components as above (its droppers (16,4,34) 2 and (16,5,34) 1 overworld, (3,9,-1) 2 and
  (3,10,-1) 1 Nether, their floor hoppers, the items in flight). Each pulse spits one item from a
  side's firing dropper and lifts one into it, and each side receives what the other sends, so the
  laid 3 / 3 stays 3 / 3; one item lost (3 / 2) or doubled (4 / 3) still leaves at least two on each
  side, its firing dropper refilled at every pulse (a reading of the design, not a run: this
  loader's redsim row is NOT MODELED);
- the hopper clocks (overworld (12,4,34), Nether (6,9,-1) with their partner hopper beside it): the
  period is 16n - 12 gt for n stones, 196 at n = 13, so one stone lost gives 180 and one doubled 212
  (the worst gap then about 214, 86 left of 300). But the clock's stones move hopper to hopper inside
  one chunk's records, never through an entity or a dimension: a crash can't tear them.

THE MODULE CHUNK IS ENTITY-TICKING IN EVERY LOADER LAYOUT (ironspec 06:54 (1), G1 06:47 (1)): the
arrival detector (9,−9,−5) sits inside the module chunk, and each copy's module chunk is
entity-ticking (level <= 31) from the loader's overworld PORTAL ticket in every layout of record
(N = 1 / 2 / 4 and the clock): the chunk print's bar (a chunk with a rail or an entity at <= 31)
holds it, printed per chunk in each run of record.

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
