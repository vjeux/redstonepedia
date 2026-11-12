# Carousel assembly v0 (farmopt, WIP)

The three finished parts in one design on ironfarm's paired lanes (x 0,1 | 3,4 | 6,7 | 9):
ironfarm's release + hold, farmopt's station + 16-wide portal pad + Nether part (kill, sorter,
crafter, double chest), day-1's counter v7. The sleep routine is stood in by the benches'
once-a-night LAST_SLEPT (the real-game gate has no stand-in: it runs once the routine lands).

| file | what | written by |
|---|---|---|
| carousel-v0a.txt | step a: counter v7 under the stand (y -11..-9), the pad's pulse wired to its input; the counter's lines drive the slope controls IDEALLY in the bench (no line wiring) | `mctas mobfarm assembly --out F` |
| carousel-v0b-wip.txt | step b WIP: counter v7 in the yard space (z 30..42), the pulse wire along x 13, every line wired (fan-out, inverters, stand towers and torch chains, delivery cells at z 20 for the yard's hold feeds) | `mctas mobfarm assembly --wired --out F` |

## Runs

- v0a, the running preview: `mctas mobfarm station-bench carousel-v0a.txt --from 1 --to 1 --days 1
  --timeline DIR` (the counter in the loop: its real ring, its fallback and the pad's real
  pulse; each line switches its lane's slope controls ideally). 4 seeds x 1 day: a golem in 99.79%
  of 960 slots, one advance per slot, the batch at the stand a mean 25.8 gt before its slot, no
  violations. Static: `mctas mobfarm station-check carousel-v0a.txt` PASS.
- v0b, the wiring layout (static): `mctas mobfarm station-check carousel-v0b-wip.txt` PASS (0
  strays, held riders' searches empty); `mctas mobfarm assembly-check carousel-v0b-wip.txt`
  (the redstone alone): the counter cycles in order with the real pulse wire (397 advances in
  400 slots, 0 out of the cycle, never two lines high), the delivery cells follow their lines in
  2-6 gt. KNOWN DEFECT: the stand torch chain puts torches in the stand control cells, and an
  ascending rail needs a sturdy block at its high side (BaseRailBlock.shouldBeRemoved), so the
  three stand slopes pop off at the paste: a run of v0b shows the carts failing at the stand.
  The fix (each control a solid block fed NOT line: level 0 from a torch below, level 2 from a
  repeater on its north; level 1 has no free face) waits on ironfarm's stand geometry.

## Box

- v0a: 1,321 overworld blocks + 122 Nether, box x -4..14 y -11..9 z -6..25 = 12,768.
- v0b WIP: 1,978 + 122, box x -4..13 y -5..9 z -6..42 = 13,230 (day-1's counter + scheduler block
  under the yard replaces v7 and the hold delivery cells; ironfarm's yard sets z to 48).

## One chunk (onechunk, WIP, Nov 11)

vjeux: "Can we have the system in one chunk?" Paper: ironspec's v0 (artifact 1105169452432266), the
gravity queue v0 (2098083320824660), day-1's fold plan v1 (1017115541419820). The skeletons in
`wip/onechunk/` are sketches: the committed parts placed as they are (`mctas mobfarm column`), no
wiring, nothing benched, everything else a reserved box; both are written in the aligned frame
(loop + 6,0,6); cells below are in the loop frame, y = the stand's rails.

- Committed boxes (loop frame): stand A + pad (S3 92d039c02) x -3..9 y -6..6 z -6..7 (its posts down
  to -6); R1 (d10a339bb, r1-combine's tail box + post columns) x 0..9 y -1..15 z 21..34 (its post at
  y 12 over its rails); C24's register 17 x 16 x 47 (1,586 blocks) and controller 17 x 27 x 55
  (1,185): neither fits 16 x 16 as committed (day-1's fold re-generates them); the turned bed stop
  (e5737dbb7) 14 x 18 x 14.
- The pad's golem spots (`mobfarm exit`, the 3 riders): all 44 at feet y 3 (the trench row at 2),
  every run into a portal; per pocket alone 40 / 44 / 40. Lifted 1: all at y 4, each pocket
  reaches them. Lifted 3 (y 6): pocket 3 alone finds no spot ("no golem can spawn anywhere in the
  build"; the flood scans from the rider's cell and the one above, + 6).
- The lock box decides every waiting tier (villager box grown 17.84 against the golem's): feet
  at least 19.79 below the lowest golem spot or 20.54 above the highest. A straight run in the chunk
  rises 15 at most, so a 20-cell climb takes a corner.
- A dug cavity's rock floor inside the stand's golem band (y -8..+6 around the pockets) is a golem
  floor: a stone floor at y -7 under the stand gives spots all over the column and golems that
  reach the zombies; bottom cobblestone slabs on it (y -6) leave the 44 pad spots only.
- `skeleton-b-folded.txt`, (b) with the folded register: pad +1, queue tier -21..-16 (peaks -17), H
  at rails -17, gap -15..-7, bed stop -39..-22, controller -49..-40, cover 8..10: 16 x 60 x 16 =
  15,360 blocks³; `mobfarm bill` (void, middle) 130.3 build minutes on its 771 placed blocks.
- `skeleton-v3-gravity.txt`, the gravity queue: today's pad, the stand tier's floor capped, golem
  zone 7..24, queue tier 25..39 (hold feet >= 24.5), tail tier 40..49, bed stop 50..67, cover
  68..71: 16 x 78 x 16 = 19,968 blocks³; 140.0 build minutes on its 1,020 placed blocks.
- `bill --terrain` finds a spot that deep in village-edge only: plains and forest refuse inside the
  judge's 14-column spot search (water at y 25 under plains' start, a mineshaft at y 4..6, the
  forest's river and logs, uneven ground); the samples are full height. b8949c96's dug-in finder
  (explore/farmopt-dugin) searches the whole sample.
- `skeleton-v4-inverted.txt`, the lead (ironspec 17:30): v3 with the pad below the stand (the
  committed pad moved down 9: all 44 golem spots at feet y -6; per pocket 40/44/40; the pad one
  lower and pocket 1 alone finds no spot), so every waiting rider needs feet >= 14.54 and the
  hold sits at 16.18. The flood with every villager position a summoner (124: riders, every
  sketch rail cell, the napper on foot; the bed cleared so it floods as a part) finds the pad's
  spots only, with no bottom slab: the stand's blocks and the tracks offer no golem floor in the
  band and the cavity's floor (y -10) lies below every pocket's reach; a stone with air above
  beside the stand is caught. The stand's posts move off the pad (post-search: (4,2,-6),
  (2,4,-6), (9,4,-4); flee-check's verdicts as the committed stand's); the pressed run's posts
  (2,31,5), (2,24,-2), (3,28,4), (3,21,-3) cover its 14 cells (least 6.35); the napper on foot
  sees no zombie (its waker behind the hatch), and a post moved into its view at (-1,48,8) shows
  in sight (and inside 6.05). Its header carries the joints table. 16 x 73 x 16 = 18,688 blocks³.
- The sentinel villager (b8949c96's rule: in the shared bed's section, its box more than 11 from
  every villager position's box on one axis, sealed, the AFK 2 sections away) has no room in the
  bed's section of any of the three: the bed stop fills the 16-wide column; it goes in the next
  chunk at the bed's height.
- v4 = C′ (ironspec 18:21-18:45, the layout of record): 58f770cc's minimal bed tier in the SW
  quadrant (bed head (-3,50,4): every rider more than 16 + 1 from it, the least 17.13 at the tail's
  R4 slope; `mobfarm column --bed`), 9fc3155f's hold (hold-v4) and tail chain with the switch flat
  (tail-v5), the hold's leaving-edge detector Ds (hold-ds-v4.txt), the cavity floor at y -11 (the
  flood with every rail cell a summoner found the stand's departure rails at y -3 reaching a floor at
  -10: 101 off-pad spots), the sentinel room in the east chunk ((c) 12.00 on x). 16 x 69 x 16 =
  17,664 blocks³. G0's departure detector (g0-detector-v4.txt, replacing booster #1 at (9,-3,1)) is
  prepared, placed once dfb89499's bench keeps 2 gt at every corner without that booster.
- Condition (a) over every villager position (`column --sections`): 67..118 of 146 positions lie
  more than 1 section from both homes by the site's y phase; a third sealed home at mid-height in the
  east chunk brings every phase to 0 (an option line: ironspec ruled the stamp bench instead).
- The full bills (provisional: b8949c96's dug-in prices at its whole-sample spots, 2 s a placed
  block, 1 s a dug one, its cave fills and spawn-proofing slabs; my estimates for the parts not laid
  at the judge's middle prices, the committed C24 file's 5.33 s a block for redstone):

  | | v4 = C′ plains / forest | (b) plains / forest |
  |---|---|---|
  | the skeleton file, dug in (b8949c96) | 631.4 / 651.5 | 464.3 / 512.3 |
  | spawn-proofing (673 / 1,052 cells; 418 / 528) | +22.4 / +35.1 | +13.9 / +17.6 |
  | parts not laid (release, controller, wiring, boosters, clock, divert; (b): register ~520, controller ~750, H, climb, descent, R1, posts, 18 riders) | +53.2 | +190.1 |
  | the bed tier (58f770cc's 191-block minimal tier; (b): in place of the committed bed stop) | +16.7 | -20.5 (void; about -6 dug in) |
  | AFK room, shaft and hatch | +1.5 | +1.9 |
  | sentinel room | +6 | +6 |
  | total | about 731 / 764 | about 656..670 / 707..722 |
  | blocks³ | 17,664 | 16,384 |
  | option: a third sealed home | +6 | |
