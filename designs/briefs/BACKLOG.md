# Backlog: what players need from redstone

Reviewer, 2026-10-21. Per [docs/process.md](../../docs/process.md) step 0: no outside designs, no
internet. Each row is a need a survival player has, the build that serves it, the number a player
would compare, that number's ceiling worked out from the game code (26.3, `mctas src show
<Class>`; derivations below the table), what the site has (main = live site at 0001c5a49; a branch
name = not published yet), and the gap. Ranked by how much a survival world needs it: things every
base builds first, then progression, then convenience and show pieces.

Gap words: **missing** (nothing on the site), **ours worse** (below the obvious design or far below
the ceiling), **too small** (fine per unit, but no full-size build), **at ceiling**, **not judgeable**
(needs players, mobs or villagers that our judge cannot run).

| # | need | build | the number | ceiling (from the code) | site has | gap |
|---|---|---|---|---|---|---|
| 1 | Storage: put loot away by kind | item sorter slices + overflow | items/s into the right chest; blocks a slice | 2.5 items/s per hopper line (C1) | sorter-classic 15 b/slice, storage-bulk; 2.5/s (100%) | at ceiling; a full system (input chest, 8-16 slices, overflow, unsorted chest) is missing as one page |
| 2 | Smelting ore in bulk | furnace array ("super smelter"), 8-16 furnaces | minutes to smelt a chest (1728 items); blocks | 288/N min for N furnaces: 36 min at 8, 18 min at 16 (C2) | smelter-1 (4 b), smelter-clock-2/3/4 (31-63 b), smelter-row(-2,-3) 2-furnace slices 12-14 b/furnace + 125-210 b ends, clocks never stop; ~5.5 items/min/furnace (92%) | ours worse: rate near ceiling but 3x the obvious design's blocks, 38-65 hoppers awake forever; no 16-furnace page with minutes-per-chest. Topic in flight (sorter) |
| 3 | Paper (books, maps), rockets (elytra flight) | sugar cane farm, full size | items/h for the farm; items/h per block | 3.30 cane/h per plant; plants on at most 2/3 of the floor (C3) | 1/2/4-plant harvesters, 12.7 cane/h max (96%/plant) | too small: a farm making 13/h is useless; need 64-128 plants (210-420/h). Topic in flight (clk-rt) |
| 4 | Food, emeralds, compost: melons and pumpkins | stem farm, full size | fruits/h for the farm; per block | 13.2 fruits/h per stem (4 fruit sides, f 7); 4218 slices/h for 64 stems; ~6.6 slices/h per block with the least machine (C4) | 1-stem and 4-stem harvesters; melon-x 60 slices/h (91% of a lone stem) | too small: 60 slices/h vs 680/h for the obvious 64-stem rows (measured, 1.31/block) and ~4200/h ceiling. Topic in flight (w86) |
| 5 | Securing and hiding a base entrance | 2x2 and 3x3 piston doors | blocks/volume; open/close ticks | 2x2 volume 24 is argued the floor (piston-door-2x2/WR.md) | full leaderboards; classic6 2x2 at 20 b | at or near ceiling |
| 6 | Hands-free door | automatic door (plates or tripwire) | blocks; opens before you reach it, never shuts on you | obvious: classic6 (20 b) + plates both sides + inverter ≈ 35-40 b (estimate) | auto-door-lock-v12 91 b | ours worse (2.3x the obvious design's estimate); see AUDIT |
| 7 | Iron, XP, gold, raid loot | mob farms (golem, mob tower, piglin, raid) | items/h | from spawn rules, not redstone | none | not judgeable (mobs, villagers) |
| 8 | Bread, carrots, potatoes | crop farm with replanting | items/h | 1 harvest per 21 random ticks at best (f = 1/3, 7 ages) = 2.5 harvests/h per plant (C5) | none | not judgeable fully (replanting needs a villager or player); a water-flush harvester with manual replant could be judged but saves little |
| 9 | Meat, leather, wool, eggs | animal farms (chicken cooker etc.) | items/h | breeding rules | none | not judgeable (mobs) |
| 10 | Wood | tree farm | logs/h | sapling growth + player chopping | none | not judgeable (player axe; leaves) |
| 11 | Fuel for the furnaces | bamboo or kelp farm feeding the smelter's fuel line | fuel items/h; items smelted per hour of fuel | bamboo 17.6/h per stalk, one stalk per floor cell; kelp 7.4/h per plant (C6) | 1-stalk bamboo (17.2/h, 98%), 1-plant kelp (7.2/h) | too small; no farm-to-furnace link |
| 12 | Moving items up (farm to storage) | item elevator | items/s; blocks per block of height | 2.5/s from one feeding hopper (C1); dropper re-fire 4 gt (C7) | dropper pairs 11 b @4 high, 51 b @20; bubble column 40-125 b; 2.5/s (100%) | at ceiling; fine |
| 13 | Getting between floors | player elevator | seconds per trip; blocks | bubble column: no redstone, rises up to 0.7 b/gt (C8) | flying-machine elevators 161-206 b (10 b up in 7.5 s); 3-floor 599 b | ours worse: a soul-sand column does it with a few dozen blocks; not judgeable as a ride (needs a swimming player) |
| 14 | Travel between bases | minecart line + stations | blocks; stops any cart; one press to leave | — | station-slope 6 b (1-way), 10 b (2-way) | fine |
| 15 | Moving farm output by rail | cart loader / unloader | items/s; seconds per cart | loader: a hopper minecart sucks 1 item/gt from a container above it = 20 items/s, 320 items in 16 s (C9); unloader: the hopper under it, 2.5/s | loader-t5 35 b at 2.48/s (12% of ceiling); unloader-t3 22 b at 2.48/s (99%) | loader ours worse |
| 16 | Bulk transport, trading | shulker loader / unloader | boxes/min; blocks | 1728 items/box: 11.5 min per hopper at 2.5/s; a hopper minecart under a box empties it at 20/s (C9) | 14 designs, best unloader 11.6 items/s | fine |
| 17 | Auto-crafting (iron blocks, etc.) | crafter fed by a hopper | crafts/min | hopper-bound: 9 items at 2.5/s = 16.7 iron blocks/min (C1) | ironblock-v24 16 b at 16.6/min (99%) | at ceiling |
| 18 | Bone meal from farm waste | composter | bone meal/h; items/h it can take | one composter takes ~2 items/s: ~370 bone meal/h from seeds (C10) | compost-row 18 b per composter, one composter per crop kind (explore/sorter) | ours worse: the obvious hopper-composter-hopper (3 b + chest) does a single-crop farm; the filter row only earns its 18 b on mixed input |
| 19 | Potions | auto brewer | potions/min per stand; blocks | 3 potions per 400 gt brew = 9/min per step; 4.5/min for a 2-step potion (C11) | brewer-small 42 b at 1.9/min (42%) | ours worse |
| 20 | Stone, cobble, basalt | generator | blocks/min | lava refills every 30 gt: 40/min per face (mechanics.md tick table) | none | not judgeable (player mining) |
| 21 | Digging and clearing | TNT duper, tunnel bore, world eater | TNT/min; blocks | — | 8 TNT dupers, 200 TNT/min engine 17 b, lever on/off | dupers fine; bores not judgeable (terrain) |
| 22 | Hidden ways in | piston bridge, piston staircase | blocks; one lever | — | bridges 6-13 b 1-wide, 38-46 b 2-wide; staircases 8-18 b (explore/pext) | 1-wide fine; 2-wide ours worse (see AUDIT) |
| 23 | Locking a door | combination lock | blocks per digit | — | dial locks 23 b (1 dial) to 99 b (4); sequence locks 753 / 997 b | dial fine; sequence lock ours worse (obvious estimate 40-60 b) |
| 24 | Showing a number (score, counter, clock) | digit display | reads as a digit from 10 blocks; blocks | — | 7-segment 587 b, one lamp per segment | ours worse: does not read as a digit. Topic in flight (clk-obs) |
| 25 | Timing farms and machines | clocks and timers | period; blocks | — | 835 clock types | fine |
| 26 | Green dye, XP by smelting | cactus farm | items/h | 3.30/h per plant; no block beside a cactus (CactusBlock.canSurvive) | 1-plant 9 b | too small |
| 27 | Moving builds, world-eater engines | flying machines | blocks; speed | 1 block per 10 gt for a slime/observer engine | 13 shown | fine |
| 28 | Building parts for the above | gates, latches, T flip-flop, pulse shapers, counters, decoders | blocks; latency | — | hundreds (pulse 187, memory 69) | fine; parts, not needs |
| 29 | Games, loot rooms | randomizer | blocks | — | 2- and 4-way | fine |
| 30 | Doorbell, music | note block circuits | — | — | none | missing; low need |
| 31 | Getting on top of the Nether roof, clearing bedrock (vjeux 2026-11-06) | bedrock breaker ([brief](bedrock-breaker.md)) | blocks; armed every (gt) | one swap piston arms at most once every 6 gt (its cycle: brief) | 11 b for one bedrock, rows of 2 to 15 side by side (6N + 3 b), armed every 6 gt | at the 6 gt ceiling; blocks near the ~9-10 b floor |

## Ceilings, from the code

- **Random ticks.** `ServerLevel.tickChunk` picks `randomTickSpeed` (3) random blocks in every
  16x16x16 section each tick: a given block gets 3/4096 random ticks per gt = **52.7 per hour**.
  Every crop ceiling below is that times the crop's chance.
- **C1 Hopper.** `HopperBlockEntity.MOVE_ITEM_SPEED = 8`: one item per 8 gt = **2.5 items/s** per
  hopper; a line is no faster than its slowest hopper.
- **C2 Furnace.** `AbstractFurnaceBlockEntity.BURN_TIME_STANDARD = 200` (the cook time when the
  recipe gives none): one item per 10 s = **6/min per furnace**; coal burns 1600 gt = 8 items. A
  chest is 27 x 64 = 1728 items, so N furnaces need at least 1728 / (6N) min. A hopper line moves
  150 items/min, enough for 25 furnaces, so feeding is never the limit; spreading the load is.
- **C3 Sugar cane.** `SugarCaneBlock.randomTick`: only the top block grows, and only when its age
  reaches 15: 16 random ticks per new block = **3.30 cane/h per plant**. Every cane needs water
  beside its soil (`SugarCaneBlock.canSurvive`): rows cane, water, cane put plants on 2/3 of the
  floor (2.2 cane/h per floor cell, before the harvester's blocks).
- **C4 Stems.** `StemBlock.randomTick`: grows on `nextInt((int)(25 / f) + 1) == 0`, f from
  `CropBlock.getGrowthSpeed` (1 + 3 for wet soil under it + 3/4 for each wet neighbour soil, halved
  when the same crop sits on both axes or a diagonal). A grown stem picks ONE random side per
  success; the fruit grows only if that side is air on dirt or farmland. Lone stem, 4 free sides,
  wet diagonals: f = 7, 1 in 4, every success grows = 52.7 / 4 = **13.2 fruits/h**, the per-stem
  ceiling. f = 10 is out of reach: melon, pumpkin and piston heads are not in `maintains_farmland`,
  so a fruit cell turns to dirt (`FarmlandBlock.canSurvive` -> `turnToBaseBlock`; w86's
  correction, designs/briefs/farm-melon-pumpkin.md). A stem row between two fruit rows: f = 5.5,
  1 in 5, half the sides free = 5.3 fruits/h per stem. A melon drops 3-7 slices (5 on average).
  64 stems at the ceiling: 844 fruits = **4218 slices/h**; per block, at most 16.5 slices/h on
  soil alone and ~6.6 with the least machine (w86's doc).
- **C5 Crops.** Same f; 7 ages, so at best 21 random ticks per harvest = 2.5 harvests/h per plant
  (a field of one crop on both axes halves f: 1 in 6).
- **C6 Bamboo, kelp.** `BambooStalkBlock.randomTick`: `nextInt(3) == 0` on the top = 17.6/h per
  stalk, up to 16 high, no water needed. `KelpBlock` grows on 0.14 per random tick = 7.4/h.
- **C7 Dropper.** `DispenserBlock` schedules its fire 4 gt after a rising edge.
- **C8 Bubble column.** `Entity.handleOnInsideBubbleColumn`: an upward column adds 0.06 b/gt per
  gt up to 0.7 b/gt (down: 0.3 b/gt).
- **C9 Hopper minecart.** `MinecartHopper.tick` -> `tryConsumeItems` -> `HopperBlockEntity.suckInItems`
  every tick with no cooldown: **1 item/gt = 20 items/s** from the container above it.
- **C10 Composter.** `ComposterBlock`: the first item always adds a layer, later ones by their
  chance (seeds 0.3, cane 0.5, wheat 0.65: designs/circuits/tracks/composter/NOTES.md); level 7
  waits a 20 gt scheduled tick. Seeds: 1 + 6/0.3 = 21 items at 8 gt + 20 gt ≈ 196 gt per bone meal
  = ~370/h, eating ~2 items/s: more than any farm on the site makes.
- **C11 Brewing stand.** `BrewingStandBlockEntity`: a brew takes 400 gt and does 3 bottles.
  `SLOTS_FOR_DOWN = {0,1,2,3}` and `canTakeItemThroughFace` lets a hopper under the stand take
  any bottle at any time, so a plain hopper below steals water bottles before they brew: the
  obvious brewer needs that hopper locked for the 400 gt.

## What to work next (top of the table, gaps first)

Already in flight: 2 (super smelter, sorter), 3 (sugar cane, clk-rt), 4 (melon/pumpkin, w86), 24
(digit display, clk-obs). Next, in order: **6** automatic door at the obvious design's size, **15**
cart loader at the 20 items/s a hopper minecart allows, **19** brewer toward 4.5 potions/min for a
2-step potion, **11** a full fuel farm feeding the super smelter's fuel line, **1** a complete
sorting system page.
