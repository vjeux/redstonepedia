# Carousel station + portal pad (brief farm-iron-carousel.md, part 3)

The batch at the stand panics at its zombies and summons at its slot; the golem lands on a pad
cell touching a portal and leaves through it in 2 gt; its tripwire pulse advances the counter; in
the Nether a lava sheet kills it over a hopper floor and the collection keeps 18 h of iron. Tools:
`mctas mobfarm station` (the bench design; `--part` this member), `station-check` (static: every
rider's golem search; `--official` the pulse on the real game), `station-bench` (the seven batches
on ironfarm's real carts and lines, 100 seeds x 6 days), `station-storage [--official]`.

## Members (best first: `members.tsv`)

| design | blocks | success per slot | storage | build min (middle / late) |
|---|---|---|---|---|
| carousel-linear-stand.txt | 300 (pad 178, Nether 122) as the first version, on the single-lane linear stand | 99.28% (static 99.30%, 44 spots per rider) | 18.4 h | 62.1 / 16.6 (v1) |
| carousel-station-v3.txt | 380 (pad 258, Nether 122) | 99.50% (static 99.30% at the end lanes; paired lanes) | 18.4 h | 68.8 / 19.2 |
| carousel-station-v2.txt | 332 (pad 210, Nether 122) | 99.31% (static 99.30% worst lanes, shifted stand) | 18.4 h | 64.8 / 17.7 |
| carousel-station-v1.txt | 300 (pad 178, Nether 122) | 99.06% (static 99.30%) | 18.4 h | 62.1 / 16.6 |

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
