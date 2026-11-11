# Carousel station + portal pad (brief farm-iron-carousel.md, part 3)

The batch at the stand panics at its zombies and summons at its slot; the golem lands on a pad
cell touching a portal and leaves through it in 2 gt; its tripwire pulse advances the counter; in
the Nether a lava sheet kills it over a hopper floor and the collection keeps 18 h of iron. Tools:
`mctas mobfarm station` (the bench design; `--part` this member), `station-check` (static: every
rider's golem search; `--official` the pulse on the real game), `station-bench` (the seven batches
on ironfarm's real carts and lines, 100 seeds x 6 days), `station-storage [--official]`; the member
on the linear stand: `mctas mobfarm linear --out F` (settled by default), `linear-check F`,
`linear-bench F --from --to --days [--placed-raw | --placed-in-order [GAP] | --placed-all |
--build-gap-sweep] [--drive-out F]`.

## Members (best first: `members.tsv`)

| design | blocks | success per slot | storage | build min (middle / late) |
|---|---|---|---|---|
| carousel-linear-stand.txt | 300 (pad 178, Nether 122) as the first version, on the single-lane linear stand | 99.28% (static 99.30%, 44 spots per rider) | 18.4 h | 62.1 / 16.6 (as the first version) |

Archived in `archive/` (the 7-lane layout, which vjeux turned down on 2026-11-10 at 05:58 for a
single lane with the batches back to back; kept as evidence, not on the site):

| design | blocks | success per slot | storage | build min (middle / late) |
|---|---|---|---|---|
| archive/carousel-station-v3.txt | 380 (pad 258, Nether 122) | 99.50% (static 99.30% at the end lanes; paired lanes) | 18.4 h | 68.8 / 19.2 |
| archive/carousel-station-v2.txt | 332 (pad 210, Nether 122) | 99.31% (static 99.30% worst lanes, shifted stand) | 18.4 h | 64.8 / 17.7 |
| archive/carousel-station-v1.txt | 300 (pad 178, Nether 122) | 99.06% (static 99.30%) | 18.4 h | 62.1 / 16.6 |

## How it works

- Pad: two portal planes (9 x 3 inside, z -5 and -2), a trench row on both sides of each (floors
  at y 2, x -2..8): each of the 21 riders' golem searches finds 44 spots that touch a portal cell
  and none that does not; every other floor in the searches is glass, leaves or capped.
- Pulse: a tripwire line on each frame's top row (the frame's cap and the counter's input): a
  golem's box reaches 0.2 into the string cell at its first tick; the pulse is on 1 gt after the
  spawn for 10 gt (the official server, 8 cells, and redsim agree).
- Nether part: the golem arrives centred on a 2 x 3 portal, the player corridor's 2-high ceiling
  shifts it 0.2 east, a lava sheet at its feet (sources against the plane, wall signs below)
  pushes it into a 2-deep pit, the sheet at its head burns it: dead 250-269 gt after arriving,
  its feet over the hoppers. Every wall's top is 2 over the frame's bottom row.
- Collection: the catalog's sorter (classic stock) and iron-block crafter under the pit; poppies
  to a composter (bone meal into a chest); the crafter's buffer chest drained into a double chest
  at the player's room, reached by a 2 x 2 spiral of steps from the corridor.

## Evidence (the first version)

- station-bench, 100 seeds x 6 days on ironfarm's lines: 143,900 slots, a golem in 99.06%; every
  golem 2 gt in the overworld; 142,332 killed in the Nether, none back, no item back, 569,883
  iron, 0 lost; zombified piglins 1.3 a day on Hard, all held in the planes' bottom rows.
- station-storage: 18.4 h at ~2,850 iron/h (redsim); the official server over 8 h put 22,797 of
  22,802 iron in the double chest with no item left in the world, as redsim.

## Settled as placed, and built in order (the brief's two checks; redsim, not yet the real game)

The member file is written settled (`mobfarm linear --out`, `placing::settle_design`): every cell
as the bench finds it after the settle. 14 cells changed, all in the Nether part: the sorter's three
comparators powered with their outputs as `nether nbt:` lines (OutputSignal 2, 9, 2), its dust in its
settled shapes and powers (the generator wrote every dust as a cross at power 0), and the lava
sheet's two flowing cells (2,64,0..1), which the flow had made. Its `build: fills-last` line lists
the sorter's stocks as the build's last steps. Still bench-driven: the batches are teleported to the
stand (the release is stubbed); the pad's pulse and the Nether sorter are real redstone.

`linear-bench`, 20 seeds x 2 days each:
- the judge's placement: 20 of 20 seeds clean, a golem in 9,512 of 9,580 slots (99.29%);
- settled as placed (`--placed-raw`, both parts pasted raw, no settle): 20 of 20 clean, 9,512 of
  9,580 (99.29%);
- built in order (`--placed-in-order`, the priced gaps: the Nether part first, then the overworld's;
  395 cells and fills in 338 steps, 8,500 gt): 20 of 20 clean, 9,511 of 9,580 (99.28%); the build
  ends exactly as the file (every cell, comparator output and container);
- `--build-gap-sweep`: every gap from 1 to 40 gt passes (4 seeds x 1 day each).

What the build needed (each found by the build's diff against the file):
- **Portals lit, not placed.** The default order puts each portal row's cells before the row's east
  pillar. In the game a portal cell whose frame is incomplete when a neighbour along its plane
  changes turns to air; redsim does not model that, so the bench alone would not show it.
  `placing::build_steps_of` lights each portal at one tick after every other block.
- **Lava poured last.** In the default order the sources come with the supports, before the end
  wall's and side walls' cells at y 64, and flow out of the unfinished basin.
- **The sorter's stocks after its redstone (`build: fills-last`).** Filled as their hoppers are
  placed, the iron lock hopper is not locked yet (its torch comes with the redstone): it passed its
  64 iron to the crafter and the filter refilled it, 38 of the filter's 41 iron gone and an iron
  block in the double chest by the build's end.
- **No comparator output merged.** Merged at the comparator's placement, before its stock, the
  stored output already matched the stock's when the stock went in, so the comparator never updated
  its dust ((1,59,2..3) and (2,60,-3..-2) stayed at 0 instead of 2, 1).
- **The player's 80 gt in the portal back** (prices.tsv's portal_crossing) before the Nether
  unloads: at gaps under 10 gt the last lava pour had not flowed when the Nether stopped ticking.
- **A bench bug:** linear-bench's first slot was the first hundred after spawn + lead + 1, so a build
  ending within 39 gt before a hundred skipped batch 0's first slot and its golem counted as "not the
  stand batch's" (gap 2). The first slot is now the first hundred after the spawn (batch 0 starts at
  the stand); the judge's and the raw runs start at 200 and 0 and are unchanged.

Drives for tick-exact: `carousel-linear-stand-raw.drive.txt` (settled as placed, `--placed-raw`)
and `carousel-linear-stand-built.drive.txt` (the build: the Nether part as `act t nether-set` /
`nether-nbt` lines in the Nether's coordinates, its world ticking for t 0..3040, then the overworld's
`act t set` lines; `# spawn 8500`), both `linear-bench --from 1 --to 1 --days 1 --lead 60`. The
viewer's `carousel-linear-stand.drive.txt`, recorded again from the settled file, is byte-identical.

## What did not work

- A 2-long pit with an end wall 1 over the frame's row: a golem pushed by the pit's crowd stepped
  onto it, then onto the side walls and out (one lived 23,000 gt; another's drops went back
  through the portal). Every wall 2 over the row fixed it.
- The catalog's poppy chests: a full poppy side would stop the line hopper and with it the iron.
- The output chest under the crafter: a crafter above a chest stops the player opening it.
- A ladder, then a scaffolding column (redsim models no ladder; middle prices have no
  scaffolding): replaced by steps.

## Next ideas

- The single lane (vjeux 05:58: all 21 villagers in carts back to back, released 3 by 3): the
  stand is the release's front 3 carts on one track. `mobfarm linear` writes the bench design
  (carousel-linear-stand.txt: the pad and Nether part of the first version, the 3 carts along x at z 0, x 2..4,
  two stand zombies in boats, the other 18 villagers in a far calm hold, the release stubbed by
  moving each batch's carts to the stand 60 gt before its slot); `linear-check`: each rider 44
  spots, 0 strays (99.30% at 3 callers; also at x0 1..2, z 0..1, or along z at x 3);
  `linear-bench`, 100 seeds x 6 days: 99.28% of 143,900 slots, every golem 2 gt in the
  overworld, 142,670 killed in the Nether. Its drive (`linear-bench --from 1 --to 1 --days 1
  --drive-out carousel-linear-stand.drive.txt`, mobfarm::drive's format) is what the live viewer
  replays.

- The second version is the first one 13 wide for ironfarm's stand shifted one block south (234af4b65; the shift gives the
  level-1 stand control a free face for the torch chain): its level-2 riders then miss row -6,
  and 11 wide would give 98.91% at 3 callers. On the official server (`station-check --official`,
  5 cells: both ends of the long rows and a middle cell) every pulse is on at t+1, the golem in the
  Nether at t+2 and the pulse off at t+11, as the 11-wide pad.
- Paired lanes (ironspec's ruling, x 0,1 | 3,4 | 6,7 | 9): each rider needs 12 pad columns in its
  +-8 window, so the pad goes x -3..12 (16 wide; ironspec accepted it over the box: any wirable
  single-level arrangement puts the end lanes 9 apart): the third version. Static check of all 21 riders: lanes
  0 and 6 k 48/48/36 (99.30% at 3 callers), the middle lanes up to 64/64/48 (99.89%); 0 strays;
  the held riders' searches (every batch at its hold) find no spot. station-bench, 100 seeds x 6
  days: a golem in 99.50% of 143,900 slots, every golem 2 gt in the overworld, 142,873 killed in
  the Nether, none back, 571,613 iron, 0 lost; piglins 2.07 a day (84 portal cells), all held.
