# ironfarm track notes (iron farms on redsim's mob layer)

Brief: [designs/briefs/farm-iron.md](../../../../briefs/farm-iron.md) (G1 approved, revision 3).
Judge: `circuit: mob-farm iron_ingot [day-1|middle|fastest]` (crates/circuit/src/studies/mobfarm.rs)
on redsim's mob layer (crates/redsim/src/entity/mob/). Tools: `mctas mobfarm gen|exit|run|probe|validate`
(crates/mobfarm).

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
villager's line of sight to the zombie with the trapdoors shut and open.

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
but L3, restarts by itself after a missed night. The design files are in this directory: `iron-obvious.txt` (L1),
`iron-day1.txt` (L3), `iron-middle.txt` (L2d 6x6, 4 villagers), `iron-fast-module.txt` (L2d 8x8,
4 villagers), `iron-scaled-2x2.txt` (L5). The tiers (brief revision 3):

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
