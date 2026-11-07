# Breaking bedrock in survival

bedrock track, 2026-11-07. Gate G1 (docs/process.md). Asked by vjeux on 2026-11-06: "There's a
way to break bedrock on java by using headless piston and lag the game. But the setup is pretty
brittle. Can you find a bedrock breaking setup that's reliable and easy to do in survival." The
work, the evidence and every tool are in `designs/bedrock/NOTES.md`; this brief is what the site's
kind and judge rest on.

## Use

A survival player removes bedrock where no tool can: a hole in the Nether roof's top layer (to get
on top of it or back down through it, or to clear a strip of it), the Nether floor or the
Overworld floor (to reach the void).

## The number

1. **Blocks placed** (the bedrock and the roof are not counted).
2. **Armed every**: the longest wait, in game ticks, between two moments where a press works.
   A press at a random moment wins 1 time in that many.

Reported: how many bedrock one build removes (side by side), the one-press repair after a miss,
works in every orientation and position.

## Mechanics, from the game code (26.3, `mctas src show <Class>`)

**The bug: a queued retract runs on whatever piston is there when it runs.**
- `PistonBaseBlock.triggerEvent` reads `FACING` from the block's state when the event RUNS, not
  when it was queued. For a retract (`b0` 1) of a non-sticky piston it ends with
  `level.removeBlock(pos.relative(direction), false)`: no hardness check.
- `ServerLevel.doBlockEvent` only checks that the block at the position is still the event's
  block (`blockState.is(event.block())`): any piston will do.
- So: a piston facing sideways has a retract queued; before it runs, the player breaks it and
  places a piston facing DOWN in the same cell. The retract runs on the new piston and removes the
  block under it: the bedrock.

**When is a retract still queued in the player's window?**
- Block events run in the tick's block-event phase and are drained until the queue is empty: a
  retract queued during the scheduled ticks or the block events runs in the same tick, before the
  player can act.
- A retract queued in the **block-entity phase** (after the block events) stays queued until the
  next tick's block events. The player's packets are handled between ticks: that is the window.
- A piston's own head landing is a block-entity event: `PistonMovingBlockEntity.tick` reaches
  progress 1 and `finalTick` sets the head and updates its neighbours, the base included. If the
  base finds itself extended and unpowered then, `checkIfExtend` queues the retract in the
  block-entity phase: **armed** for exactly one window.
- So the piston must lose its power silently (no update) between extending and its head landing.

**Silent power: an observer and quasi-connectivity.**
- `PistonBaseBlock.getNeighborSignal` also counts power into the cell above the piston, from that
  cell's neighbours (quasi-connectivity): a powered block X diagonally above the piston powers it.
- `ObserverBlock.tick` switches the observer with flags 2 (no neighbour updates) and then
  `updateNeighborsInFront`: X (the block its output faces) and X's neighbours. The piston is not
  one of them: its power comes and goes with no update.
- The observer's pulse lasts 2 ticks (`ObserverBlock.tick` schedules its own turn-off 2 ticks
  later), exactly the piston's 2 ticks of motion: the piston extends inside the pulse, the pulse
  ends silently, the head lands: armed.

**What makes the piston extend inside the pulse (an update while powered).**
- Redstone dust that changes power updates its neighbours and their neighbours
  (`RedstoneWireBlock.updatePowerStrength`): a dust diagonal to the piston (it never powers it,
  it does not point at it) nudges it.
- The clock's redstone block lands in the block-event phase (a spit:
  `PistonBaseBlock.triggerEvent` finishes a block still moving with `finalTick`) and relights the
  dust under it: the nudge, inside the observer's pulse.
- After the clock's other move, the piston's own base, retracting from the previous arm, lands
  inside the pulse (`PistonMovingBlockEntity.finalTick` again): it updates itself and extends.
- Two pistons side by side update each other when either changes state (`Level.setBlock` with
  flag 1): one nudged piston starts the whole row in the same phase.

**The press a player can make.**
- A piston is placed facing away from the look: `PistonBaseBlock.getStateForPlacement` uses
  `context.getNearestLookingDirection().getOpposite()`. Looking up more than 45 degrees places it
  facing down; looking mostly east places it facing west (the repair).
- An instant break: `BlockState.getDestroyProgress` is speed / hardness / 30 per tick for a
  block the tool may break; a piston (hardness 1.5) breaks in one tick at speed 45.
  `Player.getDestroySpeed`: a netherite pickaxe 9 + Efficiency V (`MINING_EFFICIENCY`, 5² + 1)
  = 35; Haste II multiplies by 1.4 (49: enough); Haste I by 1.2 (42: not); off the ground ÷5.
  Haste II in survival comes from a beacon with a full 4-level pyramid.
- One key, both actions: the 26.3 client keeps a list of mappings per key (`KeyMapping.MAP`), so
  a key bound to Attack and Use clicks both. `Minecraft.handleKeybinds` consumes the attack clicks
  before the use clicks in the same tick; `startUseItem` uses the block hit as it was (no air
  check), so the offhand piston goes into the cell just broken (air is replaceable:
  `BlockPlaceContext.replaceClicked`).
- Reach and eye: `Player.blockInteractionRange` 4.5; a crawling player's eye is 0.4 above its
  feet. A trapdoor closed over a one-block gap makes the player crawl next to the piston, low
  enough to look up at its side.

**Why one swap piston arms at most once every 6 ticks** (the ceiling of the second number).
One try is one full cycle of the piston: it extends (2 ticks of motion), is armed for one window,
its retract runs, its base moves back (2 ticks), and only then can it extend again: 6 ticks.
A queued event always runs in the next tick, so the armed window is one tick long.

**The portal method (floors), a different bug.** `PortalForcer.createPortal` writes the obsidian
frame (lines 137-144) over whatever is there; `canHostFrame` only asks the bottom row to be
solid. A portal trip that creates its exit portal on the floor turns bedrock into obsidian, one
1x4 line per trip, down to the void. It cannot reach the Nether roof (`maxPlaceableY` 127, a frame
bottom at most y 122). No redstone: it is a recipe, not a build (NOTES section 1).

## What a player does

Stands on the Nether roof (the machine is built ON TOP of the roof, nothing is dug), builds the
machine, places the clock's lamp last (the clock starts), lies under the trapdoor beside the swap
piston, looks up at its side, and presses one key in rhythm with the machine until the bedrock is
gone. After a miss (the piston now faces down into the bedrock): one press looking the other way
puts the sideways piston back.

## Trade-offs

- **Lag against a machine.** The known headless-piston method needs the server to lag at the right
  moment: brittle. The machine needs no lag and no TNT; the cost is the tick-precise press.
- **Precision against odds.** A random press wins 1 in 6; in rhythm with the visible cue (the
  swap piston's arm finishing its extension) a player does better. Nothing faster was found
  (below).
- **One build per bedrock against a row.** 11 blocks clear one bedrock; a row clears N side by
  side for 6N + 3 blocks (each extra bedrock: its piston, trapdoor, stone, observer and one dust
  on a block). The swap piston and its trapdoor must sit on each target, so a fresh 11-block build
  per bedrock costs more placements than a row.
- **Haste II.** The one-key press needs an instant break: a full beacon. Without it the break takes
  2 ticks and the two actions can no longer land in one window.

## Five layouts

| # | layout | blocks | armed every | % of the ceiling (6 gt) | constraints |
|---|---|---|---|---|---|
| 1 | **Obvious: a piston on the bedrock held extended by a torch; in ONE tick the player breaks the torch, breaks the piston and places one facing down** (`scenarios/torch-3-actions.json`) | 2 | only while the player lands 3 clicks in one 50 ms tick: measured 11/11 windows on the simulator, 2/2 on the official server, and the same clicks split over two ticks 3/3 fail | n/a | needs a macro or server lag (the known lag method is this with a lag spike widening the tick): brittle |
| 2 | A clock powering the swap piston directly | 4-6 (est.) | never (from the mechanism: the retract is queued in the scheduled or block-event phase and runs in the same tick) | 0% | nothing to time |
| 3 | **The first working machine**: observer + quasi-connectivity, two updater pistons, a 12 gt repeater clock (`machines/wired2.txt`) | **45, measured** | **6 gt, measured** | 100% | big |
| 4 | **The 11-block machine**: the clock's own redstone block does the timing (`machine-roof-11b.txt`) | **11, measured** | **6 gt, measured** | 100% | one bedrock |
| 5 | **Rows**: one stone and observer per piston, the clock's block sliding behind them (`bedrock row N`) | **6N + 3 (14 for 2 after the minimizer), measured** | **6 gt, measured** | 100% | N <= 15 (the clock line's signal) |

Also measured and set aside: three pistons around one stone (15 blocks for 3 bedrock, the arms of
a plus around a bedrock that stays) and six per clock (24 blocks, two such groups): vjeux, "the 6
one is useless, this pattern doesn't make sense to break in a real game situation".

## Decision

Layout 4 is the pick for one bedrock; layout 5 for several side by side. Layout 1 is the bar a
player has today, and it is not a machine a player can use without lag; layout 3 was the first
design that arms by itself: the search (`bedrock graft`, every archived 12 gt clock in every
orientation, then `bedrock minimize` over the door search's edits, then a hand fold) took it from
45 to 11 blocks at the same 6 gt. Arming faster needs the swap piston to stay extended between
tries (power back before its queued retract runs, so it does nothing, then a landing beside it
every couple of ticks); no archived clock of period 4 to 11 grafted onto the machine arms every 5
ticks or better (195,198 tried).

## Must-haves

- Every click a real player can make: the bot's look reaches the clicked face within 4.5 blocks
  from a crawling eye, and the placed piston faces what that look gives (`bedrock vanilla`'s
  legit stances).
- The survival break: an instant break with Haste II and an Efficiency V pickaxe, on the ground.
- Works in every orientation and position (the swap piston's update order must not matter).
- A miss is repaired with one press, and the machine arms again by itself.
- Built on top of the roof: nothing below the top layer is dug or touched.
- A used piston (its bedrock gone, the down-facing piston left on the hole) does not stop the
  others of a row.
- Survival materials: pistons, sticky pistons, observers, redstone, a redstone block, a lamp,
  trapdoors, stone.

## Not goals

- The layers under the roof's top layer: the press cannot look up at a piston sitting in a hole.
- Better than 1 in 6 per press.
- Telling the player when to press (the machine is visible and periodic).

## The judge (`circuit: bedrock-breaker N`, crates/circuit/src/studies/bedrock.rs)

- Ports: `port bedrock[.k]` (a target), `port swap[.k]` (its swap piston, on the target),
  `port press[.k]` (the cell the player lies in, under a closed trapdoor), `port start` (the
  clock's kick, placed last).
- Each build (positions x rotations): the machine is placed and kicked; for each swap piston, the
  press (break it, place a piston facing down, 3 ticks) is tried in every window of two clock
  periods of the steady state, each try on a fresh build; it must work in at least one window of
  every 6, and the bedrock must stay when nobody presses. Then a miss (a press in a window that
  does not work) and the repair (the original piston put back): a later press must work.
- Metrics: armed every (gt), bedrock (N), the miss repair, blocks.
- The redsim twin is the whole judge on redsim (`designs/bedrock` `bedrock judge`).
- The official server's checks (the survival bot on the real Nether roof) stay in NOTES: they are
  too slow for every build of the archive.

## The first design, measured (G2)

`designs/bedrock/machines/wired2.txt` (45 blocks): armed every 6 gt at 256 of 256 variants on
redsim; on the official server's real Nether roof, 12/12 armed presses removed the bedrock and
12/12 others did nothing over 3 fresh worlds.

## Ceiling

- **Armed every: 6 gt** (one swap piston's cycle, above). Every layout from 3 on reaches it.
- **Blocks for one bedrock: about 9-10, counted by parts.** The swap piston and its trapdoor (2),
  a powered block and an observer (2), a clock that moves something every 6 ticks (3-4 with its
  kick), one or two dust to nudge the piston. The 11-block machine is at 82-91% of that floor.
