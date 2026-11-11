# Mechanics (read from the 26.3 source)

Everything here was read in the decompiled game (`mctas src show <Class>`; class names are 26.3's,
unobfuscated, and line numbers are that listing's) and checked in the simulator. Every effect a
door relies on is proven by a gadget (a small design plus a drive script with `expect`s,
[section 10](#10-gadget-catalog)), and the whole gadget library behaves identically in the
unmodified official server ([section 11](#11-vanilla-confirmation)). Ticks are game ticks (gt,
1/20 s); "k" or "t" = ticks after the lever flip (the flip happens between ticks, so the first
tick after it is k1). "1-tick" in the redstone community usually means one REDSTONE tick (2 gt, an
observer pulse); this page uses game ticks throughout.

Sections: 1. one tick, 2. pistons, 3. redstone dust, 4. other blocks, 5. timing of every
component, 6. what happens in 0 gt, 7. odd ticks, 8. techniques the doors use, 9. floors for the
fastest 3x3 door, 10. gadget catalog, 11. vanilla confirmation, 12. redsim, the exact model
(where each rule lives in `crates/redsim`, with source lines and evidence). (Sections 5-9 and 11
are the tick-study track's timing study.)

## 1. One tick (`ServerLevel.tick`, 358-460)

| order | phase | source | what runs there |
|---|---|---|---|
| between ticks | player actions | `LeverBlock.pull`, `ButtonBlock.press` | lever/button state + neighbor updates at once: dust re-powers, pistons QUEUE events, observers/repeaters/torches SCHEDULE ticks, lamps light, bulbs toggle |
| 1 | `handlingTick = true` | 360 | (used by `PistonBaseBlock.checkIfExtend`, section 2) |
| 2 | game time + 1 | 387 `tickTime()` | (world border, weather and the sleep check run before) |
| 3 | scheduled block ticks | 394 `blockTicks.tick` | repeaters, comparators, observers, torches, buttons, plates, lamps (off), leaves... by (time, priority, insertion) |
| 4 | fluid ticks | 396 | water 5 gt, lava 30 gt (then raids, and chunks: random ticks, spawning) |
| 5 | block events | 409 `runBlockEvents` (1267-1292) | pistons (`triggerEvent`), note blocks, bells/chests; FIFO `removeFirst` until EMPTY, events added meanwhile run in the same phase; the queue is an `ObjectLinkedOpenHashSet` (215): an identical pending event is dropped |
| 6 | `handlingTick = false` | 412 | |
| 7 | entities | 431 | |
| 8 | block entities | 459 `tickBlockEntities` | `PistonMovingBlockEntity.tick` (progress, landing), sculk sensors, daylight detectors (every 20 gt), hoppers |

Neighbor updates go out in the fixed order `WEST, EAST, DOWN, UP, NORTH, SOUTH`
(`NeighborUpdater.UPDATE_ORDER`, through `CollectingNeighborUpdater`), so pistons powered by one
change queue their events in that order (gadget `event-order`). Dust updates its neighbors and
their neighbors in `HashSet` order (position dependent; `DefaultRedstoneWireEvaluator`).

**Orientation-based update order:** 26.3 ships `ExperimentalRedstoneWireEvaluator` and passes an
`Orientation` through `CollectingNeighborUpdater` (149-155) and `PistonBaseBlock.moveBlocks`
(336), but `ExperimentalRedstoneUtils.initialOrientation` (9-22) returns null and
`RedstoneWireBlock.updatePowerStrength` (271-275) uses the default evaluator unless the world has
the `redstone_experiments` feature flag. Default worlds (the judge's, the official server's) keep
the fixed W,E,D,U,N,S order and the position-dependent dust order.

## 2. Pistons (`PistonBaseBlock`, `PistonStructureResolver`, `PistonMovingBlockEntity`)

* **Power** (`getNeighborSignal`): any neighbor except the front gives signal into the piston, OR
  any neighbor of the cell *above* the piston (except the piston itself) gives signal into that
  cell: **quasi-connectivity**. The piston only re-checks when it gets an update, so a QC-powered
  piston can sit unextended until something updates it (BUD).
* A power change queues a **block event**; the event re-checks power when it runs (so a pulse that
  ends before the block-event phase is cancelled).
* **Extend:** the resolver pushes the line in front (max **12** blocks; slime/honey drag
  neighbors; `POPPED` blocks like dust/torches are destroyed and drop items). Moved blocks become
  `moving_piston` with a block entity at progress 0; the block-entity phase of that tick brings it
  to 0.5, the next tick to 1.0, and the tick after places the real block. A piston action started
  at tick t is finished in the block-entity phase of tick t+2.
* **Retract:** the base itself becomes a `moving_piston` for those 2 ticks (so a retracting piston
  cannot be pushed or pulled until it is done). A sticky piston pulls only the block in front of
  its head (plus whatever slime/honey drag along), and only with event type 1.
* **Short pulses (spitting):** if power drops while the block 2 in front is still a
  `moving_piston` being pushed by this piston (progress < 0.5, same tick, or during the tick
  phase), the retract event is type 2: the moving block is finished in place and **not pulled**.
* **Immovable:** extended pistons, obsidian/bedrock (`getDestroySpeed == -1`), blocks with block
  entities, `PushReaction.BLOCK` (piston heads, moving pistons). `PUSH_ONLY` blocks (glazed
  terracotta) can be pushed but not pulled and are never dragged by slime.
* **Slime/honey:** a moved slime or honey block drags every adjacent movable block, except that
  slime and honey never stick to each other. Non-sticky moved blocks drag nothing: moving a piston
  that sits under a slime block leaves the slime behind. Blocks with push reaction DESTROY (dust,
  repeaters, levers, torches) next to a moving slime block are left in place, not dragged or
  broken (only a DESTROY block in the movement path breaks); their floors are normal blocks and do
  get dragged (glazed terracotta never is). A slime-dragged neighbor that is immovable (extended
  piston, piston head) is skipped too.
* A piston that lands somewhere (moved by another piston) runs `onPlace` and extends at once if it
  is powered there (gadget `landing-fire`).
* **Piston heads forward updates:** `PistonHeadBlock.neighborChanged` passes every neighbor update
  to its base, so an extended piston also re-checks when a cell next to its head changes.
* **Sticky retract during a push finalizes, never pulls** (seen in rb9): if a sticky piston
  retracts while the block two in front is still a moving piston it pushed (any progress, as long
  as it has not landed), that block is placed at its destination at once and the piston retracts
  alone.

The rules that set the timing, with their source lines:

| rule | source |
|---|---|
| A power change only QUEUES an event (0 extend, 1 retract-and-pull, 2 retract-and-drop); the event re-checks power when it runs: an extend whose power is gone is dropped, a retract whose power is back just re-sets `extended` | `checkIfExtend` 93-114, `triggerEvent` 143-151 |
| An extend is only queued if the push resolves at that moment (no retry later: gadget `no-retry`) | 96-98 |
| Moved blocks become `moving_piston` at the destination at once; their old cells are air at once (head cell: the moving head) | `moveBlocks` 299-326 |
| A moving block's entity goes 0 -> 0.5 -> 1.0 in the block-entity phases of t and t+1 and is placed in the block-entity phase of t+2 | `PistonMovingBlockEntity.tick` 294-327 |
| Retract while extending: the head's moving entity is finalTick'd (to air: it is the source piston) | `triggerEvent` 164-166, `finalTick` 270-271 |
| Sticky only: if the block two ahead is a moving block this piston pushed (extending, same direction), it is finalTick'd: PLACED NOW, not pulled. Normal pistons only remove their head (the block keeps moving) | 180-190, 204-205 |
| Retract type 2 (drop) when the block two ahead is moving and (progress < 0.5 or this tick's entity tick already ran or the unpower happens in the scheduled/block-event phases); else type 1 (pull) | 101-110 |
| Type 1 with a plain block two ahead pulls it (also when the front cell was EMPTY: reach-through) | 192-202, `PistonStructureResolver` (retract starts at pos+2) |
| A pull is never finalized early: pulled and dragged blocks always land at t+2 | (finalTick needs `isExtending`) |
| The retracting base is a `moving_piston` (immovable, cannot act) until t+2; it lands with `setBlock(..,67)` + `neighborChanged`, so a powered piston re-extends at t+3 at the earliest | 168-179, `PistonMovingBlockEntity.tick` 313-314 |
| A piston placed by `finalTick` or a landing runs `onPlace` and queues an extend if powered (in the same phase when placed by a 0-tick) | `onPlace` 80-86 |
| A moving block's entity is also finalTick'd when its cell is overwritten (`preRemoveSideEffects`), which no piston action does (moving pistons are immovable) | `PistonMovingBlockEntity` 286-288 |
| Extended pistons, moving pistons, obsidian, block entities are immovable; 12 blocks max | `isPushable` 215-257 |

## 3. Redstone dust (`RedstoneWireBlock`, `DefaultRedstoneWireEvaluator`)

* Vanilla still uses the default evaluator (redstone experiments are off): when a wire's power
  changes it updates itself and its 6 neighbors, and their neighbors, in **`HashSet<BlockPos>`
  order: position dependent**. The judge therefore tests every design at many world positions.
* Dust powers the block below it and the blocks it points into. While a wire computes its own
  input it turns off every wire's signal (`shouldSignal=false`), which is why dust does not feed
  dust through a solid block ("weak power"), and why **dust does not pick up a block powered only
  by other dust**: to tap a dust net one-way into another, put a repeater (or a block strongly
  powered by a repeater) between them.
* **Shape:** the default `redstone_wire` state is a dot, and a dot stays a dot
  (`getConnectionState`: `wasDot && isDot`), powering only the block below. Write an isolated dust
  as a cross (`redstone_wire[north=side,east=side,south=side,west=side]`) when it must point
  sideways: it stays a cross after the shape update and powers all four neighbors. A dust with one
  connection becomes a line through it and points both ways.
* **Strong power back-feeds dust:** a block strongly powered by a repeater (or a lever, an
  observer, a torch below it) powers every dust touching it, including dust *on top* of it, and a
  dust directly UNDER a block strongly powered by an observer lights (gadget `dust-below`). A dust
  only *weakly* powers the blocks it points into or sits on (they power components, never other
  dust).
* A wire connects up a step only if the block above it is not a conductor (`RedstoneWireBlock`
  150): pushing a conductor off a dust step connects the step at once (gadget `instant-on`).
* **Dust reach:** a dust changing power updates the neighbors of its neighbors: a held piston two
  cells away (dust, block, piston) re-checks (gadget `dust-reach`).

## 4. Other blocks

* **Observer:** reacts to *shape updates* of the block it faces (not to neighbor updates),
  schedules a 2 gt tick, turns on, schedules 2 gt more, turns off: a 2 gt pulse starting 2 gt after
  the change, output from its back. It ignores changes while powered and sees 0-ticks
  (`ObserverBlock.updateShape` 67). **Observers on pistons fire twice:** a piston (or its head
  cell) changes state at the event, 2 gt later and again 2 gt after that, so an observer watching
  it pulses twice per motion. Watch a block that moves once, the lever, or another observer.
* **Lever:** strongly powers the block it is attached to, weakly every neighbor. It is a signal
  source for adjacent *components* (dust connects to it, a repeater whose back faces it reads it),
  but an adjacent solid block other than its attachment is not powered: a repeater reading the
  lever is a one-way tap (gadget `lever-tap`).
* **Torch:** 2 gt delay; powers every neighbor except its attachment, strongly the block above; a
  lit/unlit change also updates the neighbors of its neighbors. Burnout: 8 toggles in 60 gt -> off
  160 gt (`RedstoneTorchBlock` 74, 87).
* **Repeaters update quietly:** a repeater switching a solid block in front of it updates that
  block's neighbors only (`updateNeighborsInFront`), not their neighbors. A piston powered through
  that block by quasi-connectivity is not updated: it neither extends nor retracts until something
  next to it changes (silent hold). A pulse shorter than a repeater's delay comes out exactly that
  long; a repeater locked by another pointing into its side keeps its output.
* **Lamp, copper bulb:** a redstone lamp lights in the tick it is powered and goes dark 4 gt after
  (asymmetric delay, visible to observers). A copper bulb toggles `lit` on every rising edge of its
  power in the same tick (`checkAndFlip`) and a comparator reads it as 15/0. Unwaxed copper
  oxidizes over time (random ticks), which observers see: the judge rejects it, use waxed copper.
* **Note block, target:** a note block is a plain conductor whose `powered` state follows
  `hasNeighborSignal` in the same tick (`NoteBlock.neighborChanged`, `setBlockAndUpdate`: it also
  updates its neighbors), so an observer watching it pulses on both edges of its power, with no
  lamp-style fall delay; its `instrument` follows the block below (an observer sees that change
  too). A weakly powered note block passes nothing on: next to a lever it is a zero-delay,
  non-conducting watch target (sf-holdA's 38). A target is a conductor and a signal source (power 0
  unless hit), so dust always connects to it (gadget `target-tap`).
* **Sculk sensor** (`SculkSensorBlock`): a lever flip is a vibration (BLOCK_ACTIVATE/DEACTIVATE),
  and so is a piston's move (a blocked extension is not; observers, repeaters and dust make none). A
  sensor within 8 blocks goes ACTIVE `max(1, floor(distance))` gt after the event, in the
  block-entity phase, stays active 30 gt (power `max(1, 15 - floor(15/8 * d))`, weak on every side,
  strong only into the block below), then COOLDOWN 10 gt during which it ignores the door's own
  piston vibrations. It has a block entity: immovable, never dragged. A player standing on a sensor
  triggers it, which breaks some sculk doors in real play.
* **Calibrated sculk sensor:** active 10 gt (not 30), hears within 16 blocks, outputs on every side
  except its back, and a non-zero signal into its back makes it hear only that frequency (lever on /
  piston extend = 10, lever off / piston retract = 9). With nothing behind it, it is a one-block
  wireless delay line: activation floor(d) gt after each flip, d up to 16. Echo rule for every
  sensor: once it is INACTIVE again (wake + 20 calibrated, + 40 normal) any piston move within its
  radius wakes it, so whatever it drives must stop moving before then, or the re-run must be a
  no-op.
* **Gravity blocks** (`FallingBlock`, `FallingBlockEntity`): sand, gravel and concrete powder
  schedule a tick 2 gt after being placed (pistons place them at the end of a move) or
  shape-updated; the tick makes them fall if the block below is air, fire, liquid or replaceable.
  A `moving_piston` below counts as support. Slime next to sand does not hold it up. A falling
  block that lands on a moving piston (or anywhere it cannot be placed) breaks into an item;
  `Sandbox.idle()` waits for falling blocks to land, so the judge sees that item.
* **Persistent leaves** recompute `distance` in a scheduled tick 1 gt after a neighbor change
  (`LeavesBlock` 102): a clean 1 gt element (gadget `leaves-delay`).
* **Scaffolding** (`ScaffoldingBlock`): every shape update (a neighbor changed) and every
  placement, its own distance changes included, schedules a tick 1 gt later; the tick re-reads
  `distance` (0 on a block with a sturdy top, else the scaffold below's distance, lowered to 1 +
  a scaffold beside it) and writes it with `setBlockAndUpdate`, which updates its neighbors: a
  second 1 gt element that works with ANY sturdy block as the moving part (gadget
  `scaffold-delay`; a tower on a top iron trapdoor passes the lever's edges up 1 gt per block and
  swallows pulses shorter than 2 gt, link table in section 12). At distance 7 it breaks (an item)
  or, if it was placed at 7, falls as an entity. Only its top face is sturdy (dust, torches,
  repeaters and rails can stand on it), it is not a conductor, pistons pop it, and the judge
  places a design's scaffolds as written with no tick until something next to them changes.
* **Walkway rule:** the judge used to ask the walkway only for *no collision*, so collision-free
  blocks (dust, torches, levers, buttons, rails) could sit in it. Since 2026-10-18 16:40Z nothing at
  all may stand in front of or behind the door ([rules.md](rules.md)).

## 5. Timing of every component

| component | input -> output | delay (gt) | phase / priority | notes | gadget |
|---|---|---|---|---|---|
| lever | click -> powered | 0 (between ticks) | - | strong power into its support, signal to adjacent components | all |
| stone / wood button | click -> on, then off | on 0, off +20 / +30 | scheduled | `ButtonBlock` 97 | `button-pulse` |
| pressure plate | entity -> on, off | +20 (weighted +10) re-check | scheduled | `BasePressurePlateBlock` 116 | - |
| redstone dust | any change | 0 | where the change happens | updates 2 cells deep, `HashSet` order | `instant-off`, `dust-reach` |
| repeater | input change | 2 x delay | scheduled; EXTREMELY_HIGH if it feeds another diode, VERY_HIGH when turning off, HIGH when turning on | a pulse shorter than the delay, even a 0-tick one, is stretched to it; locking is instant (side input) | `pulse-stretch`, `zero-tick-pulse`, `repeater-lock`, `early-window` |
| comparator | input change | 2 | scheduled; HIGH if it feeds a diode, else NORMAL | output recomputed in its tick: a change undone before then (a 0-tick pulse) is swallowed; subtraction keeps its inputs' parity | `zero-tick-pulse`, `comparator-odd` |
| redstone torch | input change | 2 | scheduled | burnout: 8 toggles in 60 gt -> off 160 gt | `torch-delay` |
| observer | shape update from its face | on +2, off +2 more | scheduled | ignores changes while powered; sees 0-ticks | `observer-delay`, `zero-tick-observer`, `odd-observer` |
| redstone lamp | power | on 0, off +4 | off scheduled | visible to observers | `lamp-fall-delay` |
| copper bulb (waxed) | rising edge | toggles 0 | in the update | comparator reads 15/0 | `bulb-toggle`, `bulb-pulse` |
| note block | rising edge | `powered` 0, sound = block event (same tick) | block-event phase | vibration for sculk | `odd-pulse`, `note-watch` |
| target block | projectile hit | power, off +8 (arrow) / +20 | scheduled | weak power; dust next to it bends into it | `target-tap` |
| lectern | page turn | 2 gt pulse | scheduled | `LecternBlock` 152 | - |
| daylight detector | time of day | updated every 20 gt (game time % 20 == 0) | block-entity phase | locked to absolute time; constant with `advance_time` off | - |
| bell | rising edge | rings at once, no redstone output | in the update | vibration | - |
| sculk sensor | vibration | active after floor(distance), 30 gt (calibrated 10), cooldown 10 | block-entity phase | category sculk | `sculk-hold`, `sculk-delay-line` |
| piston | power | powered between ticks (lever) or in the scheduled or block-event phase: acts in that tick's block-event phase (lever: tick 1); powered in the block-entity phase (a landing): next tick | block-event phase | moved blocks: section 2 | all |
| persistent leaves | neighbor change | 1 | scheduled | a clean 1 gt element without a block that breaks | `leaves-delay` |
| scaffolding | any shape update (a neighbor changed) | 1 per block | scheduled (NORMAL) | distance re-read from the floor / the scaffold below / 1 + a scaffold beside; a tower or chain passes an edge on 1 gt per block and swallows pulses < 2 gt; breaks at distance 7 (falls if placed at 7); pistons pop it | `scaffold-delay` |

No gadget for: lectern (its pulse needs a page turn, a menu click), daylight detector (follows the
time of day, frozen in the judge's worlds; a right-click inverts it at once), bell (no redstone
output), pressure plates and target blocks (need entities or projectiles). Their rows are read
from the source only.

**Pulse generators:**

* **0-tick pulse on a piston** (extend and retract in one phase): power it, and have its own move
  update a silently armed partner that moves its power source away (`zero-tick`; the speed doors'
  "F 0-ticks" is the same pattern with landed redstone blocks). Variants: `-normal`, `-reach`,
  `-onplace`, `double-move`.
* **0 gt power edges**: `instant-off` (moved source), `instant-on` (conductor off a dust step);
  **0 gt power pulse**: `zero-tick-pulse` (a redstone block placed and shoved away in one phase).
* **2 gt pulse**: an observer (`dual-edge`, `spit`); **20/30 gt**: buttons; **any 2d**: repeater.
* **Odd pulses**: k3..k5 from an observer on a k1 piston (`odd-observer`); k5..k7 / k7..k9 from a
  repeater reading a k1-moved redstone block + note block + observer (`odd-pulse`); parity flip by
  persistent leaves (`leaves-delay`).

## 6. What happens in 0 gt (inside one block-event phase)

Block events (piston moves) run FIFO until the queue is empty, so any number of piston actions can
chain in one tick as long as every link is triggered by something that happens inside the phase.

| effect | why (source) | gadget | vanilla |
|---|---|---|---|
| **0-tick piston**: a sticky piston extends and retracts in one phase; the block it pushed is PLACED in that tick | 180-190 (finalTick two ahead) | `zero-tick` | PASS |
| a 0-ticked NORMAL piston's block still lands at t+2 | 204-205 | `zero-tick-normal` | PASS |
| **reach-through**: a 0-ticked sticky piston with an empty front pulls the block two cells away | 101-110 (type 1: nothing it pushed is moving), retract resolver starts at pos+2 | `zero-tick-reach` | PASS |
| a piston placed by a 0-tick fires in the same phase | `onPlace` 80-86 | `zero-tick-onplace` | PASS |
| one block, two placed moves, one tick | finalTick's `setBlockAndUpdate` updates the next (armed) piston, which is 0-ticked in turn | `double-move` | PASS |
| power LOSS: a moved redstone block (or any moved conductor) cuts power at once; pistons on that line queue retracts that run in the same phase | `moveBlocks` (source becomes `moving_piston`) | `instant-off` | PASS |
| power GAIN without a moved source: a conductor pushed (or pulled) off a dust step connects the step at once | `RedstoneWireBlock` 150 (`canConnectUp = !above.isRedstoneConductor`) | `instant-on` | PASS |
| power GAIN from a placed source: a redstone block finalTick'd by a sticky retract powers its neighbors in the block-event phase (their events run in the same phase) | 180-190 + `finalTick` 276 | `zero-tick-onplace` (a piston instead of a redstone block), `spit-saves-tick` | PASS |
| an observer SEES a 0-tick (the cell changed twice inside the phase): one pulse 2 gt later | `ObserverBlock.updateShape` 67-68 | `zero-tick-observer` | PASS |
| a 0-tick POWER pulse (source placed and removed inside one phase): lamp lit 0..4 gt, repeater stretches it to its delay (on 2 gt later), comparator swallows it; a piston next to the source moves only if its event is queued before the one that removes the source (it is then 0-ticked itself) | `DiodeBlock.tick` 50-63, `ComparatorBlock` 145-146, `RedstoneLampBlock` 36 | `zero-tick-pulse` | PASS |
| event order = update order (W,E,D,U,N,S), FIFO; a piston whose front became a moving block meanwhile does nothing | `runBlockEvents` 1270-1271, `moveBlocks` returns false | `event-order` | PASS |
| a powered piston carried onto a powered spot by a LANDING fires next tick (landing is in the block-entity phase) | `onPlace` | `landing-fire` | PASS |
| a quasi-powered piston ignores its source until any neighbor update (BUD); arming one silently is how 0-tick partners wait | `getNeighborSignal` 116-136 | `qc-bud`, `silent-hold`, every 0-tick gadget | PASS |

**Constraint for doors:** a BUD-armed (powered, not yet updated) piston cannot exist in the REST
state: the judge places a design with a `self` update of every block. Partners and armed pullers
must be armed after the flip: by the lever itself (it only updates its own neighbors and its
support's: a lever next to a piston's quasi cell arms it silently), by dust feeding a conductor
next to the quasi cell from 3+ cells away (dust updates reach 2), or by a redstone block placed
next to the quasi cell (its updates reach only its own neighbors).

## 7. Odd ticks (1 gt)

Every redstone component delays by an even number of game ticks. Odd ticks come from pistons:

| source of odd timing | when | gadget | vanilla |
|---|---|---|---|
| pistons the lever powers directly move at k1 | lever between ticks, events in tick 1 | every lever gadget | PASS |
| a block pushed at t lands at t+2 in the block-entity phase, after that tick's block events: pistons it powers act at t+3 (k1 push -> k4), components it feeds schedule from t+2 (repeater delay 1: k5) | `ServerLevel.tick` 409 vs 459 | `landing-next-tick` | PASS |
| a spit block is placed in the block-event phase of the retract tick: pistons it powers act THAT tick (one tick earlier than a landing) | `checkIfExtend` 108 (`isHandlingTick`) | `spit-saves-tick`, `spit` | PASS |
| so a PUSHED block arrives after 0 gt (0-tick), 1 gt (its sticky pusher unpowered in the next tick's scheduled or block-event phase: spit at t+1), or 2 gt (spit at t+2 in the block-event phase, or landing in the block-entity phase); a PULLED or dragged block always after 2 gt (landing) | 101-110, 180-190 | `zero-tick`, `spit-saves-tick`, `landing-next-tick` | PASS |
| an observer watching a cell a k1 piston changes fires k3..k5; chains stay odd (k5, k7...) | observer 2 gt from the block-event phase | `odd-observer` | PASS |
| a repeater reading a redstone block a k1 piston moves falls at k1+2d; a note block turns that edge into a state change an observer sees | `DiodeBlock` 84-99 | `odd-pulse` (speed-2) | PASS |
| comparator subtraction keeps the parity of its inputs: side input from a redstone block a k1 piston pushes (lands k3) -> output off at k5; pulled away at k1 -> output back at k3 | `ComparatorBlock` 145-146 | `comparator-odd` | PASS |
| **1 gt scheduled ticks**: persistent leaves recompute `distance` 1 gt after a neighbor change (also scaffolding, and 26.3's survival checks of sugar cane/cactus/bamboo/kelp/chorus, which break blocks); a piston at k1 -> leaves at k2 -> observer k4; a piston at k2 (even) -> leaves k3 (odd) | `LeavesBlock` 102, `ScaffoldingBlock` 83/103 | `leaves-delay` | PASS |
| water spreads every 5 gt, lava 30 (fluid ticks) | `fluidTicks.tick` 396 | - | - |
| sculk sensors wake floor(distance) gt after a vibration, in the block-entity phase (any parity, category sculk) | `SculkSensorBlock` 190/208 | `sculk-delay-line`, `sculk-hold` | PASS |

## 8. Techniques the doors use

* **Silent hold** (gadget `silent-hold`, rb5): a piston kept extended by a quasi source that later
  turns off WITHOUT updating it stays extended, unpowered, until any later neighbor update (a
  piston head forwards neighbor updates to its base, so a block landing next to the head counts).
  A repeater switching a solid block only updates that block's neighbors: feed the piston's quasi
  cell through such a block to drop the hold silently.
* **Redstone block shuttle** (rb1): a sticky piston pushing a redstone block from next to piston A
  to next to piston B switches A off at once (the block becomes a moving piston that tick) and B
  on 2 gt later (when it lands): one actuator, ordered "A then B"; pulled back it gives "B off then
  A on". A **pulse-driven shuttle breaks the drag**: a 2 gt pulse on the shuttle piston spits its
  redstone block, which is then placed in the block-event phase; the piston it powers fires in
  that same phase, before T (lifted 2 ticks earlier) has landed, so T is not dragged. Keep it
  steady.
* **Riders** (center-mech, riders): a redstone block carried by a slime structure powers a piston
  at rest and lets it go when the structure moves. It must touch the piston itself: in a
  quasi-only position its return never updates the piston, which then stays retracted. Two pushers
  can share one powered block (v2u2: both side structures pushed out from under the walkway off one
  obsidian block). An observer carried by a structure fires 2 gt after EVERY landing (gadget
  `rider-observer`); a plain block glued to a structure and watched by an observer is a motion
  trigger, a **flag** (`flag-observer`, fresh-b's mirrored pump).
* **Sculk hold** (gadget `sculk-hold`, top-control): one sculk sensor next to a piston holds it 30
  gt after a lever click. It wakes in the block-entity phase floor(distance) ticks after the click,
  so a piston whose source is the lever directly retracts at tick 1 before the sensor can hold it:
  cover the first ticks with a 4 gt repeater from the lever. The door's own late pistons move
  inside the 10 gt cooldown, so they do not re-trigger it (rb2s2 50, m51s 48).
* **Pulse-driven pistons** (logic-min, gadget `pulse-toggle`): any door piston that only has to
  move its block once per lever edge can be retracted at rest and fired by a 2 gt observer pulse
  (pull on open, push + spit on close). The measured windows (`door ideal`): side/bottom pistons
  accept the pulse at any tick on both edges; A needs one pulse 0..4 gt after the flip plus the two
  B-edge pulses. **Three pulses toggle it too** (fx1, m05, sf-holdA): one net move, timed by the
  last pulse, so a piston can reuse a source that fires three times per flip.
* **Pull-type pulse piston** (fc2): a sticky piston two cells from a structure (one-cell gap) pulls
  it on the first 2 gt pulse and pushes + spits it back on the second, so a side column can be
  pulled open from OUTSIDE its backbone.
* **Stack feed** (observer-seq, gadget `stack-feed`): one pulse fires a stack of 3 retracted
  sticky pistons: a cross dust behind the stack at the top level points into the top piston, its
  floor powers the middle one and quasi-powers the bottom one.
* **OR of two repeaters in one dust:** a dust sandwiched between the output block of one repeater
  (above it) and of another (below it, its floor) is lit by either, and a cross dust powers the
  block it points into: a 2-input OR feeding a torch with 1 dust (logic-min P_T).
* **Early window** (gadget `early-window`): a delay-1 repeater reading a block fed by an observer
  pulse and by a delay-2 repeater from a SECOND observer gives a window at t+4..6 and t+8..12 (a
  2 gt hole: a repeater's turn-off tick outranks another's turn-on tick), vs t+10..18 for the plain
  pulse stretcher.
* **Symmetric door control** (fresh-c fc1/fc2): with an A-on-B elevator, T dragged out by the right
  structure and one shuttled redstone block (A off then P_R on / P_R off then A on), no hold is
  needed if (1) A gets one pulse BEFORE the shuttle moves, on both edges (open: A is still powered,
  the pulse is swallowed; close: A spits C down one cell, freeing T's cell), (2) the shuttle piston
  is the lever through one repeater (>= 2 gt after that pulse), and (3) the B window comes at the
  same delay on both edges: on close A is extended again and B's push fails (gadget
  `blocked-push`). Close = spit C, T back, A pushes T and C home.
* **Asymmetry for free:** a lamp lights at once and goes dark 4 gt after its power ends (m05's late
  right pulse); a waxed copper bulb toggles on rising edges only, so an observer on it pulses once
  per edge (`bulb-pulse`); a push-type and a pull-type pulse piston fed the same pulse act in
  opposite orders on the two edges (`order-flip`).
* **Observer chains** cost 1 block per 2 gt. The tick at which a chain can fire a given cell is
  fixed mod 4 (aim at the piston's quasi cell to shift it by 2), and an observer watching a
  pulse-pulled door block fires 2 gt earlier on close than on open (wire19sf).
* **In one plane** (flat): repeaters cannot be locked (their sides face out of the plane), and a
  lever powers its neighbors, so it must stay off every piston and quasi cell.

### Wiring tricks for tight flat boxes (found 2026-10-18 evening, below-72 hunt)

Read these before wiring a flat door in a box smaller than 8x9. Each names its proof (gadget or
track notes).

* **Pulse merger** (`pulse-merge`, flat-seam e89): observer -> delay-2 repeater -> observer turns a
  double pulse (two 2 gt pulses up to 4 gt apart) into ONE 2 gt pulse; 8 gt apart stays two
  (`merger-gap`).
* **Observer busy window** (wire56 NOTES sec 5): an observer ignores changes while it is powered or
  has a tick pending, so a piston firing twice exactly 6 gt apart gives its watcher 3 pulses (an
  odd count), and an observer on a door block's PARKING cell can see 3 changes per edge. Links
  between wiring regions are therefore not always even (this refutes the parity premise of box56's
  7x8 proof and of `door topology`).
* **Memory link** (`memory-link`, wire56): waxed bulb B1 -> comparator C1 -> bulb B2 -> comparator
  C2 -> observer gives exactly one 2 gt pulse per lever edge from any even input (~7 cells with the
  comparator floors). A single bulb + comparator window FAILS on close for a pull-type piston
  (`memory-window`: the long window pushes the block back, the sticky retract pulls it out again).
* **Torch link** (`torch-link`, box72): a side piston held extended by a torch on the lever's
  block moves once per flip, so the observer watching it fires once (a parity-clean link). Any
  LEVEL-held piston is such a link (wire64 on 8x8-118: E3+ and E5+).
* **Lever outside the box** (box72 m2w): only the lever is excluded from the volume; hang it on a
  box cell from outside (its support block is inside).
* **Conductor feeds do not update** (box64): a powered plain block above a piston quasi-arms it
  without updating it, so pist19r's B can be fed by a conductor at (2,5) without firing A.
* **Note-block window** (wire64 w102-left): a note block under a cross dust carries a window into a
  piston; an observer watching the note block fires at both ends of the window.
* **Pitfall: a real observer next to a pull-type piston fires twice per pulse** (start of the
  extension and end of the spit retraction), 4 gt apart (flat-seam cd8-bw).
* **Dropper/dispenser = QC power sensor**: immovable full conductor whose `triggered` follows
  power including the cell above (QC), re-read on neighbor updates only and set WITHOUT neighbor
  updates; an observer facing it fires on each flip.
* **Non-conducting full blocks** (glass, tinted glass, glowstone, sea lantern, waxed copper grate):
  sturdy (they hold dust, diodes, torches) but never powered, so they stop leaks into neighboring
  pistons and quasi cells.
* **The only 1 gt delay**: a scheduled leaves distance update (persistent leaves next to a log);
  every other component delays an even number of ticks.
* **In-plane limits**: no repeater locking and no comparator side inputs (both need a diode at
  z+-1).
* **Carried puller: a middle-row side block leaves vertically in 2 rows** (vmid vb2/vb3, ideal
  plans at 32 positions): a sticky piston facing UP resting in the corner (-1,-1) is pushed into
  the ring cell (0,-1) by a piston at (-2,-1) (1 gt pulse: spit), pulls ML (pos+2 = (0,1)) to (0,0),
  is pulled back by the same piston (now a pull-type, its front empty), and a puller at (0,-2)
  finishes ML to (0,-1); anything already lowered into row -1 (BL, BM) rides one cell right with it.
  An elevator needs 3 rows, this 2. It is drivable in-plane only facing up: its quasi cell is (0,0),
  fed by a plain stone in the ring cell (-1,0) that a component at (-2,0) powers strongly. The top
  mirror (facing down, into (0,3)) has its quasi cell at (0,4), the finishing puller, so it cannot
  fire without that puller (vc1).
* **Door blocks and ring stones relay power into the opening's quasi cells**: a strongly powered
  plain block in a ring cell gives signal into the opening cell next to it, which QC-powers a piston
  facing up just below that cell (the carried puller above); a parked door block does the same.
* **Two pulses 8 gt apart from one pull-type puller** (cosearch2 pockets, ring56 s1
  `runs/pk-s1-k5/pocket0-set1.txt`): observer watching the puller -> stone -> delay-4 repeater ->
  observer watching the repeater: the puller's extend and pull retract give two output pulses (t13,
  t21 after a t0 pulse, both edges), here fed into the elevator's lower piston by QC through the
  empty cell of the upper one.
* **A redstone lamp mirrors a DE input cell for an observer chain** (cosearch2 f131X): a lamp next
  to a strongly powered input cell lights at once and goes dark 4 gt after; an observer on the lamp
  gives one pulse at the window's start and one 4 gt after its end (D1's b and c shots from DE's
  window).
* **Lamp merger for A's driver** (w63top lamp plan, ideal/lamp-f.ideal): an observer watching a
  redstone lamp fires once per lamp edge; a lamp whose power ends exactly 4 gt before a new power
  pulse turns off and on in ONE tick (its off tick was scheduled first), so the observer fires once.
  Lamp state changes use flag 2 (no neighbor updates): a lamp next to a piston's quasi cell, fed
  through a conductor, never updates that piston. B's double watcher pulses then give exactly one
  A2 and one A3 pulse.
* **A parked door block as a gate** (w63top u7/m9, m10r): a door block that sits in a frame cell
  only while open (TL at (0,3), TR at (2,3)) relays a repeater's strong power into the piston next
  to it only on open; on close the same pulse does nothing. A conductor driving a piston's quasi
  cell (instead of an emitter) QC-arms it without updating it, so it fires only where something
  else updates it.
* **Wall torch on a static frame stone** (w63top NOTES "Pivot"): a torch hung on the outer face of
  a static frame stone ((-2,2) on (-1,2)) powers the side puller below directly and the one under
  that by QC (it emits into (-2,1)): a level-held 2-stack, switched off by powering the stone.
* **A level-held piston is a clean link**: it moves once per lever edge, so an observer on its base
  fires exactly one pulse per edge (box72's torch link, c79's (4,0) on MR's puller, u7-m10a's R0).
* **Corner pistons of a 2-row bottom take no observer pulse** (s79 NOTES §2): the corner piston K
  (-1,-1) facing east is fed only from (-2,-1) (east-pointing emitter) or (-1,-2) (up-pointing): an
  observer pointing into K watches (-3,-1) or (-1,-3), outside a 7-wide x 2-row bottom; repeaters
  and comparators have no floor there. K can only get dust on a powered (-2,-2), a conductor at
  (-1,-2) powered by an observer at (0,-2) watching U, a torch (inverted) or a sensor window. With
  c79's mechanism the dust route takes L0's only feeder and the conductor route takes U's only
  feeder: c79's mechanism is not wirable in 7x8 (cosearch pockets k=4 with sculk: no filling drives K).
* **A 1 gt pulse makes a piston watcher fire once** (s79 c78y/c78z): an observer watching a sticky
  piston fires at its extension (X+2) AND at its retraction's landing (X+4 -> X+6) for a 2 gt pulse;
  with a 1 gt pulse (retract at X+1, landing at X+3 inside the busy window) it fires once. A 1-tick
  sticky retraction still pulls the block two cells ahead (open: U pulls BM from (1,0)); on close the
  push is spit. The U-watcher bottom-left (observer (0,-2) east -> stone (-1,-2) -> K; dropper
  (-2,-2) -> observer (-2,-1) down -> L0) works only with it (ideal-rs: never with 2 gt).
* **A lamp as a level conductor adds +4 on one edge** (s79 c78z): a redstone lamp under the lever's
  dust holds the pullers below it like a dropper, but goes dark 4 gt after the lever drops, so an
  observer watching it fires at t6 on open and t2 on close (c79's dropper: t2 both): the elevator's
  push moves 4 gt later on open only.
* **Sound crosses sealed regions** (m79s, archive m79s-sculk-63b-50-50b / m79s-sculk-8x8-64b-50b): a
  region that sees nothing the lever moves first (a top-left behind the elevator column, a bottom-left
  behind U) can hear the lever or the t1 pistons: activation = event tick + floor(distance). A plain
  sensor (range 8) ignores a lever more than 8 away and hears the t1 pullers instead; a lever behind a
  wool block is inaudible too. Category sculk.
* **Wool kills the re-arm echo** (m79s): a sensor re-arms 40 gt (plain) / 20 gt (calibrated) after it
  wakes and then hears the door's own late moves (an endless loop); put WOOL (a conductor, so it can also
  be a wire block or the elevator's feed block) on every line from the late movers to the sensor
  (`sculk::is_occluded`: all 6 nudged lines must cross wool).
* **Plain sensor -> lamp -> observer = 2 taps, no echo** (m79s 8x8 K): lit at T, dark at T+34: both
  taps land inside the sensor's 40 gt dead time. **Calibrated sensor -> observer = 3 taps 10 gt apart**
  (T+2, T+12, T+22): an odd count for a pull-type piston (pull/push/pull on open, push/pull/push on
  close), and its window can hold another piston meanwhile (the 63's B through dust -> wool).
* **On a door still moving after the re-arm, read the sensor with an observer rather than letting it
  hold a piston** (clk-item, q69h / q69c, 6x9 doors that move 57-83 gt): a calibrated sensor (range 16)
  hears every piston of a 6x9 box, a plain one (range 8) all but the far corners (the box's diagonal is
  9.4), and a piston's extend / retract is the same game event as the
  lever's on / off (`BLOCK_ACTIVATE` 10 / `BLOCK_DEACTIVATE` 9, `VibrationSystem`), so no frequency
  filter separates the lever from the door. A sensor powering a piston directly re-holds it on every
  echo wake: q69h's Qr (calibrated, woke t2, again t29 and t57), q69c's (0,4,1) (plain, woke t5, again
  t49 from a piston 2.2 blocks away whose line crosses only free cells, so a wool DOOR did not help)
  and q69c's A were all broken. (One direct drive does work: q69h's W, with a calibrated sensor at
  (-2,-2) next to the cell the slime drags W into, plus a wool shield at (0,-2); redsim 32 x 3 and the
  game. So measure, do not assume.) An observer watching the sensor instead turns every phase change of
  every echo cycle into one more 2 gt tap, which is harmless for a piston that tolerates extra pulses:
  q69c's A (sensor (3,-3), observer (3,-2) facing down) and (1,-1,1) (sensor (0,-3), observer (0,-2))
  both WORK at 32 positions x 3 cycles and in the game. The observer may write into a WOOL block that
  is both the wire (it QC-arms the target through the target's quasi cell) and a shield: with stone
  in that cell (3,-1) the same filling fails. `ideal-rs --holds` models one hold and no echo, so its
  windows are optimistic: check a sensor input with a real sensor block in redsim.
* **A block riding a slime structure is a free once-per-edge trigger** (clk-item, q69c's P_T): a stone
  set on top of the slime column is dragged on every slime move, and an observer watching its rest
  cell fires on each, on both edges, with no input of its own (observer (0,5) facing west watching
  the stone at (-1,5), QC into P_T at (1,4) through (1,5)). The cell the stone is dragged into must
  stay empty.
* **A lever arms the pistons two cells below its block** (clk-item, q69h): the block a lever hangs on
  is strongly powered, so it QC-arms a piston whose quasi cell touches it. A lever on the right wall's
  back face at (3,1,1) extends q69h's K at (3,-1) (quasi cell (3,0)) the moment it turns on, while
  (3,2,1) arms nothing there; q69c, whose corner piston sits at the top, takes (3,1,1) fine.
* **Glass under a wire dust** (m79s s88h/s88i): a stone under a dust is strongly powered and QC-arms
  any piston whose quasi cell it touches; glass carries the dust without powering anything.

Notes for builders of fast doors:

* Pistons react to a landing one tick late (landing = block-entity phase): spit (2 gt pulse) or
  0-tick to place blocks in the block-event phase instead.
* A reach-through puller (sticky piston, empty front, 0-ticked) moves a block two cells away
  without ever having its head next to it at rest: the cheapest way to pull C or T out of a cell
  that was full at rest.
* Arm 0-tick partners after the flip (section 6's constraint), and give each one a power source it
  can shove away.
* Use one update to order several pistons (W,E,D,U,N,S), or chain them through their own moves:
  every piston action updates its base's and front cell's neighbors.
* Two chains of events started together interleave in the FIFO queue (each event appends its
  follow-ups at the end): if chain B needs chain A's result, start B from A's last action (a relay)
  or one tick later. A piston whose push fails when its event runs never retries. (Interleaving
  goes one event per chain per round, so a chain that must act second can also be made longer:
  `open-4-close3` pads T's chain with one piston to run it after C's in the same tick.)
* **T flip-flop level for a level-held piston** (c88 `tracks/c88/s24.txt`, the solid 8x8 c88-s26):
  a waxed copper bulb toggled by ONE pulse per lever edge (an observer on the lever's dust ->
  stone -> bulb) read by a comparator is a level that is on at rest and off when open (the judge's
  periodic rest accepts it); repeaters after the comparator delay it like any level (t16 here).
  It made the elevator's top piston B level-held far from the lever, so B moves once per edge and
  its watcher fires ONE pulse per edge (a pulsed B's watcher fires twice: extend, then landing).
* **Fire a QC-only piston with an update, or deliberately without one** (c88 s22/s25): a piston
  powered only through its above cell reacts only when something updates it. An emitter INTO the
  above cell updates that cell's neighbors, the piston included (observer (0,4) into (1,4) fires A
  at (1,3)); powering a solid block NEXT to the above cell ((0,4) or (2,4)) updates that block's
  neighbors only, so the piston stays put (s22's A never pushed). s25 uses the second on purpose: a
  stone (2,4) powered from (3,4) fires A's top pull at (1,4) (its neighbor) but leaves A at (1,3)
  alone on close (QC-powered, never updated), which removes c79's close hazard.
* **Odd-parity reshaper** (c88 s26): an observer on a once-pulsed sticky piston fires twice (extend,
  then landing 4 gt later). Feed that double pulse into the empty quasi cell of a NORMAL piston
  and watch the normal piston with an observer: the downstream train is odd on both edges (3 on
  open), and three pulses toggle a pulsed sticky piston (BR's puller here).
* **Copper bulb + observer counts input EDGES, not pulses**: the bulb's `powered` flips on both
  input edges, so `bulb-pulse` gives one pulse only for inputs <= 2 gt; a 30 gt sculk window into a
  bulb under an observer gives two pulses (c88 s15sk).
* **A normal piston as a 1-block watched relay** (shrink, archive t7b-pw42-42b): an observer fires
  a normal piston whose head pops into the cell another observer watches: one block where a
  conductor + a watched note block took two (t7b's row -4 route: piston (2,-4) facing west into
  (1,-4), watched by (0,-4)).
* **A moving observer = merger + router** (shrink, archive t7b-ob40-40b): a double pulse (an
  observer on a pulse-pulled piston) into a sticky piston that faces an observer: pulse 1 pushes
  the observer next to target A (2 gt: spit, it stays) and it fires A on landing; pulse 2 pulls it
  home and it fires again on landing, into a different cell (here the empty quasi cell of a normal
  piston). One pulse per edge into A plus a second, later signal elsewhere, from 2 blocks (sticky
  piston + observer) where t7b used a delay-2 repeater merger + support + observer + route.
* **Search with pistons in the palette** (shrink): `door fill --palette ...,sticky,piston --gaps`
  found every one of the above; palettes without pistons (pockets defaults, satwire) cannot.

## 9. Floors for the fastest 3x3 door

Cells: opening x, y in {0,1,2} in the plane z = 0; the walkway is the same x, y for |z| <= 3. C =
center (1,1); T = an edge cell next to it (say (1,2)); E = the cell outside T ((1,3)). A door is
open when no cell of the opening or walkway blocks (moving blocks block) from that tick on; closed
when the 9 cells hold their rest blocks (placed) from that tick on.

**Floors (solid doors): open >= 4 gt, close >= 1 gt.** The wired doors are at 4 / 15 (tick-door,
229 blocks) and 9 / 7 (speed-2). The speed
track's bound (7 / 6) is for one topology (the edge block above the center leaves through the
center's exit cell); routing that edge block out through its corner instead lets the center start
moving in tick 1, which is what makes 4 possible. **The same machine opens in 4 and closes in 3
(`open-4-close3`, ideal inputs, vanilla-identical).**

### Open >= 4 (proof)

1. Nothing moves before tick 1: the flip only queues events, they run in tick 1's block-event phase
   (section 1).
2. C is 2 cells from the outside of the 3x3 x 7 box in every direction (4 along z), and a piston
   moves a block 1 cell per move: C needs 2 moves, the second one taking it out.
3. A cell a block moves out of is air at once (section 2), so "open" is the tick in which C's last
   move STARTS (if everything else is out by then).
4. C can only move again once it is a placed block. After a pull, a drag or a plain push it is
   placed in the block-entity phase of t+2, so the next move is at t+3. The only early placement is
   `finalTick` by a sticky piston retracting while C is the block two ahead of it, i.e. C was the
   FIRST block of that piston's push: the piston's base is adjacent to C's start cell, and all six
   cells next to the center are in the opening or the walkway. That base is a `moving_piston` for
   2 gt after the retract (t' >= t) and can be moved out at t'+3 at the earliest; the door is not
   open before.
5. Either way open >= t + 3 with t >= 1: **open >= 4**.

(Solid doors only: gravity door blocks leave their cell as falling-block entities, which no piston
has to move twice; the gravity board's g10 opens in 3 at some positions.)

### An open-4 schedule (every step is a gadget)

The edge block T must leave T's cell before C's first move, and NOT through E (C's puller sits
there): it goes sideways through its corner, which costs it 2 moves too (k1 into the corner, k4
out), in parallel with C.

| tick | moves | gadget |
|---|---|---|
| k1 | the lever's piston shoves the redstone block feeding the pullers' dust line: the 6 side/bottom pullers (extended at rest, heads touching their blocks) retract in the same phase: ML, BL, MR, BR, TR, B out; TL pulled up into (0,3) | `instant-off` |
| k1 | TL's departure updates P_T at (-1,2) (armed after the flip): it extends into TL's empty cell and is 0-ticked by its partner X_T (in P_T's quasi cell): reach-through, T pulled into (0,2) (lands k3) | `zero-tick-reach` |
| k1 | T's departure updates A' at E (quasi-powered through F's cell): it extends into T's cell, its partner X_A shoves that source up and updates it: reach-through, C pulled into T's cell (lands k3); the shoved redstone block lands next to F's quasi cell at k3 and arms F silently | `center-exit` (tick 1) |
| k4 | an observer chain off the lever (2 observers: k4) unpowers the two aside pullers: A' pulled out of E, P_T pulled out of (-1,2) | `observer-delay`, `center-exit` (Y) |
| k4 | E's emptying updates F (above E): F extends into E, its partner X_F shoves its quasi source, F retracts: reach-through, C pulled into E: **C out** | `center-exit` (tick 4) |
| k4 | the same at (-2,2): G_T reach-pulls T out of TL's cell into (-1,2): **T out, open at 4** | `center-exit` (tick 4) |

**Built: gadget `open-4`** (ideal inputs: the scripted redstone blocks stand for the lever flip at
k0 and an observer chain at k4): 9 stone door blocks, 7 plain pullers, the 4 reach-through pullers
with their partners and the 2 aside pullers. The opening is full at k3 (T and C sit in the top edge
cells) and clear from k4 on. What remains for a real door: wire the inputs (lever, instant-off
line, arming paths after the flip, the k4 observer chain).

The speed track's topology (T pulled into E at k1, lands k3; a pusher shoves [A', T] at k4 so a
puller takes E; C into T's cell at k4, out at k7) has floor 7; its best wired door opens at 9
(speed-2, with an odd-tick pulse).

### Close >= 1, and what each schedule costs

Close is measured on placed blocks, so a 0-tick placement counts in the tick it happens. Trivially
close >= 1 (nothing moves before tick 1). C needs 2 moves in (E -> T's cell -> center) and T must
enter its cell after C went through:

* **close 1-3 (with same-tick relays)**: C pushed E -> T's cell by a 0-ticked piston above E (C
  placed: `zero-tick`), a piston carried into E by a 0-tick fires in the same phase
  (`zero-tick-onplace`) and 0-ticks C into the center; **gadget `center-return` does this for the
  center: C placed in the edge cell at k1 and in the center at k2**. T comes back through its
  corner: pushed into the corner from the side cell R by a 0-tick, a piston carried into R fires and
  0-ticks T into its cell, the corner block enters from the other side cell U (**gadget
  `corner-return`: T placed in the corner at k1 and in its own cell at k2, the corner block at
  k3**). The carried pistons must be ordered AFTER the cell they enter is emptied, which a 2-cell-away
  pusher cannot see directly: a relay (a piston next to the cell that pushes a conductor off a dust
  step feeding the pusher: `instant-on`) does it in the same tick, the tick boundary in the next. The
  6 other blocks only need plain pushes at k1 (landing k3). Two carried-piston chains started by the
  same tick boundary interleave in the FIFO block-event queue, so C's and T's chains share k2 only
  with a relay or with T's chain padded (built: close 3); with tick boundaries only, close 4.
* **close 6**: the speed track's: C pushed into T's cell at k1 (spit), T pushed into E, F re-extends
  at k4 pushing [T, C] together; C is the second block, not finalized: lands k6 (the boards: 7).
* **close 4 on the open-4 machine: built** (`open-4-cycle.drive`). Its open state IS the close
  layout: k1 F 0-ticks C into T's cell and G_T 0-ticks T into TL's cell, the 6 plain pullers push
  their blocks home (land k3); k2 Y_A 0-ticks A' back into E, A' fires as it is placed and 0-ticks C
  into the center; k3 Y_P 0-ticks P_T back, P_T fires and 0-ticks T into its cell; k4 TL's puller
  0-ticks TL home. Started in the same tick, the A' and P_T chains interleave (FIFO: P_T fired while
  A''s head was still in T's cell, and a failed push never retries), hence one chain per tick there.
* **close 3 on the same machine: built** (`open-4-close3.drive`). Both chains start at the k1/k2
  boundary, A''s first; the queue runs them one event per chain per round, so T's chain is made one
  event longer (Y_P fired by the update of a padding piston D1 and 0-ticked by a partner in its own
  quasi cell), which puts P_T's push one round after A' left T's cell: C and T both placed at k2, TL
  at k3 (5 more padding pistons would bring TL into k2: close 2).

Open + close >= 5; the open-4 machine does open 4 + close 3 = 7 with ideal inputs, against the
wired doors' 9 + 7 (speed-2) and 4 + 15 (tick-door).

### What a lever-driven open-4 must provide (the script's inputs)

| when | scripted input (open-4-cycle.drive) | role | what can provide it |
|---|---|---|---|
| open k0 | air at the 7 pullers' redstone blocks, TL's first | the 7 plain pullers retract at k1; TL's puller must run before B's (F's update while B still blocks it) | one lever-fired piston shoving the redstone block of a dust net feeding them (`instant-off`); the dust order is position dependent, so feed TL's puller separately and first (update order W,E,D,U,N,S) |
| open k0 | redstone blocks next to the quasi cells of X_T and X_A, and R_T | arm the k1 partners and P_T silently | the lever itself next to a quasi cell (it updates only its own and its support's neighbors), or dust -> conductor -> quasi cell from 3+ cells away |
| open k3->k4 | air at Y_A's and Y_P's redstone blocks | the aside pullers retract at k4 | an observer pair off the lever (k4) firing a piston that shoves their redstone block (`observer-delay` + `instant-off`) |
| open k3->k4 | arm G_T (R_G + X_G) and X_F | k4 partners | the same k4 pulse, or R_A's own landing (F is armed that way already) |
| open, idle | disarm the 4 partners | ready for the close | any time while open (each removal updates the partner's head) |
| close k0 | the 6 plain pullers' redstone blocks back | plain pushes at k1 | power ON at k1 on the lever-off edge: an instant-on (a conductor pulled off a dust step, `instant-on`); a redstone block merely pulled back lands at k3 and the pushes would land at k6 |
| close k0 | F and G_T 0-ticks (R_F2 / R_G2 ordered before X_F2 / X_G2) | C and T into the top-row cells, placed at k1 | the lever-off edge reaching the pistons before their partners (update order), partners armed after the open |
| close k1->k2 | R_A3 + X_A arming, Y_A 0-tick | A' carried back into E, fires, C home at k2 | an observer pulse at k2 off the lever-off edge |
| close k2->k3 | R_T4 + X_T arming, Y_P 0-tick | P_T carried back, fires, T home at k3 | a k2 observer + 1 more link (odd: k3 comes from a k1 piston's observer, `odd-observer`) |
| close k3->k4 | R_TL3, X_TL arming, a poke next to TL's puller | TL home at k4 | an observer pair at k4 |

(`open-4-close3`: the k2->k3 row moves into k1->k2, T's chain started by a padding piston D1 fired
at the same boundary; TL's row moves to k2->k3.) Observers fire on both lever edges, so every
observer-driven input of the open also fires on the close (and vice versa): the wired door must
make those runs no-ops (`blocked-push`, `order-flip` patterns) or route the two edges apart (a
copper bulb toggles once per rising edge: `bulb-pulse`). The tick-door track wired the open to a
lever: `archive/tick-door-open4-229b.txt` opens in 4 (closes in 15, 229 blocks).

## 10. Gadget catalog

Each gadget is a small design plus a drive script in `designs/gadgets/` whose `expect`s prove the
behavior: `mctas door drive --quiet designs/gadgets/*.drive` prints PASS/FAIL per gadget.

| gadget | blocks | what it proves |
|---|---|---|
| `dual-edge` | observer | an observer watching a lever pulses 2 gt, 2 gt after *both* flips |
| `pulse-stretch` | repeater | a pulse shorter than a repeater's delay comes out exactly that long (2 gt -> 8 gt); a second pulse while it is on is swallowed |
| `pulse-merge` | observer + delay-2 repeater + observer | a DOUBLE pulse (two 2 gt pulses up to 4 gt apart, e.g. an observer watching a pull-type piston's gap or a piston that extends and retracts) becomes exactly ONE 2 gt pulse: the repeater is on for exactly 4 gt and its off edge comes while the observer watching it is still on (the repeater's tick has the higher priority). flat-seam e89 (8x9 flat seamless record) uses it twice: the T puller's feed and the relay that watches the T puller |
| `merger-gap` | delay-2 repeater + observer | the pulse merger (flat-seam e89): two 2 gt pulses 4 gt apart come out of the observer as ONE pulse, but two pulses 8 gt apart (an elevator's B window edges) come out as two (box56's parity argument) |
| `window-watch` | note block + cross dust + observer | (wire64) a note block under a cross dust carries the dust's WINDOW into the piston below it (a lift piston held extended for the window, its sticky retract pulls the lifted piece back), and an observer watching the note block fires once at each edge of the window, 2 gt later: the first pulse lands on the lifted piece's arrival, the second after the pull started (families 8x8-102's E3 + lifted L1, no race) |
| `stopper-release` | held-extended sticky piston + observer + puller | (wire64) a piston head held extended at rest is a STOPPER: P's push line ending on it is blocked, so an early pulse does nothing; when a puller takes the block out of P's line during the pulse, its departure updates P, which fires at that tick (8x8-118 + SW+(4,1): L1's shot coupled to D's arming waits until D has pulled ML out) |
| `spit` | observer + sticky piston | a 2 gt pulse makes a sticky piston push its block and leave it (retract event type 2) |
| `qc-bud` | repeater, piston | a piston is powered by anything powering the cell above it, but only acts when updated (BUD) |
| `lamp-fall-delay` | lamp + observer | lamp: rise 0 gt, fall 4 gt |
| `bulb-toggle` | copper bulb + comparator | T flip-flop: toggles on rising edges only, no delay |
| `slime-nonstick` | slime, glazed terracotta, obsidian | slime drags stone but not glazed terracotta or obsidian |
| `silent-hold` | block + repeater | a quasi-powered piston ignores its source switching on and off until a neighbor update |
| `pulse-bridge` | block, cross dust, block | an observer pulse moved one cell sideways into a repeater's back with 4 blocks (mech-50) |
| `lever-tap` | repeater | a repeater reading the lever feeds a net that the lever's block cannot back-feed (mech-89) |
| `pulse-toggle` | observer + retracted sticky piston | one 2 gt pulse per lever edge drives a piston that is retracted at rest: the on pulse pulls its block (still: normal pull), the off pulse pushes it back and spits it (no torch, no steady line; logic-min) |
| `lever-observer` | observer under the lever | the lever's support is an observer facing up that watches it and fires out of its back (logic-min) |
| `stack-feed` | cross dust + floor + observer | one pulse fires a stack of 3 retracted sticky pistons: a cross dust behind the stack at the top level points into the top piston, its floor powers the middle one and quasi-powers the bottom one (observer-seq s6: a whole slime-free side column per dust) |
| `sculk-hold` | sculk sensor + repeater | (category sculk) a sculk sensor next to a piston holds it 30 gt after a lever flip (each flip is a vibration; a lever click or piston move within 8 blocks wakes it floor(distance) gt later), then cools down 10 gt; the repeater bridges the sensor's latency (center-mech) |
| `head-forward` | piston head | an extended piston held unpowered (silent hold) retracts when a block is placed next to its HEAD: heads forward neighbor updates to the base (the rb doors' Q release) |
| `dust-reach` | dust | a dust changing power updates the neighbors of its neighbors: a held piston two cells away (dust, block, piston) re-checks and retracts |
| `observer-delay` | 3 observers | a chain of observers watching each other delays a pulse 2 gt per observer, with no floors and around corners |
| `repeater-lock` | 2 repeaters | a latch: a repeater locked by another repeater pointing into its side keeps its output whatever its input does, and follows the input again once unlocked |
| `honey-slime` | slime + honey | slime and honey never stick to each other: two touching structures move independently |
| `rider-observer` | observer on a slime structure | an observer carried by a structure fires 2 gt after EVERY landing, out of its back wherever it landed: a structure's landings become pulses (riders r3: the left structure re-fires the shuttle piston) |
| `rider-hold` | redstone block on a slime structure | a redstone block carried by a structure powers a piston only in one position: the piston ignores every update (its own source going away included) until the structure comes back (riders r2: the left structure holds Q) |
| `landing-fire` | piston moved onto a powered spot | a piston pushed onto a powered cell extends as it lands (onPlace), no neighbor update needed |
| `no-retry` | piston, moving block | a powered piston whose push line holds a moving block queues nothing and never retries when that block lands two cells away: it sits powered and retracted until a neighbor update (speed track's gotcha) |
| `blocked-push` | 2 sticky pistons | a piston whose push line starts with an extended piston does nothing; it pushes that piston once it has retracted (fresh-c: the B window fires on both edges and is a no-op on close) |
| `dust-below` | observer + stone + dust | a dust directly UNDER a block strongly powered by an observer lights (dust reads all six neighbors): a one-way tap from an observer chain into a dust net (flat track: the top chain hands its pulse to R_B's input dust) |
| `odd-pulse` | piston + redstone block + repeater + note block + observer | an ODD-tick pulse with no odd source: a repeater reading a redstone block that a lever-fired piston moves at k1 changes at k1+2d; a note block next to its output turns that edge into a state change an observer sees: pulse k5..k7 on the on flip, k7..k9 on the off flip (speed-2 a1: the first solid door opening in 10) |
| `sculk-delay-line` | calibrated sculk sensor + repeater | (category sculk) a calibrated sensor d blocks from the lever wakes floor(d) gt after each flip, active 10 gt (radius 16, no output into its back): a one-block wireless delay line; a repeater after it gives a 10 gt window whose observer fires exactly twice, while an observer on the sensor fires three times (wake, cooldown, inactive) (sculk-2 s1) |
| `sculk-wool-occlusion` | wool + sculk sensor | (category sculk) a wool block on the line from the lever hides the click entirely; a piston whose line misses the wool still wakes the sensor; with the wool gone the click wakes it floor(d) gt later. Wool is a plain full block, so wool door blocks or supports could make a sensor hear one lever edge only (when a moving wool block is in the way) or a door piston instead of the lever (sculk-2) |
| `flag-observer` | stone + observer | motion trigger (fresh-b mp6..mp8): a plain block glued to a moving slime structure, watched by an observer that touches no slime (so it is never dragged), gives one 2 gt pulse after each move, out and back; watching the cell the flag moves INTO works as well as the one it leaves |
| `slime-rider` | retracted sticky piston | a retracted sticky piston right under a slime row rides it: pushed down when another piston pulls the row down, dragged back up when the row is pushed back, so it can pump the row from either spot (fresh-b mp6..mp8) |
| `bulb-pulse` | waxed copper bulb + observer | exactly ONE pulse per input pulse/rising edge: a dust standing on a copper bulb toggles it on each rising edge, an observer under the bulb fires once (a lamp would fire twice: lit, then dark 4 gt later). fresh-d s4: fires the bottom-right puller off the P_T pulse without double-firing the middle puller |
| `pump` | observer + repeater + sticky piston | a 3+ gt pulse (repeater delay 2 = 4 gt) makes a sticky piston push a 2-block line one cell and pull back only the first block: the far block advances one cell with no piston touching it (a 2 gt pulse spits the whole line instead; fresh-b mp6..mp8 move C this way) |
| `early-window` | 2 observers + repeaters | a window at t+4..6 and t+8..12 instead of t+10..18 (section 8) |
| `head-qc` | piston head | a piston is quasi-powered through the head of the piston above it, so fc2's X dust needs no conducting floor (fc2-40b-2 stands it on glass) |
| `order-flip` | push-type + pull-type pulse pistons | the two act in opposite orders on the two lever edges when fed the same pulse |
| `sand-drop-tower` | 3 sticky pistons + sand | a 3-piston tower lowers and lifts a sand column 3 cells (the slime chain lets the bottom piston pull both upper pistons down); used by gravity door g4 |
| `zero-tick` | 2 pistons (ideal inputs) | (tick-study) a sticky piston that extends and retracts inside ONE block-event phase places the block it pushed in that tick (`PistonBaseBlock.triggerEvent` 180-190: the extending moving block two ahead is finalTick'd); the 0-tick comes from a silently armed partner piston updated by the push that shoves the redstone block powering it (FIFO block events, `ServerLevel.runBlockEvents` 1267) |
| `zero-tick-normal` | 2 pistons | (tick-study) the same with a normal piston: it retracts in tick 1 too, but only sticky pistons finalTick the block two ahead (`triggerEvent` 204-205 just removes the head): the block lands at tick 3 |
| `zero-tick-reach` | 2 pistons | (tick-study) reach-through: a 0-ticked sticky piston with an EMPTY front cell pulls the block two cells away (retract type 1 since nothing it pushed is moving, `checkIfExtend` 101-110; head finalTick'd to air, `moveBlocks(false)` starts at pos+2); the pulled block lands 2 gt later (pulls are never finalized early) |
| `zero-tick-onplace` | 3 pistons | (tick-study) a piston carried into place by a 0-tick extends in the SAME phase (`onPlace` 80-86 after `finalTick`'s `setBlockAndUpdate`): chains of placed pistons all act in one tick |
| `zero-tick-observer` | zero-tick + observer | (tick-study) an observer watching a 0-ticked piston's head cell fires once, 2 gt later (`ObserverBlock.updateShape` 67: any update from its face while unpowered), though no tick boundary shows the head |
| `zero-tick-pulse` | zero-tick + a second partner + repeater, comparator, lamp | (tick-study) a 0-tick POWER pulse (a redstone block placed and shoved away inside one block-event phase): the lamp lights at once and goes dark 4 gt later, the repeater turns on 2 gt later for its full delay (`DiodeBlock.tick` 50-63: it turns on, then schedules its own turn-off), the comparator never changes (it recomputes in its tick, the input is gone) |
| `instant-off` | lever, piston, redstone block, dust, sticky piston | (tick-study) power LOSS is instant: a moved redstone block is a moving piston at once, its dust line goes dark in the same phase and the extended piston at the far end retracts in tick 1 (a torch inverter takes 2 gt) |
| `instant-on` | lever, piston, conductor on a dust step | (tick-study) power GAIN without a moved source: pushing a conductor off a dust step (`RedstoneWireBlock` 150: a wire connects up a step only if the block above it is not a conductor) lights the upper line in the same phase; the piston it feeds extends in tick 1 |
| `landing-next-tick` | lever, piston, redstone block, piston, repeater | (tick-study) a pushed block lands in the block-entity phase of t+2 (`PistonMovingBlockEntity.tick` 294-327), which runs after the block-event phase (`ServerLevel.tick` 409 vs 459): a piston it powers acts at t+3 (k4 for a lever piston), a repeater reading it turns on at t+2+2d (k5) |
| `spit-saves-tick` | observer, sticky piston, redstone block, piston | (tick-study) a block spit by a 2 gt pulse (retract type 2: `isHandlingTick` in the scheduled phase) is placed in the block-event phase of the retract tick, so the piston it powers extends in that very tick (k4 instead of k5) |
| `leaves-delay` | lever, piston, log, persistent leaves, observer | (tick-study) a 1 gt delay element: leaves recompute `distance` in a scheduled tick 1 gt after a neighbor change (`LeavesBlock` 102); every redstone component delays by an even count, so this flips a chain's parity (piston k1 -> leaves k2 -> observer k4) |
| `odd-observer` | lever, piston, 2 observers | (tick-study) an observer watching a cell a lever-fired piston changes fires at k3 (odd), a chained one at k5; the head landing 2 gt later does not re-trigger it (it is powered then) |
| `comparator-odd` | lever, sticky piston, 2 redstone blocks, comparator | (tick-study) comparator subtraction on odd ticks: rear = a redstone block, side = a redstone block a lever piston moves; the output follows 2 gt after each input change: off at k5 (side block landed k3), back on at k3 after the off flip (pulled away at k1) |
| `center-exit` | 4 pistons + 2 partners (ideal inputs) | (tick-study) the center's two-move exit at the floor: A' (above the empty edge cell) 0-tick reach-pulls C into it at t1 (lands t3); at t4 A' is pulled aside and F, silently armed, extends into the freed cell, is 0-ticked by its partner X_F and reach-pulls C out: C out of the opening at t4 (the open floor is 4) |
| `double-move` | 2 sticky pistons + 2 partners (ideal inputs) | (tick-study) one block, two moves, both placed, in ONE tick: the block finalTick'd by the first 0-tick updates the second (silently armed) piston, which is 0-ticked in turn (what a close in 1 gt needs for the center) |
| `event-order` | 2 pistons | (tick-study) block events are FIFO in update order (one update reaches W, E, D, U, N, S in that order, `NeighborUpdater.UPDATE_ORDER`): of two pistons powered by one redstone block that both want the same stone, the west one moves it and the north one, finding a moving head in front, does nothing |
| `landing-race` | 2 sticky pistons, redstone block, sculk sensor | (sculk-ew) `no-retry` + `event-order`: a powered piston Q whose push is blocked by a moving block queues nothing until that block lands; the landing block's update (W, E, D, U, N, S) then queues Q and any piston A the block powers in that order. Q west of the landing cell takes the redstone block away before A acts; turned 90 degrees (`landing-race-turned`) A fires first and is 0-ticked, spitting its block. Why sculk-2's s4/s1 worked facing north/south only (tracks/sculk-ew/NOTES.md) |
| `button-pulse` | stone button + sticky piston | (tick-study) a button pressed between ticks powers at once and turns off in the scheduled phase 20 gt later (wood 30, `ButtonBlock` 97): the piston extends in tick 1 and retracts in tick 20 |
| `torch-delay` | wall lever, block, wall torch, sticky piston | (tick-study) a torch inverts with 2 gt (`RedstoneTorchBlock` 87): off at tick 2 after the lever, the piston it holds retracts in tick 2; relit and extending at tick 2 after the lever goes off |
| `open-4` | 9 door blocks + 13 pistons + 4 partners (ideal inputs) | (tick-study) a whole 3x3 opening clear at tick 4, the open floor (section 9): 7 plain pullers at k1; T reach-pulled into its corner cell and C into T's cell at k1 by 0-ticked pullers; at k4 the pullers in the exit cells are pulled aside and two 0-ticked reach-through pullers take T and C out. `open-4-cycle.drive` closes the same machine in 4 (k1 C and T into the top-row cells, k2 C home via A' carried back into E, k3 T home via P_T carried back, k4 TL; one carried-piston chain per tick, two started together interleave in the FIFO queue). `open-4-close3.drive` closes it in 3: T's chain padded by one piston so it runs after C's inside tick 2 |
| `center-return` | 4 pistons + 3 partners (ideal inputs) | (tick-study) the center's two-move return with both moves placed: C 0-ticked from E into the edge cell at k1; at k2 a parked sticky piston is 0-ticked into E, fires the moment it is placed (onPlace) and 0-ticks C into the center: C placed at k2 (the close side's carried-piston step) |
| `corner-return` | 4 pistons + 4 partners (ideal inputs) | (tick-study) the edge block's return through its corner, every move placed: T 0-ticked from the side cell into the corner (k1), a piston 0-ticked into the side cell fires and 0-ticks T into its cell (k2), the corner block 0-ticked in from above (k3) |
| `target-tap` | target block | a target beside a straight dust line is powered by it, a plain block there is not: a target is a signal source, so the dust bends into it and powers it; a torch on the target then reads the line (lamp-sweep: `door resynth` put a torch on a target in place of stone + dust + floor, twice in the speed doors o6 and a1) |
| `log-leaves` | oak log on a moving structure + persistent leaves + observer | asymmetric ODD delay (pareto-mid): persistent leaves (no random ticks) re-read their distance 1 gt after a neighbor changes; a log that LEAVES their cell's neighbor at tick m changes them at m+1, a log that ARRIVES is a moving block (distance 7) until it lands at m+2, so they change at m+3. Leaves at the log's rest cell: observer pulse m+3 on the move out, m+5 on the move back; at the cell it moves into: m+5 out, m+3 back. One leaves block per log (two leaves re-read each other and ripple 1 -> 3 -> 5 -> 7). mp8-leafU (43, 15/17), mp7-leafC (46, 15/13) |
| `rb-lamp` | redstone block on a moving structure + lamp + observer | asymmetric EVEN delay (pareto-mid): a lamp next to the cell a redstone block moves INTO lights when the block lands (m+2) and goes dark 4 gt after it leaves (m+4): observer pulse m+4 on the move out, m+6 on the move back (lamp at the rest cell: the other way round). mp8-lampU (44, 14/16) |
| `killer-chain` | pistons + partners (ideal inputs) | (tick-door) two BUD-triggered reach-through 0-ticks in ONE tick: when TR's puller pulls TR up, W (quasi-powered at rest, its push blocked by the door row) extends into the emptied corner and its base update fires its killer: the chain tick-door's open 4 is built from |
| `trapdoor-watch` | repeater, trapdoor, observer, piston | (exotic) a trapdoor flips the tick the repeater's output reaches it, on both lever edges; an observer watching it pulses 2 gt later; a piston above it is never powered through it (a non-conducting relay) |
| `indicators` | fence gate, skull, copper trapdoor, iron door, observer | (exotic) a fence gate, a skull, a copper trapdoor and both door halves flip in the click's own tick, on both edges; an observer watching the skull pulses 2 gt later |
| `rail-line` | powered rails, observer, piston | (exotic) rails 1..9 power up in the click's tick (slope included), rail 10 never; an observer above rail 9 pulses 2 gt after each edge; a piston under rail 5 never moves (rails power nothing) |
| `dispenser-qc` | dispenser, crafter, observer | (exotic) a dispenser triggers through quasi-connectivity on both edges (observer pulse 2 gt later) and cannot be pushed; a crafter ignores the same quasi signal and triggers only when powered directly |
| `scaffold-delay` | scaffolding, observer | (exotic) breaking the block under scaffold A: A turns distance 1 one tick later, the scaffold above it one tick after that, an observer watching it pulses 2 gt later; placing it back reverses it (a 1 gt delay chain) |
| `torch-link` | lever, block, torch, sticky piston (extended at rest), observer | (box72) a LEVEL-driven link piston: a torch on the lever's block holds a sticky piston extended (head stuck to its block); lever on: torch off 2 gt later, the piston retracts and pulls its block; lever off: it pushes it back. It moves once per edge, so an observer watching it pulses exactly ONCE per edge (a pull-type pulse piston gives two: the parity problem). A 7-wide flat door's side middle puller held this way (torch at (-2,0) on the lever's block (-2,-1)) is watched from (-2,2): a parity-clean link from below the side cut at y = 1 to above it, no merger |

The library also holds `hold-2rep`, `note-watch`, `sculk-hold-cm` and `silent-hold-rb` (variants
used by single tracks; their drive scripts say what they prove).

## 11. Vanilla confirmation

`mctas door vanilla-gadget designs/gadgets/<g>.drive` (`crates/piston-door/src/vanilla_gadget.rs`)
places the gadget's rest state in the official 26.3 server (`setblock ... strict`), replays the
script (`use` by a bot player, `setblock` = the game's own command in both worlds) and compares the
watch box after every action and every tick until the simulator is idle, at the judge's 4
positions. Logs: `designs/piston-door-3x3/tracks/tick-study/logs/`.

| gadget | result |
|---|---|
| zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer, zero-tick-pulse | PASS, identical at 4 positions |
| instant-off, instant-on | PASS, identical at 4 positions |
| landing-next-tick, spit-saves-tick, leaves-delay, odd-observer, comparator-odd | PASS, identical at 4 positions |
| center-exit, double-move | PASS, identical at 4 positions |
| open-4, open-4-cycle, open-4-close3, center-return, corner-return | PASS, identical at 4 positions |
| event-order, button-pulse, torch-delay | PASS, identical at 4 positions |

The rest of the library was run through the same tool (`logs/vanilla-gadget-library.log`): the 25
older gadgets whose scripts only `use` levers (bulb-pulse, bulb-toggle, dual-edge, dust-below,
early-window, flag-observer, head-qc, hold-2rep, lamp-fall-delay, landing-fire, lever-observer,
note-watch, observer-delay, odd-pulse, order-flip, pulse-bridge, pulse-stretch, pulse-toggle, pump,
repeater-lock, sculk-delay-line, sculk-hold-cm, slime-rider, spit, target-tap) are all PASS,
identical at 4 positions. The other 15 use the simulator's own `set`/`clear` placement; the tool
replays those as the game's `/setblock` in both worlds and checks the rewritten script's
expectations (`logs/vanilla-gadget-library-set.log`): blocked-push, dust-reach, head-forward,
honey-slime, lever-tap, no-retry, qc-bud, rider-hold, rider-observer, sand-drop-tower, sculk-hold,
silent-hold, silent-hold-rb, slime-nonstick, stack-feed are all PASS too. **The whole gadget library
behaves identically in the official server.**
| `entity-cart-detector` | minecart + detector rail | (category entity) a piston pushing a cart onto a detector rail turns the rail on in the block-entity phase of the push tick (`AbstractMinecart.move(PISTON)` -> `applyEffectsFromBlocks` -> `DetectorRailBlock.entityInside`), so a piston next to the rail extends ONE tick after the pusher (an observer watching the pushed cell: two). A cart resting on a powered detector rail keeps re-checking every 20 gt; the judge counts that steady re-check as idle |
| `entity-rail-pass` | minecart + detector rail | (category entity) a cart rolling over a detector rail gives exactly 20 gt of power whatever its speed (the re-check scheduled when it arrived finds it gone), then stops against the wall at exactly the wall face - 0.49 |
| `entity-push-exact` | 2 pistons, minecart, armor stand | (category entity) a piston pushes an entity to the moving face + 0.01 + its half-width (`PistonMovingBlockEntity.moveCollidedEntities`, PUSH_OFFSET): a cart stated at x.5 goes to x+1.500000009536743 and back to x+0.499999990463257, then the same every cycle (the judge's 1e-6 position tolerance) |
| `entity-wall-stop` | minecarts | (category entity) carts sent at different speeds into a block stop at exactly the same spot (the move is clipped at the face, that motion zeroed); on plain rails a cart only slows by 0.96 per tick and never stops, on an unpowered powered rail it stops wherever its speed falls under 0.03 |
| `entity-plate-stand` | armor stand + stone pressure plate + 2 pistons | (category entity) an armor stand is two cells tall: pistons entering the cell ABOVE a plate shove it off and back without touching the plate (a block entering a rail's or plate's own cell pushes the rail or breaks the plate). Back on: the plate is on in the push tick's block-entity phase. Off: the plate lets go only at its re-check, 20 gt after it was pressed (10 for weighted plates) |
| `rb-dust` (no gadget yet) | redstone block on a moving structure + dust + observer | asymmetric EVEN delay the other way (pareto-mid): dust next to the cell a redstone block moves INTO lights when the block lands (m+2) and goes dark the moment it leaves (m, instant-off): observer pulse m+4 on the move out, m+2 on the move back. mp7-dustU (47, 14/13) |
| `shuttle` (no gadget yet) | lever + delay-1 repeater -> sticky piston -> redstone block -> repeater | a lever level whose falling edge comes 2 gt early (pareto-mid): the piston pushes the block next to the repeater at t2, it lands at t4; closing, it pulls it away at t2 and the repeater's input drops at once. Delay d after it: on at 4 + 2d, off at 2 + 2d. mp7-repU-pgF (50, 14/12), mp7-dustU-pgF (51, 14/11) |
* **Sculk hold (gadget `designs/gadgets/sculk-hold`, top-control):** one sculk sensor next to a
  piston holds it 30 gt after a lever click (weak power on every side, strong power into the
  block below). It wakes in the block-entity phase floor(distance) ticks after the click, so a
  piston whose source is the lever directly retracts at tick 1 before the sensor can hold it:
  cover the first ticks with a 4 gt repeater from the lever. The door's own late pistons move
  inside the 10 gt cooldown, so they do not re-trigger it (rb2s2 50, m51s 48). Doors using it
  rank in the `sculk` category.
* **Silent hold (gadget `designs/gadgets/silent-hold`):** a piston kept extended by a quasi
  source that later turns off WITHOUT updating it stays extended, unpowered, until any later
  neighbor update (a piston head forwards neighbor updates to its base, so a block landing next
  to the head counts). A repeater switching a solid block only updates that block's neighbors:
  feed the piston's quasi cell through such a block to drop the hold silently.
* **Slime drags:** blocks with PushReaction DESTROY (dust, repeaters, levers, torches) next to a
  moving slime block are left in place, not dragged or broken (only a DESTROY block in the
  movement path breaks); their floors are normal blocks and do get dragged (glazed terracotta
  never is).
* **Redstone block shuttle:** a sticky piston pushing a redstone block from next to piston A to
  next to piston B switches A off at once (the block becomes a moving piston that tick) and B
  on 2 gt later (when it lands): one actuator, ordered "A then B"; pulled back it gives "B off
  then A on".
* **Redstone block riding a structure (center-mech):** a redstone block carried by a slime
  structure powers a piston at rest and lets it go when the structure moves. It must touch the
  piston itself: in a quasi-only position its return never updates the piston, which then stays
  retracted. Two pushers can share one powered block between them (v2u2: both side structures
  pushed out from under the walkway off one obsidian block).
* **Sculk sensor (center-mech, gadget `sculk-hold`):** a lever flip is a vibration
  (BLOCK_ACTIVATE/DEACTIVATE); a sensor within 8 blocks goes ACTIVE after `floor(distance)` gt
  plus its block-entity tick (after the block-event phase), stays active 30 gt (power by
  distance, all directions, strongly into the block below it), then COOLDOWN 10 gt during which
  it ignores the door's own piston vibrations. One block = a 30 gt monostable on both lever
  edges; a piston that would see the lever's fall first needs a short bridge.
* **Dust does not pick up a block powered only by other dust** (wire signals are off while a
  wire computes its input): to tap a dust net one-way into another, put a repeater (or a block
  strongly powered by a repeater) between them.
## Gravity blocks (`FallingBlock`, `FallingBlockEntity`) — track gravity

* Sand/gravel/concrete powder schedule a tick 2 gt after being placed (pistons place them at the
  end of a move) or shape-updated; the tick makes them fall if the block below is air, fire,
  liquid or replaceable. A `moving_piston` below counts as support. Slime next to sand does not
  hold it up.
* A falling block that lands on a moving piston (or anywhere it cannot be placed) breaks into an
  item. `Sandbox.idle()` waits for falling blocks to land, so the judge sees that item.
* `door redsim-verify` on the 12 archived gravity doors: g7-g10 PASS; the other 8 (g1-67b,
  g1-69b, g3-60b, g4-51b, g6-49b, sf1-66b, sf1-68b, sf3-64b) fail only the "nothing in front of
  or behind the door" rule (2026-10-18 16:40Z: their lever port `1 0 -2` stands in front), which
  today's `door verify` applies too; their archived verdicts predate it. With
  `--legacy-walkway` (both judges skip that rule) all 8 PASS in redsim at 32 positions with the
  real judge's open/close ticks (`--judge`: 0 disagreements; clk-misc's diagnosis, 2026-10-20).
  So redsim's falling-block model agrees with the game on all 12.
* In redsim, falling blocks take their ids from the counter every entity shares and tick with the
  other entities in spawn order (`entity::tick_phase`); they do not run `applyEffectsFromBlocks`
  (plates, tripwires and bubble columns do not see them).
* Gadget `designs/gadgets/sand-drop-tower`: a 3-piston tower lowers/lifts a sand column 3 cells
  (the slime chain lets the bottom piston pull both upper pistons down). Used by door g4.
* An isolated redstone dust is a dot and powers nothing sideways; written as a cross
  (`redstone_wire[north=side,east=side,south=side,west=side]`) it stays a cross after the shape
  update and powers all four neighbors.
* **Pulse-driven pistons (logic-min, gadget `pulse-toggle`):** any door piston that only has to
  move its block once per lever edge can be retracted at rest and fired by a 2 gt observer pulse
  (pull on open, push + spit on close): v2's six pistons all work that way except P_T. The
  measured windows (`door ideal`): side/bottom pistons accept the pulse at any tick on both
  edges; A needs one pulse 0..4 gt after the flip plus the two B-edge pulses.
* **Observers on pistons fire twice:** a piston (or its head cell) changes state at the event,
  2 gt later and again 2 gt after that, so an observer watching it pulses twice per motion (a
  pulse-toggled column then moves out and back). Watch a block that moves once, the lever, or
  another observer instead.
* **OR of two repeaters in one dust:** a dust sandwiched between the output block of one
  repeater (above it) and of another (below it, its floor) is lit by either, and a cross dust
  powers the block it points into: a 2-input OR feeding a torch with 1 dust (logic-min P_T).
* **Sticky retract during a push finalizes, never pulls (seen in rb9):** if a sticky piston
  retracts while the block two in front is still a moving piston it pushed (any progress, as long
  as it has not landed), that block is placed at its destination at once and the piston retracts
  alone. rb9's B pushes A down at t4 and retracts at t6: A stays down, the dance goes on.
* **Early window (gadget `designs/gadgets/early-window`):** a delay-1 repeater reading a block
  fed by an observer pulse and by a delay-2 repeater from a SECOND observer gives a window at
  t+4..6 and t+8..12 (a 2gt hole: a repeater's turn-off tick outranks another's turn-on tick),
  vs t+10..18 for the plain pulse stretcher.
* **Symmetric door control (fresh-c fc1/fc2):** with an A-on-B elevator, T dragged out by the
  right structure and one shuttled redstone block (A off then P_R on / P_R off then A on), no
  hold is needed if (1) A gets one pulse BEFORE the shuttle moves, on both edges (open: A is
  still powered, the pulse is swallowed; close: A spits C down one cell, freeing T's cell),
  (2) the shuttle piston is the lever through one repeater (>= 2 gt after that pulse), and
  (3) the B window comes at the same delay on both edges: on close A is extended again and
  B's push fails (gadget `blocked-push`). Close = spit C, T back, A pushes T and C home.
* **Pulse-driven shuttle breaks the drag:** a 2gt pulse on the shuttle piston spits its redstone
  block, which is then placed in the block-event phase; the piston it powers fires in that same
  phase, before T (lifted 2 ticks earlier) has landed, so T is not dragged. Keep it steady.
* **Pull-type pulse piston:** a sticky piston two cells from a structure (one-cell gap) pulls it
  on the first 2 gt pulse and pushes+spits it back on the second, so a side column can be
  pulled open from OUTSIDE its backbone (fc2's P_L at (-3,2,0): 3 backbone slimes, no pusher
  under the walkway).
* **Calibrated sculk sensor (sculk-2, gadget `sculk-delay-line`):** `CalibratedSculkSensorBlock`
  is active 10 gt (not 30), hears within 16 blocks, outputs on every side except its back, and a
  non-zero signal into its back makes it hear only that frequency (lever on / piston extend =
  10, lever off / piston retract = 9). With nothing behind it, it is a one-block wireless delay
  line: activation floor(d) gt after each flip, d up to 16. Echo rule for every sensor: once it
  is INACTIVE again (wake + 20 calibrated, + 40 normal) any piston move within its radius wakes
  it, so whatever it drives must stop moving before then, or the re-run must be a no-op.

## Water and bubble columns (`FlowingFluid`, `WaterFluid`, `LiquidBlock`, `BubbleColumnBlock`) — redsim M1

Modeled in redsim (`crates/redsim/src/fluid/`, track clockinfra), checked cell by cell against the
game by `redsim-fuzz fuzz --gen basin --palette stone:6,water:10,...` (random circuits in a stone
basin: 6000 cases, 0 mismatches, 2026-10-19).

* Fluid ticks have their own list (`fluidTicks`), keyed by the fluid TYPE (source `water` or
  `flowing_water`: a tick scheduled for flowing water does nothing once the cell is a source),
  run right after the block ticks of the same game tick; the sub-tick counter is shared.
* Water's tick delay is 5 gt. A flowing cell recomputes its level from its 4 sides (highest
  minus 1; 2+ adjacent sources over a solid block or a source = a new source; water above =
  falling level 8), then spreads: down first if it can (then sideways too only with 3+ source
  neighbors), else sideways toward the nearest hole within 4 cells (`getSlopeDistance`; ties
  spread to all of them, applied north, south, west, east). Side walls are collision shapes:
  water passes a face unless the two shapes' touching faces together cover it
  (`Shapes.mergedFaceOccludes`).
* Water replaces air and the `#washed_away_by_fluids` blocks (dust, repeaters, comparators,
  torches, levers, buttons, skulls, rails, tripwire...), which DROP as items; waterloggable
  blocks would take only a source (not modeled yet: a model note). Pistons destroy water (no drop).
* A full source above soul sand (up) or magma (down) schedules a 20 gt block tick that turns the
  water column above into a bubble column (flags 2, cell by cell up the sources); a column whose
  support goes away turns back into water 5 gt after its shape update.

## TNT and dead coral fans (`TntBlock`, `BaseCoralFanBlock`, `PistonBaseBlock.moveBlocks`) — redsim (track tnt)

Modeled in `crates/redsim/src/tnt.rs` (study 9, TNT dupers: designs/circuits/tnt/VARIATIONS.md),
checked by `redsim-fuzz fuzz --palette core,piston:6,simple:6,tnt:4,coral:4` (12000 cases, 0
mismatches) and the exact trace of the real-game dupe tnt-c1.

* A TNT primes on placement (a different old block) or any neighbor update while it has a neighbor
  signal, then is removed (`removeBlock`, flags 3). It is not a conductor. In redsim a prime is a
  record (`World::primed`: tick, cell), not an entity (the `PrimedTnt`'s kick draws from
  `level.random`); a primed TNT reaching its explosion tick is an UNMODELED note unless the caller
  removes primed TNT first (the TNT judge does).
* Dead coral fans are pushable (slime drags them), have no collision and drop nothing; a floor fan
  pops on a DOWN shape update without a sturdy top below, a wall fan on the update from its back.
* The dupe: `moveBlocks` copies every pushed state first and blanks the sources last. A fan
  riding the structure pops when the cell under it turns `moving_piston` (the flag-324 write
  updates shapes before the block entity exists) and updates a TNT still at its source; if the
  TNT is powered then, it primes while its moving copy lands. A TNT can never rest next to power,
  so the power must appear inside the push: a dust whose connections change (it computes them on
  the fly) is the source every duper found so far uses.

## Drops and the bridge's seeds (`Block.popResource`, `ItemEntity`) — shulker study

A piston breaking a shulker box (push reaction DESTROY) drops it with its contents through
`Block.popResource(level, pos, stack)` (Block.java 409): the item entity's position is the cell
center plus `Mth.nextDouble(level.random, -0.25, 0.25)` on each axis (x, y, z in that order; y
minus half the item's height, 0.125). Those three draws come from the level random, which the
bridge seeds per sandbox (`Sandbox.create`, `clear`). But the 5-argument
`new ItemEntity(level, x, y, z, stack)` (ItemEntity.java 62) then draws from the ENTITY's own
random: the yaw (`nextFloat() * 360`, in the type constructor) and the drift
(`setDeltaMovement(nextDouble() * 0.2 - 0.1, 0.2, nextDouble() * 0.2 - 0.1)`). Vanilla seeds that
random with `RandomSupport.generateUniqueSeed()` (a static uniquifier ^ `System.nanoTime()`), so
the same build in the same seeded sandbox drifts differently on every run.

**What the bridge does differently from vanilla**: an `ItemEntity` that enters a sandbox during
its tick (`Sandbox.tick`) through `Block.popResource(Level, BlockPos, ItemStack)` (seen on the
call stack; `popResourceFromFace`, dispensers and summoned design entities are left alone) gets
its own random reseeded to `sandbox seed ^ (0x6d63746173 + n)`, n = the n-th such drop of the
sandbox (from 0, reset with the sandbox), and the constructor's draws redone in the same order
(yaw, then the x and z drift), before it is tracked or ticked (`LevelCallback.onCreated`, the
level entity manager's callbacks wrapped). The same distribution, now one sample per seed:
judges that depend on drops vary the seed (the shulker judge seeds each build from its position
and runs 64 more seeds after the builds). Two runs with the same seed give the same exact trace;
traces without such drops are unchanged.

**Entity ids**: every entity that enters a sandbox (summoned, spawned by the game, falling
blocks, primed TNT, drops) gets the id `base + n` (`Entity.setId` in `onCreated`, before it is
tracked), n counting from 0 in the order they enter (reset with the sandbox), base =
`0x40000000 + ((sandbox id & 1023) << 20)`, a multiple of 4. Vanilla takes ids from one
server-wide counter, so `(tickCount + getId()) % 4` (an item entity at rest moves 1 tick in 4)
depended on everything the JVM had spawned before; now it depends on n only, ids grow with the
entity tick list's order, and redsim's ids (from 0, falling blocks sharing the counter) are the
same numbers: the protocol reports n as `net_id`, and `redsim-fuzz trace-compare` checks it.

## Tick exactness: the bridge's seeds for mobs, sensors and terrain — track tickexact

Every random a mob, spawn, weather or fire path draws from is seeded by the bridge from the
sandbox seed `s` (bridge `Exact.java`, `Sandbox.java`, `ExactVillageSiege.java`, `Terrain.java`),
and redsim draws the same values from the same streams (the stream registry: `redsim::rng`,
`World::construct`, `World::entering_random`). `mctas mobfarm compare <design> [--terrain S]`
runs a design in both and compares every tick (below). The seeds (`GOLDEN` = 0x9E3779B97F4A7C15,
`mix` = `RandomSupport.mixStafford13`):

| stream | seed | redsim |
|---|---|---|
| the level random | `LegacyRandomSource(s)` (`randValue` = `s ^ 0x72616E64`, the loot sequences salted `s ^ 0x6C6F6F74`) | `World::mirror_level_random` |
| an entity's own random at entering, n = the n-th entity to enter the sandbox (its id - base) | `mix((s ^ "entity") + (n+1) GOLDEN)`, then the constructor's draws replayed (below) | `World::entering_random`, `rng::entity_seed` |
| an entity's own random from its construction until it enters (every `EntityType.create`, k = the k-th construction, rejected spawn attempts included) | `mix((s ^ "constr") + (k+1) GOLDEN)` | `World::construct`, `rng::construct_seed` |
| a brain's shuffling lists (`GateBehavior`, `RunOne`, `TriggerGate` closures) | `mix(S_n + javaHash(key) GOLDEN)`, key `"<generation>/<activity>/<priority>/<index>[/<k>]"`, `"#j"` for the j-th list of a closure; the generation +1 per `refreshBrain` | `rng::shuffle_seed` |
| a raid (`Raid.java` 119) | `mix((s ^ "raid") + id GOLDEN)` | `rng::raid_seed` |
| the wandering trader spawner | `mix((s ^ "trader") + GOLDEN)` | `rng::trader_seed` |
| sounds' seeds, the AFK player's random | `s ^ "sound"`, `s ^ "player"` | (no draw redsim needs) |

The constructor's draws, replayed from `S_n` in order: the UUID (2 `nextLong`); an `ItemEntity`'s
`bobOffs`, yaw (`nextFloat() * 360`) and its drift (2 `nextDouble`: kept by `spawnAtLocation`,
drawn and replaced by `throwItem` / `Containers.dropItemStack`, absent in the other
constructors); a `LightningBolt`'s seed (`nextLong`) and flashes (`nextInt(3) + 1`); a
`LivingEntity`'s yaw (`nextFloat() * 2 pi`) and each brain sensor's start delay (`nextInt(scanRate)`
per sensor in its order; a summoned villager's brain is built twice: the constructor's, then
`load`'s); the leaf classes' draws (`Chicken.eggTime`, `Rabbit`'s idle timeout, `Squid`'s and
`Guardian`'s animation, `CopperGolem`'s cooldown); a schooling fish's goals (Mob's constructor runs
`registerGoals`: `FollowFlockLeaderGoal`'s first `nextStartTick`, `reducedTickDelay(200 +
nextInt(200) % 20)`, after the yaw). A spawn path that snaps the yaw without the
head (`NaturalSpawner`'s `snapTo`, the siege) leaves the replayed yaw in the head, and a cube
mob's move control keeps `180 * yaw / pi` as its direction until a goal sets one;
`EntityType.create(level, .., pos, ..)` sets the head and the body to its snap yaw. What a mob
draws from its own random before it enters (a zombie's `finalizeSpawn` -> `handleAttributes`, a
salmon's variant) comes from its construction seed; vanilla's siege builds its zombie with
`new Zombie(level)`, so the bridge runs `ExactVillageSiege` (vanilla's code plus that seeding).
A mob's projectile (a skeleton's arrow, a drowned's trident) is constructed and shot from an
unseeded random before it enters: the bridge reseeds it as entity n and redoes the shot from
that stream toward the shooter's target (an arrow's `setBaseDamageFromMob` triangle, then
`shoot`'s three), as redsim's `entity::arrow` launches it.

**A world sandbox** (`world` op; one per worker: 26.3 keeps the clock, the weather and the game
rules server-wide) runs the server clock and weather, in a biome (`biome`: the real-game gate's
plains void; a terrain sample's sandbox reports its world's sea level, 63, the flat source's -63
elsewhere), the five custom spawners in vanilla's order (`custom_spawners`: phantoms, patrols,
cats, the siege, the trader), and a real `ServerPlayer` with no network (`player`: creative,
flying, simulation and view distance 6; its chunks waited on until the ticking, spawning and
entity-ticking sets are stable; its `doTick` after the level's, so `TIME_SINCE_REST` counts as in
the game). Natural spawning and thunder see that player. **A terrain sandbox** (`terrain` op)
puts a `.mcterrain` sample in the way a saved chunk loads: each section's states, the saved light
(`queueSectionData`), the biome cells, the primed heightmaps, the POIs found in the blocks
(free: no worldgen villager is loaded, the sample's entities stay out on both sides), the block
entities' saved data, each chunk's inhabited time and the structure starts with their pieces
(held by the first loaded chunk their box overlaps); the import is checked cell for cell.

**Design mobs** come from their NBT with no `finalizeSpawn`: their yaw is the NBT's `Rotation`
or 0, their fire 0, their UUID `nameUUIDFromBytes("mctas-mob-" + n)` (they take no design index;
non-mob design entities keep `0x6d63746173 + i`), the follow-range bonus a harness input (one
`triangle(0, 0.11485)` per mob from `LegacyRandom(s ^ "bonus")`, applied as `base + base * bonus`),
HOME and LAST_SLEPT set like redsim's `Run::arrive` (the bed's ticket taken).

**The compare** (`mctas mobfarm compare`, `redsim::exact`): every tick the level random's state,
`randValue`, the day time, the weather, every block change in the farm box, every entity (pos,
motion, yaw, pitch, head, body, health, hurt time, fire, air, pose, vehicle, its own random's
state, every memory with its expiry, running goals and behaviors, path, target) and the
entities that entered or left; the first difference with context (how many draws a random is
off), the first per category, the game's draw log of a tick by call site with every raw output
(`--draws-at`), the spawning chunks in `tickChunks`' order, and `--twin` (the game against
itself: the bridge's own determinism). `--terrain S` builds the design at its site in a sample;
`--bare` drops the player and natural spawning; `--scan TYPE FROM TO` finds seeds where a custom
spawner fires.

## Mob farms (villagers, iron golems, zombies) — redsim mob layer (track ironfarm)

Modeled in `crates/redsim/src/entity/mob/` (the brief [farm-iron.md](../designs/briefs/farm-iron.md)
cites the 26.3 source line by line; the game checks are in the
[ironfarm notes](../designs/circuits/mobfarm/tracks/ironfarm/NOTES.md#game-validation-the-briefs-one-off-checks)).

* A villager summons a golem when it panics (a hostile in its `NEAREST_HOSTILE`: a zombie within
  8 that it sees from its eyes), has slept within the last day (`LAST_SLEPT`), sees no golem
  (`GolemSensor`, every 200 gt, from the `NEAREST_LIVING_ENTITIES` list the living sensor refreshes
  every 20 gt within the follow range, 16 plus the spawn bonus) and 3 villagers in range agree; the
  group then waits 600 gt. The summon tries 10 spots in +-8 x +-6 x +-8 (`SpawnUtil`, legacy iron
  golem strategy: a solid block with air or a liquid above, not glass, leaves, ice...; the golem's
  3 cells must be valid empty spawn blocks, the block under it sturdy).
* A golem a villager sees during its scan delays the next summon: every golem in range costs
  rate. Farms that reach 90%+ of the ceiling drop each golem out of every villager's follow box
  within about 50 gt of its spawn.
* At night villagers REST: one hide of 20+ gt (the zombie out of sight) lets them fall asleep, and
  a zombie in view from the bed wakes them scared. The dusk pulse (two inverted daylight detectors,
  one through 6 repeaters) gives that hide every night without a golem.
* Water pushes a mob at 0.014 per tick along the normalized flow (a golem drifts 0.056 b/t); a mob
  touching water falls at gravity / 16, so a golem sinks slowly until its whole 2.7-block body is
  out of the water. A mob jumping in lava rises (`jumpInLiquid`): a lava blade needs a ceiling.
* Out of the water a walking golem uses land speed (0.25 x friction factor) and steps up 1 block
  onto a floor still under its box: at an edge where floor and water end together, a golem that
  walks against the current stays on the edge.
* Flowing water enters no waterloggable block (signs, trapdoors, slabs: `SimpleWaterloggedBlock`
  takes only a source) and no block outside `#washed_away_by_fluids` (open fence gates: no fluid,
  no lava either, and no collision box); two adjacent sources make every flowing cell beside both a
  new source (a pad fed from its edges turns into still water).

## A villager riding a minecart: every way it stops riding or dies — track tickexact

The census the iron farm's riderless rule rests on (2026-11-11): every caller of `stopRiding`,
`removeVehicle`, `ejectPassengers`, `unRide` and `removePassenger` in the whole decompiled
`net.minecraft` (26.3), classified for a villager in a minecart. "redsim" says whether the
simulator models it.

**A. It leaves the cart.**

| # | path (26.3) | trigger | the farm holds | redsim |
|---|---|---|---|---|
| 1 | `Minecart.activateMinecart(state=true)` → `ejectPassengers` | the cart on a powered activator rail | activators only where meant (the siding's drop) | modelled |
| 2 | the vehicle removed: `Entity.baseTick` (`isPassenger && vehicle.isRemoved` → `stopRiding`), `ServerLevel.tickPassenger`, `tickNonPassenger` | the cart destroyed (C) | C's conditions | modelled |
| 3 | the rider removed: `Entity.setRemoved` → `stopRiding` when the reason destroys | death (removed after the 20-tick death animation), discard | B's conditions; farm villagers are persistent | modelled |
| 4 | the rider teleported: `Entity.teleport` → `stopRiding` unless `asPassenger` | chorus fruit (`TeleportRandomlyConsumeEffect`), commands, pearls | villagers never teleport themselves | n/a |
| 5 | the cart through a portal: `Entity.teleportCrossDimension` ejects, then re-mounts its passengers in the new level | a cart entering a nether portal | no portal on the track | cart portals modelled; the re-mount not bridged |
| 6 | boarding another vehicle: `Entity.startRiding` stops riding first | — | never: cart and boat pickups skip passengers | modelled |
| 7 | sleeping: `LivingEntity.startSleeping` → `stopRiding` | SleepInBed | SleepInBed refuses passengers (`checkExtraStartConditions`): only after a drop | modelled |
| 8 | a lead: `Leashable.setLeashedTo` → `stopRiding` | leashing | no leads | n/a |
| 9 | a spear's stab: `LivingEntity.stabAttack(..., dismounts)` (KineticWeapon, PiercingWeapon) | a zombie or zombified piglin holding a spear (SpearUseGoal) reaching the rider; natural zombies hold an iron spear in 1/6 of the 5% weapon chance (Hard) | no natural mob reaches a rider; the design's zombies hold nothing | modelled |
| 10 | under water: `LivingEntity.baseTick`, only a vehicle in `#dismounts_underwater` (camel, chicken, donkey, happy_ghast, horse, llama, mule, pig, ravager, spider, strider, trader_llama, zombie_horse) | — | a minecart is not in it: its rider never gets off, it drowns (B) | modelled |
| 11 | a conversion: `ConversionType.SINGLE` (`from.stopRiding(); to.startRiding(vehicle)`) | villager → witch (`Villager.thunderHit`), villager → zombie villager (killed by a zombie, Normal/Hard) | the cart stays taken, by a hostile: no bolt or zombie reaches a rider | modelled |
| 12 | growing up in a boat (`AgeableMob.ageBoundaryReached`) | boats only | — | n/a |
| 13 | chunk save and reload | none: passengers are saved with their root vehicle and restored on load; UNLOADED_TO_CHUNK does not destroy | — | modelled (the reload scenes) |

The other callers never act on a villager in a cart: RideCommand, ServerPlayer, PlayerList, Player,
DismountOrSkipMounting and PiglinAi (piglins, hoglins), ForceUnmount (the warden dismounts
itself), BeehiveBlockEntity, Display, Shulker, RunAroundLikeCrazyGoal (horses), AbstractBoat's
ejects, HappyGhast's and Cushion's `removePassenger`.

**B. It dies** (removed by A3; the cart empty 20 ticks later). Riding protects from nothing:
mob melee (an attack box meeting the rider's), projectiles, fire (1 every 20 fire ticks) and lava,
suffocation (`isInWall`: the eye in a suffocating block, 1 a tick), drowning (the eye under water:
air, then 2 a second), explosions, lightning (converts rather than kills: A11), cactus, berry
bushes, magma, powder snow, the void. Falls: `Entity.causeFallDamage` hands the cart's fall to its
passengers, but a cart's fall distance resets every tick on rails
(`OldMinecartBehavior.moveAlongTrack`), so only an off-rail drop counts, and a villager takes
floor(d - 3): a drop of 3 or less is safe (R1's 2-block jump is). redsim models each of them for
mobs; the cart-to-rider fall hand-on is not bridged. The farm holds: no reachable mob, no fire,
lava or water at a rider, no suffocating block at a rider's head, off-rail drops of 3 or less,
lightning rods.

**C. The cart is destroyed.** `VehicleEntity.hurtServer` adds 10 per point of damage and destroys
the cart past 40; the damage decays 1 a tick (`AbstractMinecart.tick`). Lava (`lavaHurt`, 4 a
tick) destroys it on its second tick in lava; a cactus (1 a contact tick) after about 5; lightning
(5: 50) at once; explosions, projectiles and players as they hit. Fire alone does not (+10 every 20
ticks against 20 of decay). redsim models explosions and lightning; a cart in lava or in a cactus's
cell is refused by name.

Bridged beside it (0 differences): a ridden cart frozen with its box in a fence gate's, the gate
shut and opened again during the freeze, moves on at the thaw (scene gate-frozen), unless its box
touches a chunk still unloaded then (it stopped in the gate's cell).

## Weather, lightning, fire and sunburn — redsim (track mobinfra)

Modeled in `crates/redsim/src/weather/` and `entity/mob/sun.rs` (the judge's settings: Hard,
`spawn_mobs`, an AFK player near the farm, a mature world), read in the 26.3 source:

* **The cycle** (`ServerLevel.advanceWeatherCycle`, every level every tick): while
  `clearWeatherTime` counts down, both flags stay off and both timers sit at 1; else each of the
  thunder and rain timers counts down and flips its flag at 0, then draws again: thunder on for
  `THUNDER_DURATION` 3600..15600, off for `THUNDER_DELAY` 12000..180000; rain on for
  `RAIN_DURATION` 12000..24000, off for `RAIN_DELAY` 12000..180000 (uniform, inclusive). Raining
  15.8% of the time, the thunder flag 9.1%, thundering (both) about 1.4%. The levels move 0.01 a
  tick toward their flags (thunder toward its flag only while it rains); `isRaining` is a rain
  level over 0.2, `isThundering` a thunder level (times the rain level) over 0.9. `weather rain|
  thunder|clear <t>` sets the timers like `setWeatherParameters`.
* **Darkening** (`WeatherAttributes`: rain and thunder layers on the sky light level, alpha
  blended toward 4 by 0.3125 x rain and 0.52734375 x thunder): the sky darken at noon is 0 clear,
  3 in rain, 5 in thunder; at night 11 whatever the weather. Daylight detectors read
  `sky light - darken`, so a dusk detector fires early in rain: an inverted detector's dusk pulse
  can come in the afternoon and miss the night's rest (the L2d farms in a forced rain day: 0-36
  golems/h in the game).
* **Rain at a block** (`Level.isRainingAt`): raining, the block sees the sky (sky light 15), no
  motion-blocking block above it (the heightmap), a biome with rain (plains). Under glass: sky
  light 15 but no rain. A mob is in rain when its feet block or its head height is
  (`Entity.isInRain`); rain or water puts a burning mob out (`clearFire`), so a golem leaving a
  lava blade in rain stops burning.
* **Lightning** (`ServerLevel.tickThunder`, only for chunks within 128 blocks of a non-spectator
  player): raining, thundering and `nextInt(100000) == 0` for the chunk this tick (about 0.24
  strikes a chunk a thunder day). The strike aims (`findLightningTargetAround`) at the closest
  lightning rod within 128 that tops its column (`WORLD_SURFACE - 1`), striking above it; else at
  a random living entity that sees the sky in the column's box (from the motion-blocking top up,
  inflated by 3); else at the column's top. No bolt when it does not rain at that block (a
  villager under glass draws every strike around it and gets none). On Hard, a strike not above a
  rod is a skeleton trap horse with chance `effective difficulty x 1%` (visual bolt). The bolt
  (`LightningBolt`: life 2, 1-3 flashes) hits every living entity in x+-3, y-3..+9, z+-3 every
  tick its life is not negative, through any roof: a villager becomes a witch, other mobs get 5
  damage and 8 s of fire, a boat or minecart is destroyed (50 > 40), a drop is destroyed (5
  health); on Normal and Hard it sets fires at its block and around it (`spawnFire`).
  `lightning::static_check` lists every bolt that can fall on a build (a lightning rod topping its
  column within 128 blocks takes every strike within 128 of it); the judge measures what they cost
  on the card, never refuses on them (vjeux 2026-11-08: "It's all about tradeoffs").
* **Lava's fire** (`LavaFluid.randomTick`, only within `fire_spread_radius_around_player` 128 of a
  player, `ServerLevel.canSpreadFireAround`; lava ticks twice per random pick: `LiquidBlock
  .randomTick`, then the fluid state's): `nextInt(3)` steps up with a random x/z jitter of +-1 each,
  lighting the first air cell beside an `ignitedByLava` block (planks, logs, fences, fence gates,
  wool, beds, signs, wooden trapdoors and doors...) and stopping at a `#blocks_lava_fire_spread`
  block; with 0 steps, 3 tries beside the lava at its level, lighting the air above an
  `ignitedByLava` block. Then `FireBlock` (ticks every 30-39 gt) burns its neighbours by their burn
  odds (fence gates and planks 20, leaves and wool 60) and spreads by their ignite odds. With a
  player near, iron-middle's oak fence gates holding lava burned first at gt 735 +- 494 in the
  game (8 servers, median 500) and at 770 +- 70 in redsim (200 runs, median 663:
  `mctas mobfarm fire-burn`).
* **Sunburn** (`Mob.isSunBurnTick`, zombies): in the day (`MONSTERS_BURN`, day time 23460..12542),
  brightness over 0.5 (raw light 12 and up: `getLightLevelDependentMagicValue` gives 0.50000006
  at 12), `nextFloat() * 30 < (brightness - 0.4) * 2`, not in water, rain or powder snow, the
  eye's block seeing the sky; a helmet takes the burn instead. A farm's zombie must be shaded,
  in water, or under a block.

## 12. redsim: the exact model (track redmodel)

`crates/redsim` is a pure-Rust, dependency-free tick-level copy of the 26.3 code paths above,
shared by the search tools (`door redsim-verify`, `cosearch`, `fullmech`, satwire's screen). It
replays every rule of sections 1-7 exactly; this section says where each rule lives and what the
differential tests found. Evidence: `redsim-fuzz fuzz` compares every block state after every tick
with the bridge sandbox (0/100,000 random circuits with `--palette all`, 0/20,000 container
circuits with `--gen containers`), pistmodel's `door pistfuzz` does the same for pistons (0/100,000,
plus 328/328 archived doors and 72/72 gadgets), `door redsim-verify --judge` gives the judge's
verdict on all 263 supported archived designs, and every regression found is a fixture in
`crates/redsim/tests/fixtures/` (`cargo test --release -p redsim --tests`). Owners: redmodel
(engine, redstone, containers, classes), pistmodel (`piston/`, `gravity.rs`).

**Engine** (`world.rs`, `ticks.rs`, `pos.rs`):

| rule | game source | redsim |
|---|---|---|
| `setBlock` side effects: block entity swap, `onPlace`/removal, flag 1 neighbor updates, shape updates with `flags & -34` (changes made by shape updates notify no neighbors), flag 16/512 cuts | `Level.setBlock` 216, `LevelChunk.setBlockState` 267 | `World::set_block_limit` (fixture shape-update-no-neighbor-updates) |
| neighbor updates W,E,D,U,N,S through a FIFO that runs nested updates after the current one | `CollectingNeighborUpdater.addAndRun` 68, `runUpdates` 87 | `World::add_and_run`, `Nu::{Full, Multi, Shape, Simple}` |
| scheduled ticks: per-chunk queues merged by (trigger, priority, sub-tick); collected first, then run; `willTickThisTick` | `LevelTicks.tick` 81, `collectTicks` 93, `runCollectedTicks` 182 | `ticks.rs` |
| block events FIFO with dedup, events added meanwhile run in the same phase | `ServerLevel.runBlockEvents` 1267 | `World::tick` |
| tick phases: time+1, block ticks, block events, entities (falling blocks), block entities (hoppers, then moving pistons) | `ServerLevel.tick` 358-460 | `World::tick` |
| comparator notification of a changed container or analog source: horizontal comparators, and the comparator behind one conductor | `Level.updateNeighbourForOutputSignal` 1015 | `World::update_neighbour_for_output_signal` |
| dust update order = `HashSet<BlockPos>` order, `hash = (y + 31z) * 31 + x`, bucket `(h ^ h >>> 16) & 15` | `DefaultRedstoneWireEvaluator.updatePowerStrength` 18 | `pos::java_hashset_order` |
| judge placement: raw writes (flags 2\|16\|32\|256\|512), a shape update of every wire, a `self` update of every block | `video.rs::place` | `World::place_like_judge` (`place_raw` + `place_updates`) |

**Components** (`behavior.rs` dispatches by block; one module per component family):

| component | game source | redsim | rules the fuzzer pinned down |
|---|---|---|---|
| dust | `RedstoneWireBlock.getConnectionState` 117, `updatePowerStrength` 264, `getSignal` 361; `DefaultRedstoneWireEvaluator.calculateTargetStrength` 40 | `redstone/wire.rs` | dots stay dots, placed crosses stay crosses; step connections cut by a conductor above / beside; wire signals off while computing (no dust-to-dust through blocks) |
| repeater | `DiodeBlock.tick` 50, `checkTickOnNeighbor` 84, `getInputSignal` 109, `shouldPrioritize` 196; `RepeaterBlock.isLocked` 84 | `redstone/diode.rs` | priorities -3/-2/-1 (section 5), locking read in `updateShape`, pulse stretching |
| comparator | `ComparatorBlock.calculateOutputSignal` 64, `getInputSignal` 90 (analog through one conductor), `checkTickOnNeighbor` 140, `refreshOutputState` 151 | `redstone/diode.rs` | compare mode updates its front on every tick it runs, even with an unchanged output (a BUD source) |
| torch, wall torch | `RedstoneTorchBlock.tick` 61, `neighborChanged` 83, `isToggledTooFrequently` 126 | `redstone/attach.rs` (`torch`) | burnout list per position (8 toggles in 60 gt -> 160 gt) |
| lever, buttons | `LeverBlock.pull` 82, `updateNeighbours` 133; `ButtonBlock.press` 94, `tick` 133 | `redstone/attach.rs` | stone 20 gt, wood 30 gt; the bridge's `use` calls `press` even on a pressed button: the pending release tick is kept (dedup) |
| observer | `ObserverBlock.updateShape` 57, `tick` 45, `startSignal` 74 | `redstone/blocks.rs` (`observer`) | fires on shape updates only, not while powered or with a tick pending |
| lamp | `RedstoneLampBlock.neighborChanged` 29, `tick` 45 | `blocks.rs` (`lamp`) | on in the powering update (flags 2), off by a 4 gt NORMAL tick from the first unpowered update, never rescheduled |
| waxed copper bulb | `CopperBulbBlock.checkAndFlip` 42, `getAnalogOutputSignal` 66 | `blocks.rs` (`bulb`) | toggles on the rising edge seen by a neighbor update; comparator 15/0 |
| note block | `NoteBlock.neighborChanged` 81, `setInstrument` 47, `triggerEvent` 141 | `blocks.rs` (`note`) | instrument from the block below, or the mob head above (a wall skull is a harp: its state has no `instrument` override; fixture note-under-wall-skull) |
| target | `TargetBlock` (signal source, power 0 unhit) | `blocks.rs` (`target`) | dust always connects |
| trapdoors, doors, fence gates | `TrapDoorBlock.neighborChanged` 116, `DoorBlock.updateShape` 78 / `neighborChanged` 215, `FenceGateBlock.neighborChanged` 172 | `blocks.rs` (`indicator`) | a door half copies the other half's state in `updateShape`; oak use = flags 10 / 2 |
| powered, activator rails | `PoweredRailBlock.findPoweredRailSignal` 23, `updateState` 121; `RailState` | `blocks.rs` (`rail`) | 8-rail search; `RailState` reshapes neighbors on place (found by the fuzzer) |
| dropper, dispenser, crafter | `DispenserBlock.neighborChanged` 121, `tick` 135; `CrafterBlock.neighborChanged` 67, `tick` 84 | `blocks.rs` (`dispenser`) | QC (power at the cell above), 4 gt trigger, `triggered` cleared with flags 2 |
| containers (items) | `DropperBlock.dispenseFrom` 41; `HopperBlockEntity.pushItemsTick` 97, `tryMoveItems` 106, `ejectItems` 143, `suckInItems` 218, `tryMoveInItem` 314, `removeItem` 74; `AbstractContainerMenu.getRedstoneSignalFromContainer` | `redstone/container.rs` | a hopper's `removeItem`/`setItem` do NOT notify comparators, a dropper's do (fixture hopper-remove-no-setchanged); wasEmpty cooldown 8, or 7 when the sender ticked no later; fill level in floats: `floor(14 f) + (f > 0)`; T flip-flop gadget: fixture hopper-t-flip-flop |
| hopper (lock) | `HopperBlock.checkPoweredState` 118 | `blocks.rs` (`hopper`) | `enabled` follows `!hasNeighborSignal`, flags 2 |
| leaves | `LeavesBlock.updateShape` 86, `tick` 76, `updateDistance` 108 | `blocks.rs` (`leaves`) | distance from logs (a 1 gt element) |
| scaffolding | `ScaffoldingBlock.onPlace`, `updateShape`, `tick`, `getDistance`, `isBottom`, `getCollisionShape` (`SHAPE_STABLE` with the empty context); `FallingBlockEntity.fall` | `blocks.rs` (`scaffolding`) | onPlace has no same-block check: each of its own changes re-schedules it; only the top face is sturdy; falling at 7 (entity: `gravity.rs`) |
| analog blocks | `CakeBlock.canSurvive` 123 / `getAnalogOutputSignal` 133, `ComposterBlock.tick` 218 / 231, end portal frame, respawn anchor | `behavior.rs` | a cake needs a SOLID block below (it pops off repeaters: fixture cake-on-unsupported-repeater) |
| plain blocks | `Blocks.java` properties (conductor, collision, support shape, instrument, push reaction) | `block.rs` (`SIMPLES`), `behavior.rs` | slime conducts and is a floor; honey and leaves conduct nothing and hold nothing (fixtures slime-is-a-conductor, leaves-honey-hold-nothing) |
| pistons, moving blocks, gravity | section 2; `PistonBaseBlock.checkIfExtend` 93, `getNeighborSignal` 116, `triggerEvent` 139, `isPushable` 215, `moveBlocks` 259; `PistonMovingBlockEntity.finalTick` 262, `tick` 294 | `piston/`, `gravity.rs` (pistmodel) | a retracting base keeps its extended box as support; a head popped by `updateShape` drops nothing |

**Block equivalence classes** (`redsim::classes`, `redsim-fuzz classes`; list in
`designs/redsim/CLASSES.txt`): two block states are interchangeable when every property the model
reads agrees (conductor; `isFaceSturdy` per face for FULL, CENTER and RIGID support; `isSolid`;
collision; push reaction; slime/honey; block entity; unbreakable; drops; analog output; leaf
distance source; own behavior). 70 plain states fall in 22 classes, 11 if note-block instruments
are ignored (they matter only directly above/below a note block): stone-like conductors, obsidian
(immovable), glass-like non-conductors, slime, honey, pumpkin (popped by pistons), glazed
terracotta (push-only), logs (leaf source), soul sand/mud (14 px collision), bottom slab, top slab.
`redsim-fuzz classfuzz` proves them in the game: 3000 random circuits against the same circuit with
every plain block replaced by a random member of its class, 0 differ modulo the classes. Blocks
with behavior of their own are each their own class (with their in-plane facings): dust, repeater
x 4 delays, comparator x 2 modes, torch, wall torch, lever, buttons, observer, lamp, bulbs, note
block, target, trapdoors, doors, gates, rails, dispenser, dropper, crafter, hopper, leaves,
scaffolding (another 1 gt element: a non-conductor whose top face alone is sturdy), cake,
composter, end portal frame, respawn anchor, skulls, pistons, gravity blocks.

**Out of scope** (a model note is recorded, never a silent difference): random ticks, weather and
time, entities and players (pressure plates, tripwire, item entities: a dropper spitting into the
world, hoppers catching drops), a dropper holding 2+ stacks (random slot), dispensers' item
behaviors, sculk, fluids and waterlogging, fence-gate `use` (needs the player's facing), unwaxed
copper. Unknown blocks are rejected (`Block::parse`), so a screen answers Unsupported instead of
guessing.

**Link table** (`redsim::links`, `redsim-fuzz links [--verify]`): what each component does to a
pulse train, measured in redsim and replayed in the game (`--verify`: 112/112 identical). Input: a
floor lever toggled between ticks (a pulse k = on before tick 1, off k ticks later); cells: the
component's output windows, first tick on - first tick off ("1.." = stays on). Pulse COUNTS
change along a chain: an observer merges changes inside its 4 gt busy window (one pulse for a
1-2 gt input pulse), an observer watching a level (lamp, note block) emits two pulses per window,
a lamp merges pulses closer than its 4 gt fall, a comparator swallows a 1 gt pulse, repeaters
stretch to their delay and merge gaps shorter than it, and a copper bulb (directly, or fed by an
observer) halves the count: 2 input pulses = 1 window, an odd link from an even source.

| component | pulse 1 | pulse 2 | pulse 4 | pulse 8 | 2x pulse 2, gap 1 | 2x pulse 2, gap 2 | 2x pulse 2, gap 4 | 2x pulse 2, gap 8 |
|---|---|---|---|---|---|---|---|---|
| repeater d1 | 2-4 | 2-4 | 2-6 | 2-10 | 2-7 | 2-4 6-8 | 2-4 8-10 | 2-4 12-14 |
| repeater d2 | 4-8 | 4-8 | 4-8 | 4-12 | 4-9 | 4-8 | 4-12 | 4-8 14-18 |
| repeater d4 | 8-16 | 8-16 | 8-16 | 8-16 | 8-16 | 8-16 | 8-16 | 8-16 |
| comparator | - | 2-4 | 2-6 | 2-10 | 2-7 | 2-4 6-8 | 2-4 8-10 | 2-4 12-14 |
| torch (inverts) | 0.. | 0-2 4.. | 0-2 6.. | 0-2 10.. | 0-2 7.. | 0-2 4-6 8.. | 0-2 4-8 10.. | 0-2 4-12 14.. |
| lamp | 1-5 | 1-6 | 1-8 | 1-12 | 1-6 | 1-10 | 1-6 7-12 | 1-6 11-16 |
| note block | 1-2 | 1-3 | 1-5 | 1-9 | 1-3 4-6 | 1-3 5-7 | 1-3 7-9 | 1-3 11-13 |
| observer on the lever | 2-4 | 2-4 | 2-4 6-8 | 2-4 10-12 | 2-4 7-9 | 2-4 6-8 | 2-4 8-10 | 2-4 12-14 |
| observer on a lamp | 2-4 7-9 | 2-4 8-10 | 2-4 10-12 | 2-4 14-16 | 2-4 8-10 | 2-4 12-14 | 2-4 8-10 14-16 | 2-4 8-10 12-14 18-20 |
| observer on a note block | 2-4 | 2-4 | 2-4 6-8 | 2-4 10-12 | 2-4 7-9 | 2-4 6-8 | 2-4 8-10 | 2-4 12-14 |
| copper bulb | 1.. | 1.. | 1.. | 1.. | 1-4 | 1-5 | 1-7 | 1-11 |
| observer -> bulb (T flip-flop) | 2.. | 2.. | 2-6 | 2-10 | 2-7 | 2-6 | 2-8 | 2-12 |
| scaffold (trapdoor floor) | 1-2 | 1-3 | 1-5 | 1-9 | 1-3 4-6 | 1-3 5-7 | 1-3 7-9 | 1-3 11-13 |
| scaffold tower x3 | - | 3-4 | 3-7 | 3-11 | 3-8 | 3-4 7-8 | 3-4 9-10 | 3-4 13-14 |

(The torch row starts lit: "0-2 4.." = lit, off from tick 2 to 4, lit again. The note block's
`powered` follows the lever with no delay, so the observer watching it behaves like one watching the
lever. Scaffold rows: a top iron trapdoor opened by the lever is the floor of the first scaffold,
whose neighbor tower stands on stone; on = distance > 0. A lever edge between ticks t and t+1 shows
at t+1 for a 1 gt element too, so the single scaffold looks like the note block; the tower shows
the 1 gt per block (3 blocks: edges at t+3) and swallows the 1 gt pulse: its first scaffold is back
to distance 0 before the second reads it.) A pulse-count planner (`door topology`) should take its
link rules from this table (or call `links::response` with its own trains) rather than assume every
change is one pulse.

**Harness notes:** the bridge's `set`/`fill` used to hand ONE `MutableBlockPos` to every
`setBlock` of a request, and a block event queued by an `onPlace` (a piston placed with updates)
kept that object, so it ran at the next block's cell; fixed on main (4a4fd3e), and the fuzzer places
update-mode blocks one per request anyway.
