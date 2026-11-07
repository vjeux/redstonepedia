# Queued-tick and block-event identity confusion (corruption audit, lane 2 of 5)

2026-11-07, gate G1 (docs/process.md), adapted: this is an audit, not a site component. It asks
where Minecraft Java 26.3 queues work for one block/state and later runs it against a different
block, facing, position, or block entity — or drops/duplicates it in a controllable way — and
which of those a survival player can reach. The known positive is the bedrock breaker's piston
event (`designs/briefs/bedrock-breaker.md`). Work log and evidence: `designs/corrupt-events/NOTES.md`.
No internet, no community designs: everything below is read in the decompiled 26.3 source
(`mctas src show <Class>`; line numbers are that listing's) and, for the finalists, measured in
the bridge and on the official server. Chunk persistence is a separate lane; it is only touched
where an event mechanism itself stores or loses work (§3.6).

## Use

A catalogue, with verdicts a builder can trust: every queued-work mechanism in 26.3, what it
captures at queue time, what it re-reads at execution time, the exact window in which a player or
a machine can swap the target, and whether the result does anything a player wants (remove a
block no tool can, fire a component that was never powered, store or kill a pending action).

## The question in one line per queue

For each queue: **is the identity check at execution weaker than the action's assumptions?**
A queue entry is *identity-confusable* when (a) execution checks only block **type** at a
**position**, (b) the action then uses the **current** state (facing, delay, inventory …), and
(c) a survival player or machine can change the state at that position between queue and
execution. The piston event is the extreme case: the check passes for any piston, and the action
uses the new piston's facing to delete a block unconditionally.

## 1. The four queues, from the game code

### 1.1 Scheduled block and fluid ticks (`LevelTicks`, `LevelChunkTicks`, `ScheduledTick`)

- An entry is `ScheduledTick(type, pos, triggerTick, priority, subTickOrder)`
  (`ScheduledTick` 8). **No block state is captured.** `triggerTick = gameTime + delay`
  (`LevelAccessor.createTick` 31–38); `subTickOrder` is a per-level monotone counter.
- Ordering: `(triggerTick, priority, subTickOrder)` (`DRAIN_ORDER` 9–17), priorities
  `EXTREMELY_HIGH..EXTREMELY_LOW` (`TickPriority` 5–12). Per tick, due entries are **collected
  first, then run** (`LevelTicks.tick` 81–91, `collectTicks` 93, `runCollectedTicks` 182–192):
  a tick scheduled during the run phase fires next tick at the earliest. Cap: 65,536 per tick
  for blocks and 65,536 for fluids (`ServerLevel.tick` 394–396); leftovers stay queued and
  fire late (§3.6).
- **Dedup key is `(pos, type)` only** — delay and priority are ignored
  (`UNIQUE_TICK_HASH` 22–34; `LevelChunkTicks.schedule` 53–57 silently drops a second entry
  while one is pending). The entry leaves the dedup set only when collected (`poll` 43–50)
  or by `clearArea` (structure/fill code, `LevelTicks` 236–252).
- **Nothing cancels a pending tick when its block is broken, replaced, or moved.** There is no
  removal call in the set-block path at all (checked `LevelTicks`, `LevelChunkTicks`,
  `LevelChunk.setBlockState`).
- Execution re-reads the world and checks type only:
  `ServerLevel.tickBlock` 826–831 `if (state.is(type)) state.tick(...)`;
  `tickFluid` 818–824 `if (fluidState.is(type)) fluidState.tick(...)`. For fluids the type is
  the exact fluid (`WATER` ≠ `FLOWING_WATER`).
- Persistence: entries are saved per chunk as `SavedTick(type, pos, remainingDelay, priority)`
  (`SavedTick` 13; `LevelChunkTicks.pack` 93–107) and re-armed on load with negative sub-tick
  orders, so saved ticks run before fresh ones at equal trigger/priority (`unpack` 109–119).

### 1.2 Block events (`ServerLevel.blockEvents`)

- An entry is `BlockEventData(pos, block, paramA, paramB)` (a record: equality on all four).
  The queue is an `ObjectLinkedOpenHashSet` (`ServerLevel` 215): **an identical pending event
  is dropped**; FIFO `removeFirst` until empty, so events queued during the phase run in the
  same phase (`runBlockEvents` 1267–1292). `blockEvent(pos, block, b0, b1)` is 1262–1265.
- Execution checks type only, then runs the **current** state's handler with the **captured**
  params: `doBlockEvent` 1294–1297 `state.is(eventData.block()) ? state.triggerEvent(this, pos, paramA, paramB)`.
- Windows: events queued in the scheduled-tick or block-event phases run the same tick; events
  queued in the entity/block-entity phases or by player packets run in the **next** tick's
  block-event phase — that inter-tick gap is the player's window (phases: `ServerLevel.tick`
  358–461; player packets are handled after the levels in `MinecraftServer.tickChildren`
  1096–1115, i.e. between two level ticks).
- Persistence: **none.** The queue is a plain field. One storage quirk: an event whose position
  is not in block-ticking range at drain time is kept in `blockEventsToReschedule` and retried
  every tick (1286–1291) — it survives chunk unload for the server's lifetime, and dies with
  the server (§3.6).
- The complete set of executors (`triggerEvent` overrides; grep over the block package):
  `PistonBaseBlock` (the only world-writing one), `NoteBlock` (sound from current
  `INSTRUMENT`/`NOTE`, 141–168), `ComparatorBlock` (delegates to its BE, which ignores it),
  `PotentSulfurBlock` (stamps `BE.eruptionTick`, 130–136), `BaseEntityBlock` → BE handlers:
  bell (swing direction from captured `paramB`, `BellBlockEntity` 43–54), chest / ender chest /
  shulker box (lid `openCount = paramB`), decorated pot (wobble), spawner and end gateway
  (cooldown/particles). Every caller queues for its own block (12 call sites).

### 1.3 Block-entity tickers (`LevelChunk.BoundTickingBlockEntity`)

- A ticker is bound to a **block-entity instance**, created from the state at bind time
  (`createTicker` 721–723). Every `setBlockState` that keeps the BE re-sets its state and
  **rebinds** the ticker (`LevelChunk` 342–344 → `updateBlockEntityTicker` 699–723); a
  block-type change removes the BE (306–315) unless `shouldChangedStateKeepBlockEntity` —
  overridden only by copper chests and copper golem statues, whose oxidation variants share
  one BE type by design. A BE whose type no longer matches the state is removed (331–334).
- Each tick re-checks `blockEntity.getType().isValid(currentState)` and passes the **current**
  state to the ticker (736–767). State-dependent ticker choice exists
  (`PotentSulfurBlock.getTicker` switches on `STATE`, 139–163) and is safe *because* of the
  rebind. There is no queue and no survival path that changes a block's state without going
  through `LevelChunk.setBlockState`. **Verdict for the whole family: not confusable** (§3.5).

### 1.4 Captured-state carriers (not queues, but the same shape)

- `PistonMovingBlockEntity` captures `movedState, direction, extending, isSourcePiston` at
  move start (persisted, 330–338). Landing places
  `Block.updateFromNeighbourShapes(movedState, level, pos)` unconditionally (294–316) — the
  cell is guaranteed to still be `moving_piston` (checked 303), which a player can neither
  break nor build into. Early abort `finalTick` (262–283): source pieces become air, foreign
  blocks land now. `updateFromNeighbourShapes` passes the real level as tick access
  (`Block` 195–205), so shape side effects (observer `startSignal`, falling-block schedules)
  really fire on landing (§3.4).
- `FallingBlockEntity` captures its `blockState` at spawn (82–92) — the entity *is* the block
  in transit; landing is guarded (`canBeReplaced` + `canSurvive` + not `moving_piston`,
  187–201). The scheduled-tick handoff that spawns it reads the current state
  (`FallingBlock.tick` 45–50), which is the only confusable seam (§3.3).
- `RedstoneTorchBlock.RECENT_TOGGLES` is a per-level list of `(pos, when)`: burnout history is
  **position-keyed**, inherited by any torch placed there (61–87). By design; §3.3.

## 2. What a swap costs (the window arithmetic)

A swap = break the block, place another of the **same type** (the execution check), with the
state the attacker wants, before execution. Placement state comes from the look
(piston/observer: nearest looking direction's opposite, all 6 facings; repeater/comparator:
horizontal). Break time at the survival maximum (netherite pickaxe 9 + Efficiency V 26 = 35,
Haste II ×1.4 = 49; progress/tick = speed / hardness / 30, `bedrock-breaker.md`):

| block | hardness | mining ticks | pending delay | windows after queue | swap? |
|---|---|---|---|---|---|
| piston (block event) | 1.5 | 1 | ≤1 tick | 1 (BE-phase queue) | yes — the breaker |
| repeater / comparator | 0 | 1 (instant) | 2–8 (delay state) / 2 | 1–7 | yes, easily |
| observer | 3.0 | 2 | 2 | 2 (machine-caused observation) | exactly fits, §3.2 |
| dispenser / dropper / crafter | 3.5 | 3 | 4 | 3 | fits, but the BE is recreated empty |
| sand &c. (falling) | 0.5 | 1 | 2 | 1 | yes, but same-type only |
| pointed dripstone | 1.5 | 1 | 1–2 | 1 | yes, but the cascade heals it |

Machines swap differently: a piston cannot break a block (except `DESTROY`-reaction
components, which pop), but it can **push a replacement in**: the pushed block lands in the
block-entity phase, in time for next tick's phase 3 / block events. TNT can remove the
original. Both are slower to aim than the hand swap and never beat it below.

## 3. Candidates, ranked

Verdicts: **LIVE** = reproduced (bridge; vanilla where marked), **SOURCE** = proven by the code
above, reproduction scripted where cheap, **DEAD** = disproven by source (the reason is the
catalogue entry). Full per-candidate sequences and run outputs: `designs/corrupt-events/NOTES.md`.

### 3.1 The piston event family (extends the known positive)

`PistonBaseBlock.triggerEvent` (139–213) reads `direction` from the **current** state (140),
re-checks power against the new facing (143–151), and — a second, opposite choice — builds the
returning base from the **captured** direction `b1` (175). Queue-side, `checkIfExtend`
(93–114) queues an extend only if the push resolves *then* (97), but `moveBlocks` re-resolves
at execution, so there is no check/use gap for pushes; the retract's `removeBlock` (199, 205)
has **no guard at execution at all**. `this.isSticky` is the current block's, which the type
check pins to the event's block — sticky↔normal swaps fail `state.is(block)` (DEAD).

| # | candidate | captured vs re-read | verdict |
|---|---|---|---|
| P1 | Retract-delete on swapped facing (the bedrock breaker) | re-read facing; unconditional `removeBlock(front)` | **LIVE** (breaker track) |
| P2 | P1 against other survival-unbreakable blocks: **end portal frame, reinforced deepslate**, trial spawner, vault; obsidian class | same | **LIVE**: bridge 3/3 + 3/3 (+obsidian 2/2); official server, survival bot: frame w1–2, deepslate w1–2 (w0 fail is a harness settle artifact — NOTES §1) |
| P3 | **Sticky** swap piston: type-1 retract deletes the front block whenever the block two ahead is not pullable (192–199) | same; two-ahead state read live | **LIVE** on bridge |
| P4 | Retract-delete aimed at an extended piston's base: the orphan head collapses in the same update (`PistonHeadBlock.updateShape` 93–96) | — | **LIVE** (clean delete, no headless by this route) |
| P5 | Retract-delete aimed at a `moving_piston` cell: `preRemoveSideEffects → finalTick` lands a foreign block instead of deleting it; a source piece vanishes | BE fields captured | SOURCE (not scripted) |
| P6 | Extend on swapped facing: pushes along the new facing; push re-resolved and power re-checked, so no guard is bypassed | re-read facing | **LIVE** on bridge (gadget-grade) |
| P7 | Sticky pull (type 1) on swapped facing: pulls the new two-ahead block, resolver-guarded | re-read facing | **LIVE** on bridge (gadget-grade) |
| P8 | Retract `finalTick`s whatever moving piece is in front of the *new* facing (164–166) — force-lands another piston's in-flight block (or vanishes its base mid-retract: source → air) | target re-read | SOURCE |
| P9 | The landed base faces the **captured** `b1`, not the swapped facing (175) — the self-repair the breaker already exploits | captured | **LIVE** (breaker track) |
| P10 | Event storage: an armed retract whose chunk leaves block-ticking range before the next drain is rescheduled in memory indefinitely (1286–1291); a server stop drops it, leaving the piston extended-and-unpowered (its state *is* saved) | queue not persisted | SOURCE (restart half needs a server-restart run to be LIVE) |
| P11 | Dedup collapse of two identical piston events in one tick | full-record equality | SOURCE, no use found (only the piston queues its own events) |

Usefulness: P2 is the new deliverable — end portal frames and reinforced deepslate are
otherwise unremovable in survival, and the same one-key press that breaks bedrock removes
them (the target only has to be the block in front of the swapped piston; no drop, the block
is deleted, its BE — trial spawner, vault — is removed by the set-block path; cross-ref the
block-entity lane for inventory side effects). P3 frees machine design (sticky swap pistons).
P6/P7/P8 are timing gadgets: they act in a direction the wiring never powered, but every
guard still runs.

### 3.2 Scheduled-tick transplants (the action fires on the swapped-in block)

The scheduled tick falls into three kinds, over all 55 `tick` handlers in the block package:

- **Recompute-from-live-world** (swap-safe): survival checks (`canSurvive` re-scanned:
  cactus, sugar cane, bamboo, chorus plant/flower, growing plants, big dripleaf stem,
  hanging moss, frogspawn, farmland), coral death (`scanForWater` live), leaves distance,
  scaffolding distance, bubble columns, cauldron drip (finds the stalactite live),
  comparator (`calculateOutputSignal` live — the model citizen), plates/buttons/tripwire
  press checks, lamp/torch signal checks, fluids (`getNewLiquid` from live neighbours,
  `FlowingFluid` 428–442). Swapping these is indistinguishable from placing the block.
- **State-machine advance on the current state** (transplantable): **diode (repeater),
  observer**, big dripleaf tilt, sculk sensor phase, dried ghast hydration, sniffer egg
  crack, eyeblossom, frosted ice ageing, composter level, fire ageing.
- **BE-delegating** (identity = the BE instance at execution; swap recreates it empty):
  dispenser/dropper and crafter (4 gt dispense), chest/barrel/ender-chest `recheckOpen`,
  sculk shrieker `tryRespond` (also gated on the current `SHRIEKING` state, which a placed
  shrieker never has), command/test blocks (not survival).

| # | candidate | why it works / fails | verdict |
|---|---|---|---|
| T1 | **Repeater pulse transplant**: trigger a delay-4 repeater (8 gt pending), break it (instant) and place a repeater facing any horizontal direction. `DiodeBlock.tick` 50–63 turns an OFF repeater ON *unconditionally* and only then, if its (new) input is empty, schedules the turn-off after `getDelay(current state)`. The placed repeater's own 1 gt placement schedule is dedup-dropped (158–162 vs §1.1). | A pulse leaves a repeater that never saw an input, in a player-chosen direction, with a player-chosen length. | **LIVE** (bridge + vanilla probe, NOTES) |
| T2 | **Observer pulse transplant**: `ObserverBlock.tick` 45–54 toggles `POWERED` on the current state and pulses out of the current facing's back. Window: observation caused *during* a tick (machine) → trigger +2 gt; the two inter-tick windows fit the 2-tick mine exactly (progress ticks run in packet handling, after the levels: `tickChildren` 1096–1115) with break+place in the second window. Player-caused observation between ticks leaves only one window — impossible. | Same free pulse, 90°/vertical redirections included, but needs the machine-caused timeline. | **LIVE** on bridge (ideal swap); survival timeline SOURCE (held mine not expressible by the vanilla bot) |
| T3 | Comparator transplant | fully recomputed live (§3.2) | DEAD (by construction) |
| T4 | Dispenser/crafter transplant (dispense along new facing) | BE recreated empty on break; nothing to dispense; `tick` doesn't even re-check `TRIGGERED` | DEAD |
| T5 | Torch/lamp/button/plate/lectern/target/lightning-rod/tripwire-hook transplants | live re-checks or unconditional clear of a state the fresh block lacks | DEAD |
| T6 | Big dripleaf / sculk sensor phase / eyeblossom | placed states can't carry the mid-machine states (`TILT`, `PHASE`, summon flags) | DEAD |
| T7 | Long timers inherited by a fresh block: dried ghast (+1 hydration, 5000 gt stages), sniffer egg crack stage, coral death (60–99 gt, live water scan) | saves at most one stage of waiting the player could have had by leaving the first block; frogspawn/frosted ice are not survival-obtainable as items | DEAD (no gain) |
| T8 | Redstone-torch burnout inherited by position (`RECENT_TOGGLES`, 160 gt retry) | position-keyed by design | DEAD (documented behaviour) |
| T9 | Fire inherits an orphan tick (relight within 30–39 gt) | tick re-scans flammability live; earlier first check only | DEAD |

### 3.3 Falling blocks and dripstone (tick → entity handoff)

`FallingBlock.tick` spawns the entity from the **current** state — but the check is same-type,
and within a type the states a player can place are the states they'd get anyway (anvil
facing; concrete-powder colours and anvil damage levels are *different blocks*). The 2 gt
window fits a hand swap; there is nothing to gain. Scaffolding's tick recomputes distance
live; its fall-vs-destroy branch reads the *stored* `DISTANCE`, which placement computes
live too. Speleothem column-fall suppression (replace the scheduled block within 1–2 gt)
heals itself: the replacement's removal/shape cascade re-schedules the column below, and a
sturdy replacement legitimately supports it. **All DEAD**, with the handoff documented as the
seam to re-check if a future fallable gains a dangerous placed-state.

### 3.4 Position confusion by piston movement (orphans and duplicates)

A pending tick is keyed to `(type, pos)`; pistons move blocks, not ticks. The moved block
lands two ticks later and re-schedules what it needs via `onPlace`/`updateFromNeighbourShapes`
(§1.4); the orphan at the old position fires on whatever is there then.

- **Moved observer: pulse preserved, not lost** — landing runs the observer's `updateShape`
  against every neighbour through the real tick access, so `startSignal` fires at the new
  position (the flying-machine atom). Duplication by an observer placed at the *old* position
  depends on how it moved (bridge-measured, NOTES §1/§3): a **push can never duplicate** —
  push duration and observer delay are both 2 gt and scheduled ticks run before block
  events, so the head is a `moving_piston` in the old cell at the orphan's trigger tick in
  every push geometry; a **pull frees the old cell a phase-pattern earlier** and the orphan
  fires on the placed observer: **one observation, two pulses** (the second observer is
  placed by hand, so the gain is a delayed pulse at a cell the player controls —
  gadget-grade). Proven for the pull, with a no-placement control.
- Pushed falling blocks / leaves: orphan hits air or `moving_piston` → dropped; landing
  re-schedules. Clean.
- `DESTROY`-reaction components (repeaters, torches, dust) pop instead of moving: no orphan
  beyond the break case (§3.2).

### 3.5 Block-entity tickers, containers, comparators

§1.3: rebind-on-state-change + per-tick validity + BE removal on type change. Hopper
cooldowns, furnace progress, sculk listeners (the pending vibration lives in the BE and
dies with it), spawner/trial-spawner state — all instance-bound, all reset by a break.
Comparator output is recomputed live into its BE (§3.2). The copper statues/chests keep
their BE across oxidation variants on purpose, with the same rebind. **Family DEAD**;
`PotentSulfurBlock` is the audited-safe pattern for state-dependent tickers.

### 3.6 Window-widening and persistence cross-cuts

- **Tick overflow**: >65,536 due block (or fluid) ticks in one tick slips the rest by whole
  ticks, widening any swap window by the slip. Sustaining it needs >65k newly-due ticks per
  tick; a transient few-tick slip from a huge fluid release is the realistic bound. No
  candidate needs it (T1/T2 fit unaided). Catalogue only.
- **Saved ticks across unload** keep their *remaining* delay (§1.1): returning to a chunk
  does not grant a longer window than staying did.
- **Block events across unload** are the one true storage (P10): rescheduled in memory while
  the server runs, lost on stop. Intersects the chunk-persistence lane only here.

### 3.7 Dedup suppression pairs

Method: every `scheduleTick` call site for blocks (≈90 in the block package, fluid
waterlogging aside) grouped by `(type)`: a suppression needs one block scheduled from two
paths with materially different delay/priority that a player can order. Found: diode
placement (1 gt, NORMAL) vs neighbour schedule (delay, HIGH..EXTREMELY_HIGH) — within 1 gt
the placement tick wins, which is exactly T1's enabler; comparator HIGH/NORMAL by
`shouldPrioritize` (same 2 gt delay — ordering only); fire self-reschedule-first (T9);
observer's own `hasScheduledTick` guard makes it dedup-proof by construction. **No useful
suppression pair beyond T1.** Block-event dedup: P11.

## Decision (what gets built and measured)

No new judge, kind, or metric. Verification reuses the bedrock track's harness
(`mctas bedrock scan|watch|vanilla`, scenario JSONs under `designs/corrupt-events/scenarios/`):

1. Generalise the scenario target from bedrock-only to any placed block (one optional
   `target_block`, default bedrock; `crates/bedrock`): P2 (×3 targets), P3, P4, P6/P7
   (target cell vacated along the new facing) on the bridge; P2 end-portal-frame and
   reinforced-deepslate on the official server with the survival bot.
2. T1/T2 on the bridge (`watch` with scripted swap, observer/lamp states as readout) with
   controls. No T1/T2 vanilla-bot run: T1's survival input (instant break + rotated place
   over up to 7 windows) is strictly easier than the 1-window press P2 proves with real
   packets, and T2's held mine across two windows is not expressible in the scenario
   format (its timeline is source-proven, §3.2).
3. §3.4 moved-observer duplicate on the bridge (`watch`): push (impossible) and pull
   (duplicates), each with a no-placement control.

The obvious-design bar for this audit is the existing bedrock machine: anything new must
either remove a block it cannot, or fire a component it cannot, with a survival input a
player can actually make (the one-key press; §2 arithmetic). By that bar the keepers are
**P2, P3, T1**, plus T2 and the pull-duplicate as mechanism-proven gadgets. Everything else
is catalogued with its disproof so the next lane doesn't re-derive it.

## Must-haves

- Every LIVE claim: a scenario file + the bridge output in NOTES; P2 also an official-server
  run with survival inputs only. (Amended during verification: T1 needs no separate
  packet-level run — its input is strictly easier than P2's proven one-window press — and
  T2's held mine cannot be expressed by the bot; both are recorded as such, not silently
  dropped.)
- Every DEAD claim: the source lines that kill it (in NOTES' catalogue table).
- Survival legality per candidate: obtainable items, placements a look can give, mining
  timelines from §2 — no debug stick, no commands in the measured sequence.

## Not goals

- Chunk persistence, save/load block-entity corruption, update suppression by chained-update
  limits (other lanes' questions; touched only at §3.6/P10).
- Random ticks (chosen live, never queued), entity AI schedules, scheduled *functions*
  (`/schedule`, command-only).
- Client-side event effects (note particles, lid animation) beyond noting they re-read state.
