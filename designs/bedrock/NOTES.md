# Bedrock breaking (track bedrock, explore/bedrock)

Started Nov 6 2026 from vjeux's ask: a bedrock breaker that is reliable and easy in survival
(the known headless-piston + lag method is brittle). Everything below is read from the 26.3
source (`mctas src show <Class>`) and checked in the simulator and/or on the unmodified official
server (`mctas bedrock vanilla`, a network bot doing real break/place packets, the server frozen
and stepped one tick at a time).

Results in one line each:

* **Floor bedrock (Nether floor, Overworld floor): the portal method.** No timing, no lag, no
  piston. Each portal trip turns a 1x4 line of bedrock into obsidian; repeated trips drill to the
  void. Verified on the official server, including the lowest layer.
* **Nether roof (and anything else): the piston swap, armed by a machine.** One key press, one
  armed window every 8 ticks, free retries, no lag (section 3); verified on the official server.


## 1. The portal method (floors)

### Why it works (`PortalForcer.createPortal`, 26.3 lines 52-156)

When an entity goes through a nether portal and no portal exists near its exit
(`findClosestPortalPosition`: 16 blocks in the Nether, 128 in the Overworld, square in x/z, any
y), the game builds one:

* It scans a 33x33 column spiral around the exit point (`approximateExitPos` =
  (x * scale, entity y UNSCALED, z * scale), `NetherPortalBlock.getPortalDestination` 140) for
  sites. A site at column C needs, for the frame row and the rows on each side of it
  (`canHostFrame` 163-182): the 4 cells at the frame's bottom (y-1) `isSolid()`, and the 4x4 cells
  above them replaceable air (no fluid).
* A FULL site (all three rows valid) closer than any other full site wins; a partial site (frame
  row only) is used only when no full site exists anywhere in the spiral (90-105). Distance is 3D.
* It then writes the frame with `setBlockAndUpdate(OBSIDIAN)` (137-144) without looking at what it
  overwrites. The sides and top are air by the site rule, but **the bottom row only had to be
  solid: bedrock passes `isSolid()` and becomes obsidian.** Only the middle row's 4 bottom cells
  are written; the side rows are checked, never changed.

So one trip = one 1x4 line of bedrock turned into minable obsidian. Not a 4x4.

### Recipe: a hole in the Nether floor

The Nether floor's bedrock is y 0 (solid) to y 4 (20%); probabilities 100/80/60/40/20% for
y 0..4.

1. In the Nether, at the floor: pick a 4x3 patch (4 along the portal axis, 3 across) whose middle
   row holds the bedrock to remove at layer L (start at L = 4). Make the patch's 12 bottom cells
   (y = L) solid (fill gaps with netherrack; bedrock and obsidian count) and the 4x4x3 box above
   them (y = L+1 .. L+4) air. No other nether portal within 16 blocks.
2. In the Overworld, at 8x the patch's frame column (Nether x, z times 8; put the overworld portal's
   interior at x mod 8 = 3..4 so both columns of it map to the same Nether block), dig down and
   build a portal whose interior bottom is at y = L+1, along the same axis as the patch.
3. Walk through. The game finds no portal to link to and builds the exit on your patch (it is the
   closest full site: distance ~0). The middle row's bottom (y = L) is now obsidian; you arrive in
   the new portal.
4. Mine the new frame above the bottom row (10 obsidian; the portal pops). **Leave the 4 converted
   obsidian in place for now:** they keep counting as solid floor for the next trip.
5. Next row: block the air space of the rows you do not want (one cobblestone in a side row's air
   is enough to make that site invalid), or move the overworld portal 8 blocks across (= 1 Nether
   block). Repeat. When a layer's rows are done, mine the obsidian.
6. Deeper: converting layer L needs the 4x3 box at L+1..L+4 to be air, so each layer needs the
   layer above cleared one row wider on each side: a stepped pit. For one 1x4 hole through all five
   layers the rows to clear are 9 at y 4, 7 at y 3, 5 at y 2, 3 at y 1, 1 at y 0; with the
   floor's random bedrock that is about 20 trips (expected value of rows holding any bedrock:
   5.3 + 6.1 + 4.9 + 3.0 + 1).
7. Getting back to the Overworld for the next trip: the generated portal must be gone (mined)
   before each trip or the next one links to it. Keep a permanent return portal at least 17 blocks
   away (Chebyshev) from the patch; its overworld twin is then >= 136 blocks from the drill portal
   (the 16 / 128 search radii match exactly at 8x, so there is no closer placement). A walk or an
   ice road between them.

Can not reach the **Nether roof**: `maxPlaceableY` = min(maxY, minY + logicalHeight - 1) = 127
(line 59) and a site needs y + 4 <= 127 (77): the frame bottom is at most y 122, and the roof's
bedrock starts at y 123. The fallback platform (no site found, 107-135) is clamped to
y 70..118.

### The Overworld floor

Same mechanics, nether -> overworld trips. The catch is the site choice: the exit point's y is the
entity's y in the Nether (>= ~1), about 60 blocks above the overworld floor's top layer (y -60),
and the closest FULL site in the 33x33 column window wins. Any cave or surface spot with a 4x3
floor and 4 air above that is closer than ~60 blocks (3D) to (8x, y_nether, 8z) beats the floor
site. It works in a void test world; in a real world the 33x33 window above the drill site has to
be checked for such spots (caves between y -59 and ~+60; the surface at y 64 is just far enough
from a y 1 exit). Targeting precision is also coarser this way (1 Nether block = 8 Overworld
blocks): pick the row with blockers (step 5) rather than by position.

### Evidence (`designs/bedrock/portal/*.json`, `mctas bedrock vanilla <file>`)

Every `execute if ...` in a scenario is an assertion: a failed test aborts the run.

* `portal_overworld.json`: a 4x3 bedrock patch at y -64 in the void overworld; an item through a
  Nether portal. The middle row's bedrock (-1..2, -64, 0) became obsidian, the side rows stayed
  bedrock, the item arrived. Result `broken`.
* `portal_drill.json` (the full cycle): two stacked plates (y -63 over y -64, 4x5). Trips A, B, C
  convert top-layer rows 0, 1, -1 (cobblestone blockers leave one full site each time; converted
  obsidian kept as support, only the frame above it mined); the top-layer obsidian is mined; trip
  D converts bottom-layer row 0 at y -64; mining it opens a hole to the void. 30 assertions,
  result `broken`.
* `portal_nether_survival.json`: the real Nether floor near (0, 0) with survival-style edits
  (cells of y 1..14 filled with netherrack, everything above y 4 mined): an item through an
  overworld portal at y 5 gets its exit portal at (0, 5, 0) with obsidian at (-1..2, 4, 0), the
  floor's top layer. (Its harness line says `fail` because its declared target is a dummy; the
  assertions are the test.)
* `portal_nether_flat.json`, `portal_nether_natural_run1.json` (the audit's runs): with the floor
  flattened the y 0 layer converts; on untouched natural terrain the exit lands at the fallback
  height (~y 70), i.e. without a player-built site nothing happens to the floor.

## 2. The piston swap (anywhere, including the roof)
### The bug (`PistonBaseBlock.triggerEvent` 139-213)

* A power change only QUEUES a block event (`checkIfExtend` 93-114): extended + unpowered ->
  retract event (1, or 2 when the block two ahead is still being pushed).
* When the event runs it only checks that the cell still holds the same block (piston vs sticky
  piston, `ServerLevel.runBlockEvents` 1267-1292). It then reads the facing AGAIN from the current
  state and, for a plain piston, `level.removeBlock(pos.relative(direction))` (205) without
  checking that this is its head. Bedrock is removed.
* If the piston is powered when the retract event runs, it is only set `extended=true` (144-146,
  flags 2, no updates): a piston that was swapped in becomes HEADLESS. A headless piston facing
  bedrock removes it whenever it is unpowered later (a normal retract).
* Events for cells that are not block-ticking are kept (`blockEventsToReschedule`, 1287) until the
  chunk ticks again; but a player's own chunk gets ticket level 31 - simulation distance (<= 31,
  `DistanceManager.getPlayerTicketLevel` 132) and its neighbours at most one more (block ticking
  is <= 32, `ChunkLevel`), so any piston within the player's reach is in a block-ticking chunk:
  the window cannot be parked.

### Verified

| scenario (`designs/bedrock/scenarios/`) | what the player does in ONE tick | simulator (`bedrock scan`) | official server (`bedrock vanilla`) |
|---|---|---|---|
| `torch-3-actions` | break torch, break piston, place piston facing down | 11/11 windows broken | 2/2 broken; the same clicks split over two ticks: 3/3 fail |
| `torch-headless` | the same + put the torch back | 4/4 headless, released later -> broken | 2/2 headless: `piston[extended=true,facing=down]`, no head; breaking the torch later removes the bedrock |
| `torch-chord-only` | break piston + place piston (no retract queued) | 0/11 | |
| `tnt-place` | TNT breaks torch and piston, the player only places | 0/11 (the blast shuffles its blocks: the torch is often broken last) | |

### Why there is no lag-free version

The swap has to happen between the moment the retract event is queued and the next block-event
phase. Nothing in the game puts a piston into a cell in that window (pistons are placed by
players, by landings in the block-entity phase of a push that claimed the cell when it started, or
by 0-tick finalTicks inside the block-event phase), and an extended piston cannot leave its cell
inside the block-event phase except by a player, an explosion or another headless piston. So a
player has to land the swap inside one tick; server lag widens that tick, which is why the lag
method works. **But the window can be made to come back on a beat, and the player's part can be
one key press: section 3.**

## 3. The self-arming machine (the roof, lag-free)

A retract event queued in the scheduled-tick or the block-event phase is consumed by the SAME
tick's block-event phase, so it is gone before the server handles the player's packets (the
connection is ticked after every level, `MinecraftServer.tickChildren` 1075-1147). Two phases run
AFTER the block events: entities (7) and block entities (8). An event queued there is still
pending when the player's next packets are handled. The machine uses that:

* **A** is the piston next to the target bedrock, facing AWAY from it (so it can extend), powered
  only by **quasi-connectivity** (a redstone block beside the cell above A). A QC-powered piston
  does not re-check when its source goes away (`getNeighborSignal` 116-136): it stays extended,
  silently unpowered, until something updates it.
* **SP**, a sticky piston, carries that redstone block in and out of the quasi cell. The pull
  empties the cell in the block-event phase and never updates A.
* **U**, a second piston, pushes a block into a cell BESIDE A. A moved block lands in the
  block-entity phase (`PistonMovingBlockEntity.tick` 294-327), and the landing calls
  `setBlock(..., 67)` + `neighborChanged`: **that update reaches A after the tick's block events**,
  so the retract event it queues on A is still pending in the player's window. ARMED.
* Both pistons push in the same tick, U's event first, so in that one block-entity phase U's block
  lands first (A armed while the power is away) and the carrier lands right after (power back).
  The pending event then finds A powered at the next block-event phase and only re-sets
  `extended` (144-146): a no-op, A keeps its head, and the machine repeats for ever.
* The player's part is **one key press**: bind Attack and Use to the same key (`KeyMapping.MAP`
  holds a list per key, and `Minecraft.handleKeybinds` 1966-1972 runs both `consumeClick` loops in
  the same client tick), hold a pickaxe that instamines a piston and the piston in the offhand.
  The press breaks A and places B facing the bedrock in one window. Pistons are
  `mineable/pickaxe`, hardness 1.5, so instamine needs tool speed >= 45: **Efficiency V diamond or
  netherite + Haste II** (47.6 / 49.0). Without Haste II the break takes more than one tick and the
  pair cannot share a window.
* **A miss costs nothing**: B is left unextended facing the bedrock. Break it, put A back facing
  away, and the machine's next landing extends it again.
* When the press lands in an armed window, the pending event fires on B. If B is powered (the
  carrier has landed), it becomes a **headless piston facing the bedrock** - the visible win, an
  arm-less piston. Cut its power (pull the carrier, then any update) and the bedrock is removed.

Measured (`designs/bedrock/scenarios/qc-landing-roof-clock.json`): one armed window every 8 game
ticks (0.4 s), for ever, A intact - `bedrock watch` prints
`xxxxxxxxxxxxAxxxxxxxAxxxxxxxAxxxxxxxAxxxxxxxAxxxx`. `bedrock scan`: the press wins in 5/5 armed
windows and in none of the others. On the **unmodified official server** with the bot pressing in
one window: window 12 and window 20 (armed) both left `piston[extended=true,facing=up]` with no
head and the release removed the bedrock; window 13 (not armed) failed.

So the odds are 1 in 8 per press, with free retries - a few seconds of pressing in rhythm, no lag,
no macro, and the headless piston tells you the moment it worked.

**Still to build:** the scenario drives SP and U from a script (the exact beat below). A survival
build wires both to one clock: pull the carrier at t, push BOTH at t+3 (U's power arriving first,
e.g. one repeater closer), pull U's block at t+7, pull the carrier at t+8 = t of the next cycle.
Never retract U while its block is still moving: the spit finalises it in the block-event phase,
and that update, with the power away, retracts A instead of arming it.

## 4. Other ways to remove bedrock (audit)

Every play-time write that can change an existing block was checked (trees, fungi, mushrooms,
plants, dripstone, moss, sculk, fluids, falling blocks, endermen, dispensers, buckets, snow and
ice, frost walker, golems, lightning, fire, explosions, the wither and the dragon, End features,
update skipping at the chained-update limit, head removal paths): see [AUDIT.md](AUDIT.md). Only
`PortalForcer.createPortal` (section 1) and the piston retract (section 2) can act on bedrock in
survival; the dragon-respawn spike clearing and the End podium/gateways write unconditionally but
only in the End at fixed places.

## Tools

* `mctas bedrock scan <scenario> [--from A --to B]`: the scenario's chord in every inter-tick
  window (fresh sandbox each), outcome per window and the fraction that works.
* `mctas bedrock watch <scenario> [--to N] [--verbose] [--cells x,y,z]`: one run without the
  chord; per window, what the swap cell holds (armed / moving / open / wasted), from the bridge's
  new `events` op (pending block events). `--verbose` prints every pending event and the blocks of
  the `--cells` per window: that is how the machine of section 3 was phased.
* `mctas bedrock vanilla <scenario> [--windows a,b] [--split]`: the same on the official server,
  the chord done by a network bot (break: `START_DESTROY_BLOCK`; place: `USE_ITEM_ON` with the
  offhand holding the block); `cmd` actions run vanilla commands in order (scenario setup and
  `execute if` assertions). `MCTAS_BEDROCK_DEBUG=1` dumps the chord's cells after the chord.

## Next

* Wire the section 3 machine from a clock (the beat is specified there) and verify the wired
  build end to end, then measure how a human does against the 1-in-8 window.
* Narrow the beat: two updater pistons out of phase would arm more often than once per 8 ticks.
* The Overworld floor: a reliable recipe for clearing (or proving absent) the closer sites.
