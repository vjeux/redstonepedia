# NOTES — queued-tick / block-event identity confusion (lane 2)

2026-11-07. Brief: `designs/briefs/corrupt-events.md`. Clone
`~/agents/corrupt-events/minecraft-tas`, branch `explore/corrupt-events`, base `35bcee3c7`
(origin/main). Method: full source read of the four queue systems in the decompiled 26.3 tree
(`mctas src show`), enumeration of every executor (55 block `tick` handlers, 12 `blockEvent`
call sites, all `triggerEvent` overrides, ≈90 `scheduleTick` sites), then bridge scenarios
(`mctas bedrock scan|watch`) and official-server runs (`mctas bedrock vanilla`, survival bot)
for the finalists. All scenario files live in `designs/corrupt-events/scenarios/`.

## 1. Measured results (bridge = `bedrock scan|watch`; vanilla = official server, survival bot)

| candidate | scenario | command | result |
|---|---|---|---|
| P2 end portal frame | p2-end-portal-frame.json | scan 0–2 | baseline Fail; **3/3 broken** |
| P2 reinforced deepslate | p2-reinforced-deepslate.json | scan 0–2 | baseline Fail; **3/3 broken** |
| P2 obsidian (control target) | p2-obsidian.json | scan 0–1 | baseline Fail; **2/2 broken** |
| P2 end portal frame | p2-end-portal-frame.json | vanilla 0,1,2 | w0 **fail**, w1 **broken**, w2 **broken** |
| P2 reinforced deepslate | p2-reinforced-deepslate.json | vanilla 1,2 | **2/2 broken** |
| P3 sticky swap piston deletes bedrock | p3-sticky-delete.json | scan 0–2 | baseline Fail; **3/3 broken** |
| P4 delete vs piston base (base+head gone) | p4-delete-piston-base.json | scan 0–2 | baseline Fail; **3/3 broken** |
| P6 extend on swapped facing | p6-extend-swap.json | scan 0 | baseline Fail; **1/1 broken**, cell left `piston[extended=true,facing=east]` |
| P7 sticky pull on swapped facing | p7-pull-swap.json | scan 0 | baseline Fail; **1/1 broken** |
| T1 repeater control | t1-control.json | watch 0–14 | north lamp lit from window 8, east dark (direction + 8 gt confirmed) |
| T1 repeater transplant | t1-repeater-transplant.json | watch 0–14 | B (delay 1, rotated) `powered=true` windows 8–9, **east lamp lit 8–13**, north never lit; B's input empty throughout |
| T2 observer transplant (ideal swap) | t2-observer-transplant.json | watch 0–8 | B `powered=true` windows 2–3, **west lamp lit**, south never lit |
| dup via PUSH (first attempt) | old dup-pushed-observer + dup-control-no-c.json | watch 0–12 | **no duplicate**: C's placement hits the in-flight head; both lamps lit identically with and without C — artifact identified (§3) |
| dup via PULL | dup-pushed-observer.json | watch 0–10 | **two pulses, one observation**: C `powered` windows 2–3 (orphan @ old cell), A `powered` windows 5–6 (new cell) |
| dup pull control | dup-pull-control.json | watch 0–10 | one pulse only (A, windows 5–6); old cell stays air |

The vanilla w0 failure for the portal frame is a harness settle artifact, not a mechanism
difference: the bridge passes w0, and w1/w2 pass on both. The vanilla runner spends its first
ticks placing/teleporting the bot (`place_bot` arrival) under tick freeze/step, so the chord
in "window 0" does not meet the same machine state as the bridge's window 0; the survival
packets themselves (insta-break torch + piston, place) are identical, and they break the
frame in windows 1 and 2. Recorded as-is: **frame 2/3, deepslate 2/2 on the official server**.

Exact player sequences (P2/P3, survival): piston (or sticky piston, P3 — with an unpullable
block two beyond the target) sitting extended on the target, redstone torch beside it on a
block. One inter-tick window, three packets: break torch (queues the retract), break piston,
place piston facing the target. Needs the insta-break loadout of the bedrock brief (Haste II,
Efficiency V netherite pickaxe, speed 49 ≥ 1.5×30) or the armed-machine variant from the
bedrock track (any arming machine works unchanged — the target block is irrelevant to the
machine). Robustness: works in every window whose retract is armed (bridge 3/3 across
consecutive windows for the torch variant; direction/position dependence is the bedrock
track's, already mapped there). The deleted block drops nothing; a block entity on it
(spawner, trial spawner, vault) is removed by the ordinary set-block path.

T1 exact sequence: power a delay-4 repeater (8 gt pending). Any time in the next 7 windows:
break it (hardness 0, instant, any tool) and place a repeater rotated to the wanted output
direction, delay set by clicks (each click is its own packet; the pending tick is unaffected
— dedup swallows the placement's 1 gt schedule). At the original trigger tick the new
repeater turns on for its own delay with no input. T2 is the same shape for observers but the
mine takes 2 ticks (hardness 3.0 at speed 49): it fits only when the observation was caused
*during* a tick (machine), leaving exactly two windows — mine held across both, break
completing in the second window's packet handling (`MinecraftServer.tickChildren`: levels
1096–1110 before connection 1113, so packet-phase break+place lands before the next tick's
scheduled-tick phase). Not attempted with the vanilla bot (its `break` is a one-window
action; a held mine across windows is not expressible in the scenario format).

## 2. Source-proven, not separately reproduced

- **P5** retract-delete vs a `moving_piston` cell lands the in-flight foreign block instead
  of deleting it (`PistonMovingBlockEntity.preRemoveSideEffects` 286–288 → `finalTick`
  262–283, run from `LevelChunk.setBlockState` 306–315 before BE removal). Source-only.
- **P8** a swapped retract `finalTick`s an unrelated moving piece in front of the new facing
  (`PistonBaseBlock` 164–166); a retracting base so hit vanishes (source piece → air).
  Source-only.
- **P9** landed base faces the captured `b1` (`PistonBaseBlock` 175) — visible in every
  successful P2 run's aftermath on the bedrock track; the mechanism behind self-repair.
- **P10** block events are memory-only: rescheduled while their chunk is out of
  block-ticking range (`ServerLevel` 1286–1291), lost on server stop (no save path exists).
  The unload-storage half follows from 1286–1291 alone; the restart half (piston left
  `EXTENDED`+unpowered, since states *are* saved) was not run (needs a server-restart
  scenario the harness doesn't have).
- **P11 / §3.7 dedup**: block-event equality is the full record (`BlockEventData`);
  scheduled-tick dedup is `(pos,type)` (`UNIQUE_TICK_HASH`). Multi-site schedulers were
  enumerated; the only useful collision is the diode's placement tick enabling T1.
- **Overflow slip** past 65,536 due ticks/tick (`ServerLevel` 394–396) widens windows in
  principle; not built (no candidate needs it).

## 3. The one false positive, and what it taught

First dup attempt (push geometry, kept as `dup-control-no-c.json`): the "old position" lamp
lit in lockstep with the pushed observer's own lamp, with **and without** a second observer
placed at the old cell. Two separate facts: (a) C's placement at window 1 hit the pusher's
in-flight `moving_piston` head and did nothing; (b) the lamp was reading power *through* the
adjacent lamp: `SignalGetter.getSignal` (65–69) relays `getDirectSignalTo` through any
redstone conductor, and a lamp is a conductor, so lamp₂ (west) sees the observer's direct
signal into lamp₁ (east). Consequences recorded for the next lane: never read a pulse from a
lamp adjacent to another lamp (or any conductor in the direct-signal path); read the
component's own state. And the push geometry is principled-dead for orphans: push duration
and observer delay are both 2 gt and scheduled ticks (phase 3) run before block events
(phase 5), so the old cell is a `moving_piston` at the orphan's trigger tick in every push
arrangement; a **pull** frees the old cell one phase-pattern earlier and duplicates.

## 4. Dead ends, with their disproofs (the catalogue)

All line numbers are the 26.3 decompiled listing.

**Queues themselves.** Scheduled ticks capture no state and are never cancelled on block
change — but execution requires `state.is(type)` (`ServerLevel` 818–831) and every handler
below either recomputes from the live world or advances a state the swap can't forge.
Fluids are the safe extreme: `FlowingFluid.tick` (428–442) recomputes the new fluid state
from live neighbours; source/flowing are distinct tick types. Block-entity tickers are
instance-bound, rebound on every state change (`LevelChunk` 342–344, 699–723), validity
re-checked per tick (736–767), BE removed on type change (306–315; copper chest/statue
oxidation keep theirs by design, same rebind). No survival state change bypasses
`LevelChunk.setBlockState`.

**Block events.** Only the piston writes to the world. Note block re-reads `INSTRUMENT`/
`NOTE` (sound only, 141–168). Bell applies the *captured* swing direction to its BE
(resonance re-scans entities live). Chest/ender-chest/shulker lids take `openCount` from the
captured param (animation/collision of the lid only; the BE is recreated by any swap, and
events drain the same tick unless queued after the event phase — no window exists for these
because their queuers are the BE tick and player opens, both examined). Potent sulfur stamps
`eruptionTick = gameTime` on its BE (130–136). Spawner/gateway events are cooldown/particle
bookkeeping. Comparator's event delegates to a BE that ignores it (179–183). Sticky↔normal
piston swap fails the type check. Extend/pull swaps (P6/P7) are real but every guard
(resolver, power re-check vs the *new* facing, `PistonBaseBlock` 143–156) still runs —
gadgets, not corruption.

**Scheduled ticks — recompute-live (swap ≡ place).** Comparator (`calculateOutputSignal`
live, 64–76, 151–171); survival checks (`state.canSurvive` re-scanned: cactus 40–44, sugar
cane 36–40, bamboo 101–105, chorus plant 76–80 / flower 36–40, growing plants 54–58,
dripleaf stem 91–95, hanging moss 82–86, frogspawn 78–84 then hatches — frogspawn is not
survival-obtainable); coral death (live water scan; species are distinct types anyway);
leaves distance (76–78; decay is the *random* tick, not this one); scaffolding distance
(110–122 live; the fall-vs-destroy branch reads stored `DISTANCE`, which placement also
computes live); bubble column / liquid→column (live scans); cauldron drip (re-finds the
stalactite, 88–96); farmland (re-checks; dirt *path* is the unconditional exception, 61–63 —
removing the cover within 1 gt doesn't save the path, noted, no use); buttons/plates/
detector rail/tripwire (live entity/press scans); lamp/torch (live signal); torch burnout
is position-keyed on purpose (`RECENT_TOGGLES`, 160 gt retry); fire (live flammability;
fire states unplaceable; soul fire is another type); frosted ice (unobtainable item);
tripwire hook (full live line scan).

**Scheduled ticks — state machines the swap can't forge.** Big dripleaf `TILT`, sculk
sensor `PHASE`, eyeblossom (transforms by time of day; open/closed are distinct blocks),
sculk shrieker (`SHRIEKING` gate + placed shriekers lack `CAN_SUMMON`), composter (7→8
only), lightning rod / target / lectern (unconditional clear of a state a fresh block
lacks), creaking heart (live log scan), dried ghast / sniffer egg (inherit ≤ one stage of
a 5000 gt / multi-thousand-gt timer — strictly worse than leaving the original block),
brushable blocks (loot lives in the BE; can't be crafted).

**BE-delegating ticks.** Dispenser/dropper/crafter dispense 4 gt after the rising edge from
the *current* BE+state (`DispenserBlock` 81–97, 121–137; `CrafterBlock` 67–86, 143–155) —
the swap window (3-tick mine at hardness 3.5) barely fits, and the recreated BE is empty,
so the firing dispenses nothing. Dropper↔dispenser fails the type check. Chest/barrel/
ender-chest ticks are lid `recheckOpen`. Command/test blocks aren't survival.

**Falling handoff.** `FallingBlock.tick` (45–50) spawns from the current state, but the
type check confines the swap to states placeable anyway (anvil facing; powder colours and
anvil damage are distinct blocks). `FallingBlockEntity` landings are guarded
(`canBeReplaced`+`canSurvive`, 187–201). Speleothem column suppression self-heals through
the shape-update cascade (each removal re-schedules the next segment), and a sturdy
replacement legitimately holds the column.

**Movement orphans beyond observers.** Falling blocks re-schedule on landing
(`FallingBlock.onPlace`); leaves recompute; `DESTROY`-reaction components pop rather than
move. Landing runs `updateFromNeighbourShapes` through the real tick access (`Block`
195–205), which is why pushed observers keep their pulse (flying-machine atom) — and why
only the pull geometry orphans.

## 5. Tooling change (in this branch)

`crates/bedrock`: scenario `target_block` (serde default `minecraft:bedrock`) replaces the
hard-coded bedrock in `trial`'s settle/gone checks (`lib.rs`), the vanilla runner's
`is_target_block` (`vanilla.rs`), and the one struct literal (`machine.rs`). Bridge matching
is a state-string prefix (so `minecraft:piston` also matches `piston_head` — used by P4);
vanilla matching is an exact block id (documented on the field). Default behaviour for all
existing scenarios is unchanged.

## 6. Next steps for whoever picks this up

1. An armed-machine (breaker-style) press against end portal frames in a real stronghold
   and reinforced deepslate in an ancient city (in-situ geometry: frames sit in a ring at
   fixed heights; the swap piston needs its cell above the frame and a crawl stance).
2. P10's restart half, if a restart-capable harness appears (extended-unpowered piston
   after relog; also the reschedule-storage swap on chunk re-entry).
3. T2's held-mine timeline on the official server if the bot gains multi-window actions.
4. Re-check the falling-block handoff seam (§4) if a future fallable gains a placeable
   state with teeth; likewise any new `triggerEvent`/`getTicker` that switches on state
   must keep the rebind guarantee.
