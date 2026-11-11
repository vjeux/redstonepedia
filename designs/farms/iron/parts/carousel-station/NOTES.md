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

## The pad under the wired pocket stand (the assembly's part)

The parent's step (17:01): the pad in, nothing bench-driven. `carousel-station-pad.txt` is the pad
and its Nether part alone, pasted at the loop's origin under the pocket stand (riders at x -1, 3, 7,
y 0, -1, -2, z 0): `mobfarm linear --stand=-1,0,0 --pitch 4 --drop 1 --slab-floor
"-2,2,-1;-1,2,-1" --alone`. 178 blocks (2 planes at z -5 and -2, frames x -2..8, trench rows z -6,
-4, -3, -1 with floors at y 2, the tripwire lines, the output line along x 9), the Nether part's
124. Ports: `output.pulse` 9 6 -2 (the dust: on 1 gt after a golem's spawn, 10 gt long; day-1's
controller input), `nether-anchor`, `nether-axis`, `nether-output`. Pasted with `mobfarm paste`
(explore/farmopt-assembly, ironfarm's runner).

- Two floor cells are top slabs: riders leave the release curve (-3,1,0) with the cart thrown to
  z 0.12..0.19 for a tick at x -1.7, the eye at y 2.68, and its box met the cobblestone at
  (-2,2,-1): one suffocation damage a pass, every rider once a slot (found on ironfarm's bench).
  A top slab is still a golem floor (solid, its top full) and does not suffocate.
- `linear-check` with the pocket stand's cells: 40/44/40 spots, 0 strays, 99.03% at 3 callers.
- On the loop (`queue-loop --stand-stop --stand-wired` + the pad; the release and the queue
  bench-driven, LAST_SLEPT set by the bench each night as the station bench does, `--slept
  --summon-at 99`), 20 seeds x 1,440 slots (40 h): 20 of 20 clean; 28,490 golems (712/h) in 98.92%
  of slots; all through the portal 2 gt after the summon, 28,450 killed in the Nether (40 alive at
  the runs' ends); 28,490 pulses, 0 without a golem; 113,858 iron into storage (2,846/h, 4.00 a
  golem); 0 summons out of turn; no rider hurt.
- `queue-loop-spots --pad=-3,2,-7,9,5,0`: 0 cells off the pad; every pad cell is also seen from
  positions other than the stand's (the release run x -3 z 2..7, the curve, the departure), the
  brief's out-of-turn flag. Those positions are empty at the summon tick (gameTime % 100 == 0) in
  every run above: the bench checks each summoner's position as it summons.

## The pocket stand's fault guard (B′ + booster + C: `queue-loop … --stand-wired --stand-guard`)

ironspec's self-recovery rule (one missed or extra pulse at each element, in every phase it can land in):
the wired stand recovered its latch and gates after one REL, but its carts did not. A crowded pocket
(D1 missed: the whole train in pocket 1) fired carts back through G1 and G2 at the next REL, because
the always-on booster-ups and gate slopes boosted any cart drifting west while the two gates stood
open (G1/G2 = REL or not Q, by construction). The guard makes every fault drain forward.

- **B′:** the booster-ups (1,-1,0), (5,-2,0) are plain rails. G0's gate slope is powered from G0's
  driver dust through a stone under it (instant). G1's and G2's gate slopes are powered 8 gt after
  their gates open (from the latch's X by R1 (5,-2,3) and a dust line under the bus), so a cart
  held against a gate's downstream side rolls onto a braking slope, stops, and is launched forward.
  Powered at once (or after 6 gt), the slope boosts that cart back through its gate.
- **Booster:** the pocket carts leave a little slower out of the braked slopes. One booster at the
  run's first cell (9,4,8), downstream of every gate, restores what the tail catches (top of the
  climb 1.05/1.01/1.12 b/t).
- **C, the D1 fallback:** a timer on REL (T3 = not (REL or REL delayed), on B3 under F) sets the latch
  through F (5,-1,2) into Y at REL's fall + 37 gt when D1 has not, and lets go 11 gt into the next
  REL. G1's step dust (4,-1,2) became a stone (G1 fed by a dust on it, still instant).
- **Cells** (against the wired stand 8481d37896): 63 added (12 repeaters, 16 dusts, 1 torch, 9 stones,
  25 glass), 1 removed (4,-2,2), 14 changed (5 redstone blocks to glass or stone, 2 booster-ups to
  plain rails, 3 gate slopes unpowered, (4,-1,2) dust to stone, (8,-3,1) glass to stone, the run's
  booster and its support). Riders' rest cells, pockets, gates and rest slopes unchanged; golem
  spots 0. The exact list went to day-1 for its re-paste (no overlap with its lean file's cells).

**Normal operation (bench-driven queue, `--release-stagger 4`).** 100 × 1,440 clean; staggers 2/3/4/6
× REL 10..50, 0..44, 16..76 all 3 × 30 clean. D1 sets the latch at +77..+79 everywhere; D2 9..15 gt
(by stagger); departures 10–13 gt apart, never touching; tail entry ≥ 9 gt; no cart off the rails,
every rider seated.

**Margins.**
- C (the stand's tightest control), centred at REL's fall + 37: set at +81 at the early corner; forced
  earlier by repeater acts it is clean down to +77 (4 gt above the last clean time) and at +75 stagger
  6 loses cart 2 behind G2 (6 above the first failure). At the late corner it sets at +113, 3 gt
  before the next REL at +116; later changes nothing in normal operation. C catches the cart that
  bounces back from a crowded pocket 1 only while it is west of G1: in absolute slot time C must fire
  by about +91 (at +87 and +91 D1 missed recovers; at +95 and +99 four carts go back through the gates).
- G2's flag (0..2 gt after cart 2's tail at stagger 4): answered by measurement, no change. G2 shuts on
  cart 1 reaching D1, so its margins follow the cart spacing, and it is already centred over staggers
  2..6: stagger 2: 6..8 gt after cart 2's front, 1..3 before cart 3; stagger 4: 4..6 after the front
  (0..2 after its tail), 5..7 before cart 3; stagger 6: 2..4 after the front (−2..0 after its tail),
  17..22 before cart 3, clean. Shutting 1–3 gt early (stagger 4) is clean; 4 gt early blocks cart 2. A
  closing gate only blocks movement into its box, so the real edge is cart 2's front, not its tail.

**Faults** (each at slot 5, 3 seeds; "perfect tail" = `--reset-queue-at 6 --reset-queue-every`, a tail
that always re-packs, so the stand's own share shows; the bench's timed tail breaks later in most
faults: the queue's share).
- Back to a clean 3 by slot 7, 0 back-crossings: extra REL 40 gt at +66/+79/+85/+92; 2 gt REL glitch at
  +66/+79; D1 missed (45 or 65 gt) at REL 10..50 and 0..44; C missed; C extra at +79/+92; the whole
  part unloaded 3/40/240 gt in 6 states (54 of 54).
- Drains forward but leaves stragglers (no clean 3 within 8 slots, 0 back-crossings in the fault's
  slots): REL missed (a 4-cart group at 10/4/9 gt, the minimum departure gap); D1 extra (+50, +66);
  C extra (+50, +66); the 2 gt glitch at +92.
- D1 missed at the late REL corner (REL 16..76): C fires at +113, too late; 3–5 carts back through the
  gates, a 5-cart group at 3 gt.
- Partial unloads at the aligned chunk line z 9|10 (`--shift 6,6`, stage 32/33): departure gaps down to
  3 gt with the stand's chunk frozen, groups ≤ 5; tail entry ≥ 5 gt, never touching, except one 4-cart
  group at 4 gt. With z 10..25 frozen (the run north's upper part): 0–3 touching pairs at the
  departure, 0 carts off the rails, 0 riders unseated or hurt, 0–7 carts back through G0 from the climb
  (departing carts meet the frozen ones at z 10 and roll back down during REL), clean from slot 7–8 in
  6 of 36 runs, from slot 14–15 in 2 more.

**Open:**
1. stragglers on the approach flats don't clear after REL missed, D1 extra, C extra (+50/+66) and most
   partial unloads;
2. D1 missed isn't recovered at the late REL corner, because C is keyed to REL's fall; C′, a
   train-synchronous fallback, is the proposed fix;
3. a frozen chunk past the climb's exit (z 10..25) rolls departing carts back down the climb and
   through G0.

The no-rest approach (ironspec's constraints) is being designed in emulation, with C′ (an edge
detector wherever the new approach gives a clean once-per-train edge, replacing C's timer: about 25
blocks against 43) and a departure top no cart can roll back from before the z 9|10 line folded in.

## Declared states and flights (the brief's "No derails", Nov 11)

Both station files carry the lines `mobfarm::declared` and `mobfarm::flight` read:
- `carousel-linear-stand.txt`: `declared:` the stand's 3 rails and the far hold's 18, unpowered
  powered rails (brakes) that must stay unpowered (`!`: checked after the warm-up too); `flights:`
  none (the bench moves the carts; none leaves a rail). `linear-bench` checks the states once the
  design is in the world and the brakes again 20 gt later, and watches every cart every tick: an
  off-rail stretch that is no declared flight, or a state that doesn't match, is a violation.
- `carousel-station-pad.txt`: both lines empty (no rail, gate, booster or detector; no flight).
  `mobfarm paste` carries a part's lines into the merged file, moved by `--at`, and refuses a cell
  both files declare differently.
- Measured (redsim): the bench design, 8 seeds x 1 day in all three placements, 21 states, 0
  mismatches, 0 derails; its three drives are byte-identical to the committed ones (`cmp`), and
  drive-check passes. Positive controls: one stand rail declared `powered=true` fails at gt 200
  and after the warm-up; a hold rail removed (no declared line) gives 41 derails, one per cart
  parked there.

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
