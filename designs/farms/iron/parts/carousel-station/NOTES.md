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
  part unloaded 3/40/240 gt in 6 states (54 of 54; re-run on main's reload fix 2cb90d3c7: unchanged,
  save order only; a whole unload pauses the part and its controls together, only world time runs on).
- Drains forward but leaves stragglers (no clean 3 within 8 slots, 0 back-crossings in the fault's
  slots): REL missed (a 4-cart group at 10/4/9 gt, the minimum departure gap); D1 extra (+50, +66);
  C extra (+50, +66); the 2 gt glitch at +92.
- D1 missed at the late REL corner (REL 16..76): C fires at +113, too late; 3–5 carts back through the
  gates, a 5-cart group at 3 gt.
- Partial unloads at the aligned chunk line z 9|10 (`--shift 6,6`, the pad pasted, perfect tail; the
  stand's chunk at stage 32 or the stand's two chunks at 33, or the queue's chunk z 10..25 at 32, for
  40 or 240 gt in each of 6 states; 36 rows, re-run on main's reload fix 2cb90d3c7 with the same
  result, save order only): departure gaps down to 2 gt (two rows) and 3 gt (ten), groups ≤ 4; tail entry ≥ 5 gt
  except one 4-cart group at 3 gt (the stand's chunks at 33 for 240 gt at REL's end), never touching;
  0 carts off the rails, every rider seated. Merged pairs (the shared check, `crate::merge`) in 9 of the
  36 rows, 8 at pocket 3's flat (-2,1,0), where an arriving cart slides into a straggler resting there,
  and 1 on pocket 3's rest slope: open item 1. Back through a gate in 8 rows, 7 of them with the
  queue's chunk frozen (departing carts meet the frozen ones at z 10 and roll back down during REL):
  open item 3. A frozen chunk's carts are not saved, so the reload fix does not reach these runs.

**Open:**
1. stragglers on the approach flats don't clear after REL missed, D1 extra, C extra (+50/+66) and most
   partial unloads;
2. D1 missed isn't recovered at the late REL corner, because C is keyed to REL's fall; C′, a
   train-synchronous fallback, is the proposed fix;
3. a frozen chunk past the climb's exit (z 10..25) rolls departing carts back down the climb and
   through G0.

The no-rest approach (ironspec's constraints), C′ and a departure top no cart can roll back from
before the z 9|10 line: next section.

## The no-rest approach, C′ wired, the departure crest (farmopt's approach lab; redsim only)

`queue-loop --boost-first 8 --row-gap 3 --detector --zombie-every 8 --tail-desc --stand-stop
--stand-wired --stand-guard --approach 5 --cprime 4,4,4,2,4,4 [--depart-crest 3]` (lab flags of
`crates/mobfarm`, explore/farmopt-approach; no member carries them). Runs need `--release-stagger`
(the guard's catch; the corner and fault tables set it themselves).

- **Approach 5.** From the queue head: boosters to z 10, five plain slopes up (the third, (-3,3,7), C′'s
  detector D), the flat top T (-3,6,4), three slopes down, and the snap turn (-3,2,0)
  (`rail[shape=ascending_west]`; its high side two glass blocks high, (-4,2,0) and (-4,3,0): with one,
  a cart pushed back west by a jam in pocket 3 climbed onto the block, off the rails). A cart crossing
  into the turn's column takes the rail below at once: no tick off the rails there, so the turn is no
  flight. Every cell from the head to pocket 3 but T is a slope or a booster: a stopped cart rolls on.
- **Why T stays (the one rest cell).** The head and pocket 3 sit at equal height, so the run climbs
  before it falls, and every climb ends in a flat (a slope up straight into a slope down is reshaped
  flat by its neighbour updates). Raising the head instead costs height and volume and moves the
  tail. An off-rail crest instead adds the ledge's 4 gt landing failure where fault groups run tight:
  carts cross T at about 0.4 b/t, so a one-block drop carries them about 2.4 blocks and a straggler
  barely 0.3, and the three cells down to pocket 3 can't hold that spread; and a cart stalled at an
  off-rail crest rolls back down the climb into the release run instead of staying on T.
- **T's recovery (measured): not by push-in.** A cart at rest on T, five places along it, is not
  pushed through: the next train's first cart meets it while still on the climb's last slope, both drop
  to about 0.05, the train rolls back down the climb into the release run's boosters (backward), and
  the stranded cart creeps into the stand alone. With the bench's own tail, 3 seeds: 0-3 of 12
  corners x staggers clean per place, the head's cart or pocket 3's; a booster on T (its redstone
  block under it) gives 0-4 of 12.
- **C′ wired.** D strongly powers its stone S0 (-3,2,7); R1 (-2,2,7), a dust U0 (-1,2,7), a six-step
  spiral staircase in x -2..-1, z 5..7 down to (-2,-4,6) (each step's support a stone; the cell above
  each lower dust air), east along z 7 at y -4 through M1..M4, N (6,-4,6), the subtract comparator K
  (6,-4,5) (side: N delayed by RS1 (4,-4,6) and RS2 (5,-4,5)), the stone K_n (6,-4,4), the wall torch
  T_n (5,-4,4), then C's line (5,-4,3) -> B3 -> T3 -> F. Routed clear of day-1's controller (its line
  along z 7 at y -1/-2 and its line along x 5 at y -6/-7: the first wiring crossed both).
  41 blocks added, 28 removed (C's timer), 5 changed; golem spots 0. The whole redesign against the
  guard (approach 5 + C′ + crest): 84 added, 69 removed, 12 changed (approach +18/-16/~5, C′
  +41/-28/~5, crest +25/-25/~3). D's rise to T3 lit: 42 gt; the
  window that keeps D1 missed clean at every corner x stagger is 40..42 (38 fails stagger 6, 44
  fails stagger 2). Normal runs: the latch set at +77..+79, never out of turn; at stagger 6 T3 lights
  a second time at the next slot's +14/+17, inside REL.
- **Departure crest (`--depart-crest 3`).** The boosted climb tops out at a flat (9,2,6), three slopes
  down to (9,-1,9), boosters past the line (9,-1,10..14), a boosted climb back to the return's y 4 by
  z 19. Day-1's controller route under it moved on explore/day1-crest (merged). It does not fit the
  14-slot escapement register: that return runs at y 10 and its climb peaks at y 12 by z 16.
- **Day-1's integrated loop** (release-gen with the crest and this C′): the nap night, 130 slots, 22
  visits complete, 0 problems, 0 merges, 0 declared mismatches, 0 carts off the rails; the divert
  corner from G0's opening as before: cart 0 in 11 out 13, cart 1 in 24 out 27, cart 2 in 34 out 36.
- **Corners** (30 slots, 3 seeds): every REL corner x stagger 2/3/4/6 clean; D2 9..15, G2 shut 5..7
  (stagger 2) to 1..3 (stagger 6) after cart 2's front, 1..3 to 17..22 before cart 3; departures 10 gt
  apart at least, tail entry 9. At entry speed +0.05, stagger 6 fails at the nominal and early corners,
  as the guard without the approach does on the same binary.
- **Fault table** (the aligned crest design, perfect tail, 12 corners x staggers, 3 seeds): clean
  everywhere: normal, D1 missed 45/65, C missed, C′'s detector missed, C′'s extra edge at +50/+66/+90.
  Not clean: D1 or C extra at +50/+66 (a set before the train shuts G1/G2 ahead of it; the train piles
  into pocket 3, merged pairs on its rest slope), C′'s extra edge at +0 (early corner) and +20 (42 gt
  later it is an early set), REL missed, extra and glitch (the settled-release gate's), and the partial
  unloads (a frozen stand chunk: as with C, 42 vs 45 clean of 144). 0 carts off the rails in every
  row since the snap turn's wall (before it, a jam pushed carts onto the turn's single glass block).
  Whole unloads (40 and 240 gt at +66/+79/+92/+12/+30/+48) under the save order, its reverse and
  seed:1..8, nominal corner x staggers 2/3/4/6: 480 of 480 rows clean from slot 0.
- **D1/C extra at +66, why the early corner doesn't recover:** the train piles into pocket 3; REL's
  44 gt can't clear the pile, so its last cart stops in pocket 1 after G0 shuts, and crossing D1 at
  about +45 sets the latch again before the next train, which piles again, every slot. A REL held
  from its rise (`--rel-min L`, emulated) can't fix every corner: the stand's REL has to end inside
  +70..+79 (after the 4-gt set pulse, before cart 1 reaches G0) while the rise moves over +0..+16:
  L 60 clears nominal and late but not early staggers 2/3; L 70 clears early but breaks nominal
  stagger 2 and the whole late corner. An end tied to the train (C′'s N, D + 36 gt: +71..+73 at every
  corner and stagger) would fit; not built.
- **The pad** (`carousel-station-pad.txt` pasted at 6,0,6, `--slept --summon-at 99`), the partial
  unloads at every corner (2 seeds): 689 of 6,140 summons out of turn in the chunk rows (the guard
  without the approach: 862 of 5,866); normal runs 0.
- **Riders and the shared bed** (day-1's bed head at (-20,2,12)): the closest a rider comes, napper
  aside, is 17.06 blocks on the release run (as in the guard); on the approach (the climb, T, the
  descent) 17.13 at the least.
- **Divert corner against day-1's +4-shifted windows** (from G0's opening, day-1's G0 at golem +13:
  cart k's window +8+10k..+17+10k): cart 0 in 11 out 13 (margins 3/4), cart 1 in 24 out 27 (6/0),
  cart 2 in 34 out 36 (6/1) at every corner x stagger, as the guard gives: the departures are the
  guard's, untouched by the approach. At +0.05 the stagger-6 rows that fail scatter it.
- **escG6L in the loop** (`--slots 14 --tail-esc 6 --esc-adm --adm-ledge`, run with `--fifo-intake 6,2
  --esc-adm 2,4 --adm-k2 10 --p-always`, 40 seeds x 30 slots): 27/40 clean without the approach, 30/40
  with approach 5 + C′; the holes are the tail's admissions (ironfarm's open item). D1 missed with
  escG6L as the tail (30 slots, 3 seeds, before the snap turn's wall): back to clean 3s by slot 7 or
  sooner at 10 of 12 corners x staggers; nominal stagger 2 not (2 merges, 1 cart off the rails at
  the turn) nor stagger 3 (one seed's tail hole).

### T, the census, and the departure crest's dip (Nov 11, bench, not yet validated)

- **Why a cart on T is not pushed in.** The next train's first cart meets a cart resting (or crawling)
  on T while its own centre is still on the climb's last slope (z 5.48-5.58, y 5.49-5.58, wherever on
  T the cart rests): the contact reverses the pusher (0.05 back) and gives the met cart 0.05 forward;
  the train rolls back down the climb, over the release run's boosters to the queue's head. The met
  cart leaves T by itself 30-40 gt later. The climb's last slope as a booster (`--approach-boost`)
  drives the reversed pusher back harder and drags the met cart with it (0-2 of 12 clean): rejected.
  So the rule (ironspec): on a slope or a boosted cell, a moving cart never meets a stopped, crawling
  or frozen cart.
- **What strands a cart on T** (`carts at rest on T` by cause, `meetings` on the climb, on T and below
  T with the slowest cart met, `climb reversals`; contact within 1.2, a held train = carts at rest or
  under 0.15 on the descent). Every fault case x 12 corners x 2 seeds (2,016 runs, approach 5 + C' +
  crest): 4 stops on T: 0 slow forward arrivals, 3 backward from the stand (REL extra, the z 9|10
  split), 1 behind a crawling train (after a 40-gt stand-chunk freeze). Normal, D1/C/C' faults and
  every whole unload: 0 meetings, 0 reversals. A frozen stand chunk with the release run running:
  1,019 climb meetings with frozen carts, 883 climb reversals (the arrival side's border problem).
- **The acceleration on the climb** (`--approach-climb-boost --boost-first 0`: the climb's slopes
  boosters but D, the release run plain; 8 cells changed): normal flow identical (arrivals at G2 at
  every corner), the split rows nearly clean (0 / 4 meetings), the frozen stand chunk not (484 climb
  meetings, 259 reversals).
- **The departure crest's dip.** The boosted dip is forbidden-class (a cart bounced backward there is
  boosted back). Plain dips: `--crest-descent N --crest-plain-dip`; farmopt's (c') (two plain slopes
  down from the crest, the flat dip at y 0 from z 9 across the line; R1's part with 13 boosted slopes
  to (9,13,25): wip/stand-departure-crest-r1.txt, `--part-handoff 13,25`). A leaked batch (chunk
  (0,1) frozen as cart 1 enters the dip) rests pressed: cart 1 frozen at z 10.25, cart 2 against it
  on the flat, cart 3 on the second slope; it restarts by itself after the thaw. With one plain slope
  ((c)) cart 3 rests on the flat and never restarts by itself. The departure-clear input
  (`--rel-clear 9,0,10`, emulated: REL_eff rises only while the cell is empty) holds the next batch
  (margin in normal flow 39 gt at every corner); `--head-wait` holds the queue head's release too.
- **Allowed cells from push-in rows** (`dip stop z Z at rest|crawling`: the head's cart set in the dip
  30 gt after REL; perfect tail, 12 corners x 2 seeds): z 9.5 and 10.5 clean (no reversal, no meeting
  on the rise), z 11.5 not (the pusher reverses onto a slope, meetings on the rise) — the opposite of
  the 1.2-contact distance rule, so the part's `border:` lists (9,0,9..10) (`--part-allowed 9,10`).
- **The tail must never back carts onto the rise:** with the bench's own tail, an extra cart in a
  departing group overflows the tail at the U-turn; the carts it backs onto the rise are boosted
  backward to the cap, clear the 2-block crest and roll down the climb through G0 (every unexplained
  back-crossing in the push-in rows traced to it).
- **The approach's border, C and C'' (wip/approach-c-border-flat.txt, approach-c2-border-flat.txt).**
  C (`--approach 4 --approach-climb-boost`: the run plain, a plain flat through z 9|10, the climb
  boosted from z 7, T (-3,5,3)) compresses the cart 1->2 gap at G2 to 4-6 gt (nothing stretches it
  before the climb) and breaks today's stand. C'' (`--boost-first 3`: the run's boosters at z 16..14
  right after the head, the plain flat z 13..8) keeps today's 9-13 gt gaps and is clean in normal
  flow at every corner, but fails its line in the frozen-stand rows: the in-flight train stacks on
  the flat against the frozen front, the next cart bounces off the pressed pair and rolls back onto
  the boosters (566 of 864 freeze rows, 8 seeds; a ridden cart barely slows on plain rail). The
  bounce off the stack leaves at stored v <= 0.25 in 649 of the 864 rows (a hump of 64 v^2 + 1 <= 5
  blocks would stop it), but carts pushed back over T come down the boosted climb boosted backward
  to stored 1-2.9 (210 rows): no hump stops those, so a boosted climb needs the stand never to push
  carts back over T.
- **Withdrawn: every "by place" climb figure of C and C'' (388/404, 723/113/148).** A parse bug, not
  meetings: `approach::summarize` read the climb count with the key `"climb "`, which first matches
  inside "flat run below the climb N", so the table's climb column repeated the flat column in every
  row. Fixed on explore/farmopt-c3 (keys `"), climb "` etc.) with a per-column positive control in
  `approach::tests` (one meeting in one place reads back in that column alone; the old key fails it:
  [1,0,..] read as [1,1,..]). Control row: C'' (bc2387241), "REL 10..50 st 2: chunks 0,0,0,0 40 +12",
  seed 1: the table read 4/4/0/0 before, 4/0/0/0 after, the single run flat 4, climb 0.
- **The real climb meetings on C'' ("meetings on the climb", 9fc3155f, validated bench: rel-sweep
  -2..+2, stagger 4, skew 2,2,2, pocket-release 13, head latch 3,-3,4,2; 73 cases x 5 x 8 seeds):**
  none in normal flow; all after a stand-side freeze. Stand freeze 240 (+40..+75): 1,326 in 116 of 180
  rows; chunks 0,0,0,0 40/240: 13/17; 0,0,0,1 40: 20; 0,1,0,1 40: 11; the 240-gt 0,0,0,1 and 0,1,0,1
  rows 0. By cause (the runner's new "meetings ... by cause" lines): a frozen cart in the pair 0, a cart
  pushed back over T 285, head-on (a forward cart onto a backward one) 342, a stalled cart met 318, a
  catch-up 381 (stand-freeze rows); 1,239 of 1,326 within 200 gt after the thaw, none during it. Traced
  (stand freeze 240 +62, REL +0): 14 gt after the thaw the restarting stack's follower catches its
  leader at the climb's foot (cart 19 z 7.61 vz -0.31 / cart 20 z 8.80 vz -0.35); at 55 gt a forward cart
  boosted to stored 2.03 meets cart 16 rolling back at +0.59 after the stand pushed it back over T;
  at 161 gt a forward cart at 0.72 meets one crawling on T (-0.010). Carts pushed back over T: 509
  (fastest stored 0.429: v_T for today's stand).
- **C''' (paused fallback, ironspec 11:23 Nov 11: the main line is C'' + a hump; riderless rows
  discharged by the siding's ridden-only exit).** wip/approach-c3-hump.txt (`queue-loop ... --approach 5
  --hump 5 --boost-first 3`, (c') departure): 3 flat boosters after the head, a plain hump (top y 7 =
  the dip + 5 = T + 1), the dip's flat z 9..10 at y 2 (the line on flat), a plain climb z 8..5 (D at z
  6), T (-3,6,4), approach 5's 3 slopes down. The run needs 17 cells: the queue's head moves north 9
  (head slot z 19 -> 28; `--boost-slopes`: the boosters as the hump's first slopes, +6). Normal flow
  (perfect tail from slot 1: the moved head breaks the bench's own tail; seed 1, nominal):
  - arrivals at T +73/+81/+92 by train place (C'': +49/+60/+72): today's stand breaks (its slack is
    ~8 gt); boosters as slopes: +63/+73/+84;
  - stored at the boosters' end 0.82/0.94/1.09, the hump's top 0.37/0.49/0.64, T 0.40/0.51/0.64,
    the snap turn 0.62/0.73/0.86; 2 or 1 boosters (flat or slopes): ridden carts don't clear the hump;
  - the game's height-change rule (0.05 per block) and the slide (0.0078 per gt) cost ~0.075 stored per
    block of rise at the visible cap (measured 0.074 over the hump's 6) and give it back going down: v +
    0.075 y is about conserved, so H's 0.6 at first contact needs <= ~0.46 at T and <= ~0.40 at the top,
    narrower than the release's spread by train place (0.25);
  - an empty cart decays (0.96 per gt): an empty 1st cart stalls before T (0.02-0.07 in the dip), is
    met on the hump's rising face and on the climb, and is shoved into H at stored 1.94.
- **C'' + hump, geometry B (the main line from ironspec 10:23 Nov 11; 9fc3155f, explore/farmopt-c3).**
  wip/approach-c2-hump-b-boosted.txt (`queue-loop ... --approach 5 --approach-climb-boost --hump 5
  --boost-first 3 --boost-slopes`, (c') departure): the 3 boosters as the hump's first slopes, a plain
  hump to y 7, the dip's flat z 9..10 (the line between), C's boosted climb, T (-3,6,4), 3 slopes down;
  the head slot moves north 6 (z 19 -> 25, GO's chunk edge). -plain.txt: the same with a plain climb.
  Rows on S3's meter (7b90c3f1's `--meter-hold 14,14,0,0,20,0,0|40,-1,1,12`, virtual B unvalidated,
  perfect tail from slot 1: the queue reset now keeps VB's parked carts, else 1 of 6 trains metered):
  - the plain climb fails: a cart at rest in the dip (z 9.5) isn't pushed up it (678 climb meetings in
    5/5 rows, mostly stalled carts), and the stand-freeze rows jam (39,132 in 180/180; 12 of 24 trains
    in): a thawed stack restarts by gravity but can't climb 4 plain blocks from the dip. Out.
  - the boosted climb passes push-in (0 in all 4 cases) and normal flow (least stored speed at the
    hump's top 0.45, at T 0.94; 1 cart per opening); a cart forced back over T crosses any hump of 2 or
    more (it comes down the boosted climb at up to ~0.4 and is boosted on).
  - cc767b78's head model (`--head-perm=-3,4,2,-2,1,0,1,1 --head-retry 1`, ported from
    explore/farmopt-s2 f67523242 with 38dd0d7f7d's summon margins; positive control: identical to its
    runner on C'' (bc2387241) on every meter and head-perm line). The A margin is each fill's Dh3 to
    the run's own next summon check (3 riders at rest and a golem); `--summon-at 93` (pad delay S+7
    assumed, day-1's c24pad; the frame is a hypothesis until day-1 measures it). Positive control: the
    train 2 / 5 gt late (`--release-start 5 / 8`) drops it by exactly 2 / 5. Worst / best of 27 corners
    (`--meter-corners`: launch +-2, H's start +-2, H's length +-2), every fill 3 at rest and a golem:
    hump 0 40/56, 1 39/55, 2 39/55, 3 34/50, 4 29/45, 5 23/39 (C'' 40/56); head moves 0, 0, 0, +2,
    +4, +6. Without `--summon-at` (the bench's default frame) hump 5 misses every A tick: a frame
    artifact, not the farm.
  - freeze and consequence rows under that head model (51 stand-freeze phases +30..+80, hump-region
    thaws, push-in, 40 back-over-T cases; 4 seeds): no height passes "0 back over T, 0 merges": 123 to
    165 carts pushed back over T at every height (hump 0 included), because under a stand freeze the
    head launches A while its last batch still waits at H (h0: 15 of 51 rows, 4 launches with carts at
    H; up to 4 carts at H, the farthest back z 4.47, past T's edge; h5 up to 6, z 6.93); every hump of 2
    or more is then crossed (183 to 244 times). The upstream fix is ironspec's GO-needs-D_arr-low
    ruling (cc767b78 modelling it); the hump is sized again on it.
  - the router switch S at the hump's top (7b90c3f1's tripwire interlock: string y 8 over z 15..17,
    anchors z 13 and 19, DP (-3,5,18), DEP_A (-3,5,14), stem west (-4,7,16)); at hump 4 the same one
    block lower fits except DP, which lands beside the first booster; at 3 DP lands on a booster; at
    2 and 1 S is face-adjacent to a booster.
  - size: the loop file +24 blocks over C'' and 1 higher (the hump's top y 7); in the assemblies +1
    in y if their z holds (day-1 checking): 48,384 (+2.9%) for today's stand, 51,072 (+2.7%) for S3
    (on hold until the height is set).
  - hump 4 with its first booster flat (`--boost-flat 1`; 7b90c3f1: DP two before S must not touch a
    powered rail), wip/approach-c2-hump4-b-boosted.txt (80d9c6903): the head slot moves north 5 (loop
    z 24, inside GO's chunk), S (-3,6,15), DP (-3,4,17) between plain slopes, the tripwire at y 7
    (hooks z 13/17, anchors z 12/18), DEP_A (-3,4,13). ironspec's expected pick, conditional on the
    head model's GO-needs-D_arr-low rows. A margin (`--summon-at 93 --pocket-release 14`, 27 corners,
    worst): 27 (all-slopes hump 4 29, hump 5 23, C'' 40). The pocket release changes nothing on the
    meter bench (its stand REL is bench-driven), nor does `--rel-sweep`.
  - hump 4 (flat booster) rows on the same bench (4 seeds): the (c') departure's dip stops (z 9.5,
    10.5, 11.5, at rest and crawling) 0 meetings, 0 merges; the hump chunk frozen 240 gt at every gt
    the train crosses the boosters (+3..+30): 0 meetings on the flat booster or after it, the one
    meeting a restart catch-up on the head slot's slope (+9: 0.374 onto 0.148, both moving); the
    stand chunk's 240-gt thaw (chunks 0,0,0,0) 14 climb meetings and 7 pushed back over T (the head
    model's second train, as above). Riderless (a consequence, not a bar: the siding releases ridden
    carts only): an empty cart clears the hump's top at 0.09-0.12; 4 merges with an empty 1st or 2nd
    cart.
  - gravity places (tick-exact's rollback rule: an unpowered powered rail holds a cart): every cell
    the approach rolls a cart by gravity is a plain rail (the falling face, the dip, T, the descent,
    the snap turn); every booster on the run and the climb is powered=true over a redstone block.
- **h0, the one S3 file's approach (ironspec Nov 11 ~15:00, against hump 5).** `queue-loop ...
  --approach 5 --approach-climb-boost --boost-first 3 --dip-run --top-flat N`: the hump's path with
  no hump: the run's flat boosters, N plain flat cells at y 1 (S's), one slope up to the dip at y 2
  across z 9|10, the boosted climb, T (-3,6,4). S (-3,1,12) on plain flat, DP's plain cell
  (-3,1,14) (7b90c3f1's cross-track tripwire at y 2, anchors x -6 and -1).
  - On cc767b78's head model (s2 266738512e: (i) the D_arr gate, (ii) the liveness-gated stale
    clear, RETRY on D_arr, day-1's one-shot L_X; `--dep-x 14 --go-lead 2`) with ironfarm's validated
    set (release start 4, stagger 4, offs 42,48,44, advance 32/24, pulse 10, pocket release 14,
    `MCTAS_METER_G2_DELAY=4`, meter 12,12,0,0,10,0,40,-1,1,12,0,0,1): A's summon margin over S3's 27
    corners 40..48 gt at h0 against 26..34 at hump 5 (day-1's S count 1 lower); router S flipped
    1,260 times on each with no cart in its cell, ridden or empty (`--s-cell`); an empty cart
    crosses S at stored >= 0.51 on h0, 0.05..0.45 over hump 5's top (riderless rows: hump 5 never
    self-clear in 7-12 of 27 corners, h0 in 0-5).
  - The head at z 22 (`--top-flat 5`, `wip/approach-c-dip-run-h0-z22.txt`): at z 20 (`--top-flat
    3`) the chunk line z 25|26 fell inside a register slot (a plain slope | a powered slot) and a
    200 gt freeze of chunk (0,1) stacked carts there (4 merges, 2 back over T, seed 1); at 19 or 22
    the line falls between a powered slot and a flat peak (0 merges there). A 38 gt (seed 1).
  - `mctas mobfarm borders <design>`: no head z puts plain unpowered cells on both sides of z
    25|26 (the register is all peaks and slopes); the tail's crossings, approach 5's dip line (the
    climb 1 cell past z 9|10; C'' passes, its climb 2 past) and A's departure crossing fail the rule
    as written on every file. ironspec's call: the gate route (day-1's liveness gate, freeze rows at
    every phase) or geometry.
  - Bench artifacts found and fixed (all emulated entity moves into chunks not ticking entities,
    farmopt's `queue::emulated_move` rule): the head model's EMP_X re-arm read the occupancy snapshot
    of the last REL (a DEP_X crossing after the stand's SE held A: hump 5's 3 of 6 golems); virtual
    B returned its parked carts into a frozen crest, or behind A's batch stalled at a frozen border
    (every "freeze 200 stand" merge at the crest); the perfect-tail reset placed carts in a frozen
    chunk (merges at the thaw). Left, real: a post-thaw merge on A's descent into H (H's 0.2 bar).

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
- The runtime check (`declared::check_runtime`, main b712c807d) runs every tick in linear-bench:
  the brakes are declared bench-set (`~`: no control line, so any power reaching them fails the
  run). 8 seeds x 1 day, all three placements: 0. Positive control: a redstone block under the
  first stand rail fails at gt 201 ("2,0,0: powered by a neighbour (it has no control line)", and
  the two rails beside it through it).

## The stand-fix options, in emulation (Nov 11; `queue-loop-run` on approach 5 + crest + C′, aligned frame `--shift 6,6`)

None of these is built; each is a bench emulation (`queue-loop-run` flags on
explore/farmopt-stand-faults) measured against the brief's fault rows.
- **(a) the two-part release** (`--split X,Y,Z,B`: G2 shut behind pocket 3's departing cart until
  REL goes off or B gt from its rise), with ironfarm's escapement tail (escG6L, 14 slots): after a
  missed REL only B = 42 passes the hard lines (40 and 44 each fail a stagger), and the batches
  wobble (groups of 0, 2, 3) to slot 18. Recorded as measured, not pursued. A missed REL absorbs its
  own surplus: a skip request after it changes only its own log line, while the same request in a
  normal run (the positive control) makes the 2-cart release it should.
- **The interlock** (`--interlock 0`, ideal edge latches): on the gates' launches it keeps normal
  timing but closes nothing; on the gates it cascades the release (departures 16..18 gt, divert
  envelope +5/+11) and the stand stops capturing whole batches. On arrivals only, keyed on a resting
  cart (the pocket past the gate occupied and the gate beyond it shut; `--interlock-arrivals`), it
  leaves normal flow unchanged at 3 REL corners x staggers 2/3/4/6.
- **The entry hold** (`--entry-hold X,Y,Z[,H]`: while pocket 3 holds a resting cart, a train is held
  before the stand). On the flat top (-3,6,4) it fails: the carts behind the held one stall on the
  climb's last slope and roll back onto the boosters. On the last descending slope (-3,3,1), against
  the drop turn, the held train rests: front z 1.49, cart 2 z 2.29, cart 3 z 3.17 (below the flat
  top), all at speed 0.00, in contact, pressed by gravity; 0 merges, 0 rider-ticks walking with a
  reachable path, the HOME stamp never set; it restarts by gravity. Normal flow never meets it (slack
  >= 21 gt at every corner and stagger). Opened as soon as pocket 3 clears, the held train runs
  through the stand while REL still holds G0 open (4..6-cart bursts after a missed REL); an added
  "shut for H gt after REL's rise" passes the missed-REL rows only at H = 15, a single
  non-monotonic point, so it is no design.
- **Ending REL on cart evidence** (`--rel-end-rear 1,-1,0,30,80`: 30 gt after pocket 3's departing
  cart enters (1,-1,0), the one cell only it crosses while the release is open): normal flow 0
  problems at every corner, a constant 43 gt open; with the gate, (i) and the downslope hold, ironspec's
  fault set at all 12 corners (seed 1) leaves 12 hard failures outside the partial freezes, 7 with the
  quiet-entry unset-pass and the pocket-1-evidence latch set. The partial freezes split trains, and
  the stand's 3-cart logic then meets partial batches.
- **Sensing at today's spacing** (arrivals 9..15 gt apart, departures 10..13; a detector re-checks
  every 20 gt, so an on-edge is evidence only for the first cart after it was last off): D1, the rear
  exit's edge, first-edge triggers and busy levels have real sensors; pocket 2/3 occupancy, a
  pocket-1 evidence independent of D1, an in-gate guard at G2 (a powered slope) and the hold's
  pocket-3 input do not. A pairwise sweep of the release gaps (G12 3..7 x G23 2..8, 3 corners) fails
  the hard lines at 24 of 105 pairs on the full emulated stack.
- **S1t** (metering the train at the hold, one cart per 12..14 gt, tripwire evidence): out. A tripwire
  sees riders, not carts, and a riderless cart reaches the stand (main's bed stop leaves its napper
  behind in 6 of 40 mid-nap reloads; dismounts, kills): its count breaks.
- **Impacts off boosters:** a cart's stored speed builds to 2.0 on boosters while its movement per
  tick stays capped at 0.4, and collisions use the stored speed, so a stopping point fed off a boosted
  run (the pockets fed off T, any hold) takes much harder impacts than the visible speed shows
  (ironspec; 1.33..1.36 b/t measured into a held train).

## What did not work: S2, the comb stand

Basis: explore/farmopt-s2 da8124a1d (`queue-loop --stand-comb 1 [--comb-gravity]`, designs s2c1 and
s2c1g: approach 5, crest 3, shift 6,6), on the OLD 20 gt bench (REL 3 gt early): not validated.
Each pocket on its own branch, reached through rail junctions (T at the climb's top, S_y after it)
flipped by single-cart detector edges in a 3-state ring.

- **The closing cause: flip windows.** A switch flips between consecutive carts, and each cart's box
  holds the cell about 2.5 gt either side of its centre, so the window is the gap less about 5 gt;
  2 gt each side needs ≥ 9 gt between carts and an exactly timed flip. On main's train (about 11 gt
  at T) the margin was already 1 gt before the next cart (T, on D_P3's edge +2). Approach C's
  arrivals reach T 4..5 gt apart: no window at all.
- After a stand-chunk freeze (level 32, 40 gt at slot 1 +20..+25), bunched trains flipped T and S_y
  with a cart in the cell (1..2 a row).
- **False arrival edges from junction snaps:** a departing cart crossing J1 from its unconnected side
  is snapped to J1's +z edge, inside the box of the detector on J1's +z neighbour (E1): a false
  arrival edge, and G0 shut on a departing cart (a real detector triggers the same way). Now
  ironspec's rule: no detector a junction-crossing cart can reach.
- **Wrong-side entry at J1/J2:** with the gate slopes powered while open, entries ≥ 0.328 b/t and
  exits ≥ 0.319 (18 crossings, 0 problems); with gravity pockets, 0.039 b/t at J1 against the 0.1
  bar (departures stall: a 4-cart window, gates shut on carts, 31 problems in 6 slots), since each
  pocket climbs a block back after two down.
- Exit edges: the only strip cell between a gate slope and the next junction sits beside that
  powered slope; a clean exit cell needs one more flat per pocket (stand area x −4..11).
- Cycle line (stem-in S_y): batch at rest by +92..+94, 4..6 gt before the summon tick; arrival
  detectors ≥ 2 cells from a junction would cost 2.5..5 gt more.

What carried over (to S3(ii)):
- **Gates shut on the next arrival's edge:** G_k opens at REL and shuts on its line's next arrival
  edge E_k, a line gate GF_k upstream of E_k: 3 gt from the edge to the shut, ≥ 33 gt from the last
  departure; at most 3 out per window by construction.
- **The head-launch latch** (the head launches only once the stand emptied since its last launch)
  never bound: slack 71..87 gt.
- **Per-pocket occupancy from single-cart cells:** one latch per pocket, set on its arrival edge and
  reset on its exit edge, drives the line gate's reopening and the head latch's "stand empty".

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
