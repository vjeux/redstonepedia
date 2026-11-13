# ironfarm track notes (iron farms on redsim's mob layer)

Brief: [designs/briefs/farm-iron.md](../../../../briefs/farm-iron.md) (G1 approved, revision 8:
tradeoffs, not pass/fail). Judge: `circuit: mob-farm iron_ingot [day-1|middle|fastest]`
(crates/circuit/src/studies/mobfarm.rs) on redsim's mob layer (crates/redsim/src/entity/mob/).
Tools: `mctas mobfarm gen|exit|run|probe|validate|sight` (crates/mobfarm).

## The stage picks (brief revision 8), design files with their redsim cards in the header

| stage | file | layout | redsim card | rank |
| --- | --- | --- | --- | --- |
| day 1 | `d1-bed-plate.txt` (`gen l6p-pad`) | the bed plate: U1's wooden plate holds the hide open by day; at REST it lies down, the plate lets go, the scared villager sleeps and wakes scared; golems burn in a 4 x 4 pit's corner under 1 lava source | 243 iron/h (241-245), 51%, 8 iron, 1 lava, 318 blocks, restarts after a missed night and after days away, robust | about 30 min to a 24-iron kit (the archived L3: about 54, 34 of them its 16 lava trips) |
| middle | `m2-open.txt` (`gen m2open`) | the bed observer (a 48 gt hide when U1 lies down) over L2d's dry hole, the drop walled only in its bottom 4 layers | 439 iron/h (435-443), 92%, 16 iron + 2 quartz, 4 lava, 404 blocks, restarts both ways, robust | a 33-min build at 439 iron/h |
| middle (walled) | `m2-bed-observer.txt` (`gen m2`) | M2, the drop fully walled | the same rate, 638 blocks | a 41-min build |

Both picks still need the game gate before they are archived (the one rule). The late search
(LB 0.69 iron/h per block flat, M2-open copies 1.09, LS-open 0.94 at N = 1, LT 0.84, TZ 0.63) is in
the brief's Late table.

## The tiers are kinds (site pages)

vjeux via the decisions inbox (2026-11-08): one site page per tier, like a crop farm's one kind
per crop. The tier is the kind's last word: `iron-day1` is `mob-farm iron_ingot day-1`,
`iron-middle` is `middle`, `iron-fast-module` and `iron-scaled-2x2` are `fastest`; `iron-obvious`
has no tier (catalog only: the site publishes no tierless mob farm). The site ranks a tier's
designs by the tier's rule (crates/site/src/kinds.rs `mob_farm_rank`: the least iron to build,
the most iron an hour per block, the most per group of villagers; then blocks), so the fastest
tier's pick is the module and its scaled build is linked from its page and back (`scaled_variants`).
Pages: `circuits/mobfarm/iron-day1.html` (its live URL kept), `iron-middle.html`,
`iron-fast-module.html`, `iron-scaled-2x2.html`. A mob farm's page says it is tested on our
simulation of the mob rules (its caption gives the game check), that the 3D view runs it with that
simulation (the judge's own `farm::Run`, its first run, tick for tick: mobviewer, docs/webrender.md),
where the chest
goes, and, for the day-1 farm, the first night's hand. Re-judged with the tiers, every card is the
same (the metrics byte for byte; only the kind and axis columns changed).

Pages at the crop farms' bar (the parent's follow-up, 2026-11-08): a mob farm's title reads like a
crop farm's ("295 iron an hour · 62% of the ceiling · 189 blocks · 1.6 per block · 9 iron · golems
burn in lava pits": crates/site-model html.rs, the `mob-farm` arm of `circuit_props` and
`mob_farm_title_tail`; the last words are the design's `how:` line, carried in
`Circuit::feature`), its second line the golems an hour, the villagers and the restart. Every
farm now holds its single chest at `port output` (`mctas mobfarm edit --chest`, and `gen` puts it
in): the download is the whole farm, the 3D view shows the chest (no test lamp), and the judge uses
that chest. Re-judged: every card the same but blocks + 1 (the chest), iron-day1's iron per hour
per block 1.57 -> 1.56, and iron-scaled-2x2's bill `4 zombies` (it read `4 zombie`). The game
checks stand: the game runs always had the judge's chest in that same cell. The 3D view's badge
(`not simulated: lava` / `red_bed`): only the mob-farm pages carry lava or beds; redsim on main
parses both (the mob layer), the committed viewer bundle (web/viewer, webrender's) predates them,
so teaching the viewer is a viewer rebuild in webrender's lane (asked through the coordinator).
The filtering UI is rebuilt from source for this deploy (a user-local rustup with wasm32 and
wasm-bindgen 0.2.129 in ~/agents/ironfarm/toolchain on devvm69776, no dotfiles touched:
`env PATH=$TC/cargo/bin:$PATH RUSTUP_HOME=$TC/rustup CARGO_HOME=$TC/cargo mctas site build ...`).

## Commands

- `mctas mobfarm gen <layout> [--pits N] [--pad N] [--floor Y] [--villagers 3|4] [--grid G] [--tier T] --out F`:
  `l1` (the bar), `l2` (wet hole), `l2d` (the dry-hole module), `l3` (day 1), `l4` (two banks of
  fence-gated platforms), `l5` (a G x G grid of l2d modules at a 27-block pitch); `--tier` writes
  the tier on the `circuit:` line. Every generated farm holds its chest at `port output` and a
  `how:` line (how it works in a few words: the end of its site title).
- `mctas mobfarm edit <design> [--chest] [--how TEXT]`: edit a design in place, its caption kept:
  `--chest` puts the farm's single chest at its `port output`, `--how` sets its `how:` line.
- The site's pages of the farms: `mctas site build --out DIR --pkg-ref origin/gh-pages` (no wasm32
  needed while crates/site-model and web/site are unchanged; with them changed, the UI is built
  from source: on devvm69776 a user-local rustup with wasm32 and wasm-bindgen 0.2.129 sits in
  `~/agents/ironfarm/toolchain`, used as `env PATH=$TC/cargo/bin:$PATH RUSTUP_HOME=$TC/rustup
  CARGO_HOME=$TC/cargo mctas site build --out DIR`), `mctas site read --dir DIR --page
  circuits/mobfarm/<file>.html --details`, and the WebKit film `mctas site smoke --dir DIR --engine
  webkit --page circuits/mobfarm/<file>.html --frames 2` (`--full`: the page from its top, one
  window at a time; WebKit installed on devvm69776 with
  `sudo dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb`, docs/site.md).
- `mctas circuit sim <design>`: the whole judge (4 rotations x 2 positions, 8 seeds x 5 days
  each, the golem exit flood, the 9 worst-policy runs, the restart probe), 3-30 s.
- `mctas mobfarm exit <design>`: the golem exit flood alone (spawn spots, positions, drift runs);
  a failure names where the golem ends.
- `mctas mobfarm run <design> --days 2 --seed 1 [--still]`: one redsim run and its misses.
  `--log <kinds> [--log-box x0,y0,z0,x1,y1,z1] [--log-from T --log-to T] [--log-moves]`: the run's
  entities of those kinds (`villager,zombie_villager,iron_golem,item`, `item:iron_ingot`, `all`)
  as they appear and go, their cell changes and their mob events (deaths, hurts, drops, pickups,
  burns: a villager discarded with a zombie villager added at its spot is a conversion);
  `--natural` (in terrain) the site's seals with each cell's natural block, and with `--watch` the
  watched cells' natural blocks (farmopt 2026-11-10: bed-plate-lava-out-of-sight's stranded drop).
- `mctas mobfarm probe <design> --row x0 x1 y z`: the built farm (water settled) along a row.
- `mctas mobfarm validate <design> --days N [--repeat R] [--kill] [--trace]`: the design run in
  the real 26.3 server through the bridge (R runs, each its own sandbox seed), against 32 redsim
  runs: golems/h (the game's mean and its 95% interval, redsim's one-run window), the day/night
  split, the spawn-cell chi-square. A 3-day run takes 15-20 s.

## The designs (redsim judge; boats need boat-leads' push, so these ran on 2d8143b2 locally)

The bills below count the bucket but not the other tools; since the stage card lines (brief
revision 5) the judge counts every tool the build needs (shears for leaves, an iron pickaxe for
redstone dust), so the archived cards read L3 11 iron, L2d 18 and L5 2x2 48. The stage lines on
every card: tools, lava sources, the materials' stage, chunk columns, storage hours, hoppers per
1000 iron/h.

Day 1 under the stages (revision 5): D1, the bed plate (`gen l6p-pad`, crates/mobfarm/src/day1.rs):
of 3 villagers only S sees the zombie; U1 stands by day on a wooden pressure plate by its bed, and
the plate powers the stone under it and the top-half trapdoor in S's line of sight (open: S sees).
At nightfall U1 lies down, the trapdoor shuts, S calms and sleeps; S's sleeping eye sees under the
slab, so it wakes and spawns at night too. Golems spawn on L2d's dry-hole pad and fall into a 4 x 4
pit whose single water source pushes them into the corner under one lava source held by 3 wall
signs, over one hopper. The whole judge: 243 iron/h (241-245), 61 golems/h, 51%, restarts after a
missed night, robust; 8 iron (bucket, hopper), 1 lava source, no quartz, 318 blocks in the void.
The same golems with the weather cycle and a forced day of rain or thunder (the infra track's
model, merged locally): the calm reads no sky. `mctas mobfarm sight <design>` shows each
villager's line of sight to the zombie with the trapdoors shut and open (and the first block that
stops a blind one).

Middle under the stages (revision 5), measured by the whole judge (64 runs, 8 builds), each the same
with the weather cycle and a forced day of rain or thunder (the infra track's model, merged
locally); both over L2d's dry hole and a chamber whose lava cannot light anything (lava only in
its north row, the open gates beside it right under the ceiling, no mouth gates: every first step of
`LavaFluid.randomTick` is the ceiling or the wall):
- M2, the bed observer (`gen m2`): an observer watching blind U1's bed head toggles a copper bulb
  when U1 lies down; a comparator reading the bulb opens the hide trapdoor and feeds 12 repeaters
  (48 gt) back into the bulb, which toggles it off. The three scared villagers fall asleep in the
  48 gt, then panic all night: 439 iron/h (435-443), 110.5 golems/h, 92% of the ceiling, robust,
  restarts after a missed night; 16 iron (bucket, iron pickaxe, 2 hoppers), 2 quartz, 4 lava
  sources, one chunk column (16 x 16), 638 blocks, 4.1 h of storage. The middle pick.
- M3, the bed plate (`gen m3`): 378 iron/h (372-383), 79%, robust, restarts; 13 iron, no quartz.
  The hide's open slab stops the x 0 villager's line (it meets the cell's west edge), so 2 see.
- The sleep check now fails a run when a villager has missed a night (no sleep for 30000 gt): M2's
  villagers sleep once a night, a few ticks later some nights, and the old 24000 gt bound failed
  2 of 8 builds on a tick or two of unwillingness that the rate already counts.
- The absence probe (`farm::run_away`, entity/mob/absence.rs; the card's "restarts after days
  away"): a day of the steady state, 3 days away (chunks unloaded: fresh randoms, villager brains
  rebuilt with only the codec memories, `refreshBrain`'s schedule activity and sensor delays; or
  loaded but not ticking), back at noon or at midnight, then 2 days. M2 restarts in all four: the
  first golem 0.0-0.1 h after the return (at night the reloaded villagers sleep before their
  first hostile scan; by day they wait for the evening's hide).

Late under the stages: M2 stacked in one chunk column (`gen stack --modules N --pitch 43`), each
module's golems never in another's villagers' follow box (the chamber above starts 43 over the
villagers below: their box ends 2.51 + 16 x 1.11485 above their feet). Runs of 5 days: N = 2,
871-898 iron/h (each module 92-94%); N = 4, 1765-1793 iron/h, all in one chunk column against the
27-pitch bar's 154 per chunk column (M2 at 27 x 27).

| design | file (gen) | villagers | iron/h (95%) | share of ceiling | blocks | iron/h per block | bill |
| --- | --- | --- | --- | --- | --- | --- | --- |
| L1, the obvious bar (trench) | `gen l1` | 3 | 139 (138-141) | 29% | 386 | 0.36 | 13 iron, 3 quartz |
| L3, day 1 | `gen l3 --pits 8` | 3 | 295 (292-298) | 62% | 189 | 1.56 | 9 iron |
| L2, wet hole 6x6 | `gen l2 --pad 6 --floor -7` | 3 | 358 (357-360) | 75% | 496 | 0.72 | 13 iron, 6 quartz |
| L2, wet hole, 4 villagers | `gen l2 --pad 6 --floor -7 --villagers 4` | 4 | 369 (365-373) | 77% | 516 | 0.71 | 13 iron, 6 quartz |
| L2d, dry hole 6x6 | `gen l2d --pad 6` | 3 | 423 (418-427) | 88% | 556 | 0.76 | 13 iron, 6 quartz |
| **L2d, dry hole 6x6, 4 villagers (middle)** | `gen l2d --villagers 4 --pad 6` | 4 | 443 (439-447) | 93% | 579 | 0.77 | 13 iron, 6 quartz |
| L2d, dry hole 8x8 | `gen l2d --pad 8` | 3 | 431 (424-437) | 90% | 600 | 0.72 | 13 iron, 6 quartz |
| **L2d, dry hole 8x8, 4 villagers (fastest module)** | `gen l2d --villagers 4 --pad 8` | 4 | 459 (454-464) | 96% | 623 | 0.74 | 13 iron, 6 quartz |
| L4, fence-gated banks | `gen l4` | 4 | 444 (440-448) | 93% | 950 | 0.47 | 53 iron, 6 quartz |
| **L5, 2 x 2 L2d 8x8 modules (fastest, scaled)** | `gen l5 --villagers 4 --pad 8 --grid 2` | 16 | 1836 (1825-1848), 459 per module | 96% | 2492 | 0.74 | 43 iron, 24 quartz |

Every one is robust (8 builds), passes the golem exit flood (every position a golem can stand on,
its edge spots included, still and 8 walking inputs, dead within 1200 gt: worst 241-581 gt) and,
but L3, restarts by itself after a missed night. The design files are in this directory: `iron-day1.txt` (L3, archived, passes the real-game gate); in `rejudged/`: `iron-obvious.txt` (L1),
`iron-middle.txt` (L2d 6x6, 4 villagers), `iron-fast-module.txt` (L2d 8x8,
4 villagers), `iron-scaled-2x2.txt` (L5), taken out of the archive on 2026-11-08 after the
real-game gate (mobinfra: unmodified 26.3 server, Hard, 3 days + a forced day of rain and of
thunder) failed them: lava's random tick lights the L2d chamber's oak fence gates (fire spreads
only with a player within 128 blocks, so the playerless bridge never showed it), the lava floods
and burns the drops; rainy dusks and thunder stop the dusk pulse; lightning made witches and broke
a boat; L1 left a golem alive in a rotated build and stops in rain. The tiers (brief revision 3):

- day 1 = the lowest bill with no quartz and 25%+ of the ceiling: L3 (9 iron, 62%).
- middle = iron/h per block among builds at 75%+ of the ceiling with a bill of at most 20 iron:
  L2d 6x6 with 4 villagers, 0.77 (its 3-villager twin 0.76 is a tie within the noise; the 4th
  villager makes it the faster of the two). Its share's interval, 92-94%, clears 75% by far
  (L2's wet hole sat on the floor: 75%, interval 74.9-75.6%, not published).
- fastest = iron/h per module: the L2d 8x8 module with 4 villagers, 459 iron/h (96%); scaled as
  L5 = 2 x 2 modules: 1836 iron/h, 43 iron, 24 quartz, 42 x 28 x 44.

Ceiling: one golem per 600 gt per villager group (120 golems/h, about 476 iron/h).

## How they work

- L3: three villagers on their beds in closed 1-cell pods under leaves; a zombie in a boat 7
  north; 8 spawn pits in a row, each a 1x1 floor with lava on both sides at head height (held by
  stone, capped by glass); one tripwire through every pit; its hook opens the trapdoor in front of
  the zombie while any golem burns (villagers calm down, at night they sleep); a water channel
  under the string carries the drops to one hopper. Hide the zombie by hand on the first night.
- The dusk pulse (L1 has a plain daylight shutter): two inverted daylight detectors switch on
  together at nightfall; D2 (dust into B1) turns torch T1 off at once, D reaches K0 through 6
  repeaters; the torch on K0 is lit for 46 gt between the two and opens the trapdoor(s). The
  villagers fall asleep during that hide (`LAST_SLEPT`) and wake scared for the rest of the night.
  No golem is needed for it: the farm restarts by itself after a missed night. 6 quartz, no iron.
- The 4-villager front: pods at x 0, 2, 4, 6, the zombie at 7.6 blocks from the far ones; three
  trapdoors 1.5 in front of its eyes (every ray crosses them), opened by dust on the blocks above.
  Only three villagers must agree, so a golem one of them sees does not stretch the cycle.
- The dry hole: a pad with water from its corners (one source per corner: no flowing cell ever
  touches two sources, which would make the whole pad a source and still it); the hole's 4x4
  middle has an outer ring of water over open fence gates (the water stays, a golem sinks through)
  and a dry 2x2 center (open fence gates at the water level stop the water). Each golem leaves the
  stone while still in water and falls freely from the dry center down a 4x4 shaft, 16 blocks,
  out of every villager's follow range; at the bottom, water from the shaft's two south corners
  (its levels fall toward the north row's middle) pushes it north into a chamber as wide as the
  shaft: lava at head height held by open fence gates below it and at its mouth, a ceiling right
  above it (a golem that jumps in the lava cannot float up with its drops), two hoppers under the
  middle of its north row (the chamber's water carries the drops there). 13 iron.

## What failed on the way (and why)

- A pad with sources along its edges: every flowing cell next to two sources became a source, the
  whole pad turned to still water and golems were pinned (seen with `mobfarm probe`).
- A hole off the pad's center: the currents meet on the pad's midline, half a block from the
  hole's center, and the golem rests on the hole's rim.
- A lava blade at the bottom of the shaft with nothing above it: a golem that jumps in lava floats
  up (`jumpInLiquid`) and its drops burn. The kill needs a ceiling right above the blade, so the
  golem must come in sideways: water pushes it from the shaft's bottom into the chamber.
- Water over the hole (L2): a golem sinks through water slowly (gravity / 16) until its whole
  2.7-block body has left it, about 100 gt in the villagers' range: 2.6 sightings per golem, 75%.
- Walled 2-wide platforms: a golem spawns at a cell center, 0.2 into the next column; a wall there
  stops it at the next wall cell ahead. Walls must stop where the golem goes on; fence gates do the
  job where water must be held.
- A drop where the floor and the water end together: a golem walking against the current hovers
  at the edge. Out of the water it walks on land (0.25 x friction, not 0.02 x 0.25 in water), the
  floor is still under its box from the last tick and it steps back up. The edge row of water over
  fence gates (no land at the edge) fixes it.
- The first L1 (water from a pad's corners to a 2x2 hole in its middle, wall signs in the hole): the
  game measured 14 golems/h against redsim's 37. Traced in the game: golems stood still for
  thousands of ticks at the hole's rim, at (3.30, -3.00, 5.70), with full health. There the box still
  overlaps the rim's floor by less than 0.001 (collisions shrink a box by only 1e-7) while the fluid
  box (the box deflated by 0.001) sees only the hole's cells, whose four currents cancel: no push,
  and the floor holds it. The exit flood now also starts golems at every reachable edge spot (box
  sides 0.0005 inside or outside block boundaries, each axis and both), and it finds that exact
  spot in the first L1; every other design still passes. L1 was rebuilt with a one-way current
  over a trench (the current never stops over a floor) and open fence gates for the pad's side
  rims (a golem spawned 0.2 into a 1-thick wall slides through it: a box ignores the shapes it
  already overlaps), glass one block further out.
- A bottom with sources on all four corners: the currents meet in the middle and pin the golem;
  a chamber narrower than the shaft: a golem against its side wall is stopped at the corner.
- 1-high glass rims: golems step over them (step height 1); rims are 2 high.

## Game validation (the brief's one-off checks)

- L1, the trench (the bar), 8 game runs of 3 days on its file: game mean 35.0 golems/h (95% of the
  mean 34.1-35.9) vs redsim 35.7 (sd 1.1). Day/night 278/2 (redsim 0.02 night). Spawn cells
  chi-square 22.42 over 20 cells (19 df, 5% critical 30.1); 3.9 iron per golem.
- L3, 8 game runs of 6 days (repeat 8): game mean 72.8 golems/h (95% of the mean 70.3-75.3) vs
  redsim's 74.8 (sd 3.0 per run): the same design, the same start state (the steady state, day
  time moved by hand), the same day count. Day/night 579/586 (redsim 50/50). Spawn-cell chi-square
  6.25 over 8 cells (5% critical 14.0), 1165 golems, no cell the game used that redsim never did.
  3.97 iron per golem. The first single run (68.5/h) was the low tail: game runs spread 68-79.
- L2d 6x6, 4 villagers (the middle), 8 game runs of 3 days on its file: game mean 114.5 golems/h
  (95% of the mean 113.1-115.9) vs redsim 112.3 (sd 3.4): redsim reads 2% low. Day/night 451/465.
  Spawn cells chi-square 15.89 over 20 cells (19 df, 5% critical 30.1). (An earlier 3-run check:
  117, 101, 117 golems/h, mean 111.7.) 3.95 iron per golem. The
  fence gates (water held, lava held, golems through), the dusk pulse and the 4-villager front
  behave in the game as on redsim.
- L2d 8x8, 4 villagers (the fastest module), 8 game runs of 3 days: game mean 116.0 golems/h (95%
  of the mean 112.3-119.7) vs redsim 115.2 (sd 4.2 per run). Day/night 461/467 (redsim 0.50
  night). Spawn cells chi-square 49.30 over 48 cells (47 df, 5% critical 64.0), no cell the game
  used that redsim never did; 3.95 iron per golem.
- Found on the way: a bridge sandbox keeps its own game rules, so its day time is frozen
  (`gamerule` acts on the server's rules, not the sandbox's) and villagers never REST; `validate`
  moves the day time by hand (`time add 20` with every 20 gt step).
- The game's villagers are summoned with data, which skips `finalizeSpawn`: their follow range is
  exactly 16 (redsim draws the bonus of a villager brought in by the player).

## Coordination

- Boat-leads (06b77372) owned the boat push (`entity/push.rs`): until it landed (2df4c90f8,
  2026-11-08), main's boat.rs refused any entity in a boat's push box, so the zombie-in-a-boat farms
  ran on a local tree (main + boat-leads' bundle 2d8143b2). Its landed push gives the same numbers.
  Boat-leads carries the fail-closed boarding branch (a living mob a boat could pick up is refused)
  and the Mob leash arm; mob holders and passenger-boat leashes stay unmodeled.
- A persistent zombie without a boat (a name tag, a 1-cell pod) would not need the boat push but
  the zombie's AI walking is not modeled (the judge refuses a zombie that does not ride): its eyes
  would move within the pod. Kept the boat.

## What the next mob farms need from the mob layer

What exists: villagers (brain, schedule, sleep, panic, golem summoning, gossip refusal), iron golems
(spawning, physics in water and lava, damage, death, loot), zombies (riding only), LivingEntity
physics (water currents, gravity in fluids, step-up, jumping in liquids, fall, lava damage and
fire), the steady-state judge, the exit flood under 9 walking policies, the game validator.

| farm | what it needs | effort |
| --- | --- | --- |
| villager breeder | villager breeding (beds, food in inventories, willingness, `VillagerMakeLove`), baby growth, item pickup by villagers; pathfinding to beds (villagers walk) | large: villager walking AI (navigation) is the core gap; a walk-policy bound like the golems' is not enough for breeding pairs |
| trading hall | job site claiming (POI), professions, restocks, trades (no rate to judge but a must-work) | medium: POI acquisition and job blocks; villagers still in 1-cell pods |
| raid farm | raids (bad omen, waves, the raid center), pillager types, totem/emerald loot, witches | large: raid logic and several mob types with ranged attacks |
| mob tower / XP | natural hostile spawning (`NaturalSpawner`: light levels, the mob cap, despawning), zombies/skeletons/creepers/spiders physics and AI walking, fall damage kills, XP orbs | large: light engine coverage for spawn checks, natural spawning, despawning rules, four mob types; the walk-policy bound applies (mobs walk randomly in the dark) |
| creeper farm | natural spawning (as above), cats or ocelots to scare creepers, creeper explosions (exists for TNT) | large (shares the mob tower's spawning) |
| gold (piglin/zombified piglin) | nether portal spawning of zombified piglins on portal blocks, or nether natural spawning; anger and aggro chains; magma/turtle-egg lures | large: portal block random ticks and nether spawning; aggro propagation |

The common first step for every hostile-mob farm is natural spawning with light levels: the light
engine is only partly classified (`daylight.rs` props) and spawning needs block light and sky light
everywhere in the spawning volume.

## The Q2 lane benches' bars: physical or inherited (ironspec Nov 12 19:07, the bar audit)

`mctas mobfarm escape-bench --siding` (crates/mobfarm/src/sidingbench.rs) and `mctas mobfarm
push-scan` (pushscan.rs). The full table (the maintained bar audit, farmopt):
https://artifacts.internalmeta.com/a/2276791626224129; ironfarm's own 31-bar list, with the commit
that set each bar: https://artifacts.internalmeta.com/a/1169491299367704. Every run's last line lists
the inherited numbers again.

- **Physical** (a game rule, or a failure measured at the boundary):
  - touching at the merge and the order of arrivals;
  - carts off the rails, riders' eyes in a block, stray power on R;
  - a napper knocked loose, rails popped, the wrong way at the merge;
  - the push test (boxes 0.98 x 0.7, inflated 0.2) against the waiting napper, for lane carts and through-carts;
  - the siding detector's false trips, carts stacked, HOME lost;
  - the napper stuck on R, a launch after the pulse;
  - the siding's single occupancy: 0 ticks with a cart on R while another rides F or moves north in K's cell (turning toward F). These are push-scan's R (x−2,24,7) <- K (x−2,25,9) pairs (ironspec 19:58), and the bench prints each next napper's K tick against the previous napper's departure off R.
    - **Two defects, fixed after a191f3dfb; every earlier R ← K figure is withdrawn (ironspec 01:33 (2)).**
      - R's cell test used the rounded y, but R is an ascending rail whose rider's y runs 24 to 25. A napper at rest past z 7.5 was never "on R", so the count could not see it.
      - K's half tested position ("north of z 9.45"), which every K-row pass on the north_east T crosses: the pass is projected to z 9 + xf/2.
    - It now tests the cell by floor(y) and the motion's direction. The turn control (`--r-joint-control turn:T`) must bite and the pass control must read 0.
    - "The napper at rest on R" (`nap_ready`, a y range) never read that test;
  - push-scan's swept boxes and its bridged margin > 0.
- **Inherited** (no failure measured at the number), with the physical basis first where one exists:
  - arrivals at the lanes' backs and landings <= 0.46: the brief's gravity-run rule. Physical: the bump bound 0.508, which is official. The 0.46 stands;
  - the calm-REST stamp <= 1,000 gt: a margin under vanilla's tooLongUnreachableDuration 1,200;
  - occupancy 8 and V never held: ironspec's slot-8 rulings;
  - lane carts off the siding: the S-bend ruling;
  - exactly 1 cart per release: the brief;
  - full ring windows: day-1's ring;
  - the delivery 0.30..0.48: the return bench's `v_merge`. Physical floor 0.27 (first failure 0.22).
- **Mine** (inherited): the windows at rest off the rails (20 gt), V held (20 gt), the launch (40 gt), stray power past the drive delay (+ 2 gt), and push-scan's creep allowance along the rest's own axis (+ 0.5).
- **The S-bend's geometry is the module's** (ironspec 20:40). D is an ascending_south detector with glass at (x,24,9), as in 7b90c3f1's split row. The old flat D and flat rail at (x,24,9) stay only as the labelled control `--flat-d`. The flat D moved the leader one tick later to the drop's lip: that was the bench's whole 1-gt offset from tick-exact's official row.
- **The pass is dated at the press tick.** The state the bench reads before a tick is the previous tick's, so the press happened then. The siding's switch comes `k_sw` gt after the press and R powers 8 gt later, as day-1's controller builds it (R at D + k_sw + 8).
  - The replica of tick-exact's official row matches it tick for tick: both carts, the press tick, and stuck at D − 5 / clean from D − 4 (ironspec 21:07).
  - The unit test `official_sbend_boundary_and_k4` pins it: D − 5 stuck at curve rule 5, D − 4 clean at 6, k_sw = 4 gives 14.
- **The curve rule** (`--curve-bar`, default 0: printed only) is information since ironspec 22:31: the S-bend's gate is the lip margin. The interim bar 11 (the band's largest curve-rule boundary over the qualifying phases, 9 at arrival 0.15 rolling, + 2) is retired onto it, with 8 and 21 before it.
  - The qualifying phases have PRESENT at least 7 gt before the leader's press; each phase prints its margin, and those under 7 are printed apart as outside normal operation.
  - `k_sw` = 6 (ironspec 21:35; `--nap-k`, default 6). k_sw = 4 missed the interim curve bar under 1–4-gt freezes at rest (least 10). k_sw = 6 is re-judged against the lip bar.
- **The lip margin** (`sidingbench::lip_margin`; adopted ironspec 22:22) is the gate quantity at the point of contact, with a **bar of 2** (`--lip-bar`, default 2; ironspec 22:31).
  - **N** is the first tick whose napper move brings its box front to the leader's rider's box back face (within 1e-4), as the rider stood after the previous tick (napper first), over the lip region (the merge's south face to 3 cells north).
  - **R** is the first tick after which the rider's box top is below the napper's floor y 24.0.
  - The margin is N − R − 1. −1 or less is contact (stuck under napper-first).
  - **Three outcomes** (`Lip`; ironspec 00:02 (1), G1's CHANGES 1 on a191f3dfb):
    - Never: the napper never reaches the rider at the lip. This is clean.
    - Held: it reaches the rider and the rider never drops below the floor. This is STUCK, a failure whatever the bar.
    - A margin.
    - "No cart ahead at the merge" and "no merge entry" are their own no-sample classes, never clean.
    - A napper released but not back at the lane's back by the run's end still gets its outcome.
  - **The positive control `--held-rider-control`** runs through the rows' own code path (ironspec 00:27). At the napper's release it lays glass two cells under the lip, so the cart ahead drops one block and stays there with its rider above the floor. The row must read STUCK and fail.
    - Glass right under the lip holds the cart on the lip instead. The napper then stops against the cart's box, 0.49 behind its centre against the rider's 0.3, and never reaches the rider: a merge queue, not the lip's failure.
  - **The bar's evidence:** the band sweep on the module's geometry over every qualifying arrival and phase (3,200 rows) has clean rows at 0, 1 and 2 and no stuck row at or above 0, so 0 + 2 = 2. At −1 the bench's outcome depends on the tick order (the napper rolls on in half the rows); that is a redsim fidelity follow-up, below the bar.
  - **The known answers, pinned on tick-exact's official per-tick rows:**
    - `lip_margin_on_official_rows`: 0.30 at rest, D − 5 → −1, D − 4 → 0;
    - `lip_margin_on_official_rolling_row`: 0.15 rolling with its K tick 12 gt before the press, P − 2 → −1, P − 1 → 2.
- **Other per-row checks:**
  - The carts' block contacts (ironspec 21:25 (4), with G1's note (c)). The method follows day-1's `note_contacts`.
    - Every cart whose move collided horizontally counts: the lane's carts, the K row's through-carts and the control cart.
    - The cells are the collision boxes its box meets grown 0.01 in x and z, the floor and the ceiling left out, kept on a face the cart moved toward. The side walls of a 1-wide path and the slope supports a riding cart's box passes through are no stops.
    - Declared: the hold's trapdoor (x,16,0), the drop entry's backstop (x,24,5) (the lanes file's `# contacts:`), and the bench's end of track (x,16/17,−1), a labelled stand-in for the station. Any other contact fails the row.
    - Contacts before t 20 (the summoned start's settle) are printed apart, not dropped.
  - No vacuous passes (ironspec 23:56). Every row prints its bars' samples, and a bar in scope with 0 samples is NOT JUDGED and fails. The matrices print the S-bend gate over their rows: the lip classes, the push passes, NOT JUDGED at 0.
  - `siding_row` requires every bar's line, and a missing or unparsable count is an error.
  - Every control asserts its own expected failure (ironspec 00:02 (3), 00:27):
    - the r_joint turn and pass controls;
    - `--side-ctl`;
    - a non-slime `--r-support`;
    - `--held-rider-control`.
  - The resting napper across the K row's passes (ironspec 23:15 (2)): its displacement and speed during every pass, with a bar of 0. This closes push-scan's R ← K pairs by measurement, as the lanes file's `# push-pair:` declares. The turn control is the positive control.
    - **Each row's computed scope** (ironspec 03:00). Each row prints:
      - the napper's windows on R (in R's cell, resting or rolling onto it);
      - every K-row pass tick;
      - the ticks where the two meet.

      Where they meet, the push is judged. A row where they never meet is "push: out of scope (computed)" only while its K row is alive (G1's guard: a dead K row is NOT JUDGED). Each such row names the push-grid row covering its rest z (`--push-grid`); an uncovered rest fails the row. No flag can declare the scope.
    - **The push grid** (`--push-grid-run`), the citable grid behind `# push-pair:`:
      - every rest position (arrival on K 0.10..0.44 by 0.01 × 8 entry phases, rest z 7.343..7.912);
      - at each one, K-row passes injected at 0.28..0.44 b/gt across 8 entry x's, 1 and 20 gt after the rest;
      - 22,400 passes, all of which met their rest, with displacement 0.000000.

      The table is `push-grid-sbend.txt` in this folder.
  - The napper's settle on R (day-1; ironspec 01:33 (1)). The bar uses PRESENT → at rest, which is the K tick → rest minus 2. The rest sweep prints it per arrival and phase, with the bar it implies, m ≥ max(7, settle − 14) + 2. The first tick on R's cell → rest is printed too.
  - `--out`: the bench's world as laid, for `mobfarm record-match` (ironspec 21:43).
