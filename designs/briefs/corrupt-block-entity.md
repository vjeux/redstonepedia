# Brief: block-entity desynchronization (corruption audit, lane 3 of 5)

## Use

Find survival-reachable ways in Minecraft Java 26.3 for a block and its block entity (BE) to
disagree — in type, facing, inventory, joined-container relationship, moving-piston payload,
effect, spawner/vault state, text/pattern, or saved data — and determine which mismatches are
useful, persistent, and controllable rather than immediately repaired or merely client-visual.
This is an audit, not a component for the site; the [process](../../docs/process.md) shape is
kept (mechanics from the code first, the game's own guarantee as the bar, then experiments
against it) with the "layout" slots read as candidate mechanism families.

## The number

For each candidate mechanism: (1) a survival sequence a player can actually perform (items,
clicks, contraption ticks), (2) whether the wrong state survives a chunk unload + reload
(`saveAdditional` vs the chunk palette), (3) what it buys (items moved/voided/duplicated, an
effect or spawn rule changed, a container joined or split wrongly, saved player/spawner data
carried across a block change). A mechanism counts only if all three are answered in the bridge
**and** on the unmodified official server.

## The mechanics, from the game code

All citations are to the 26.3 decompile (`mctas src`), class:lines.

### The one gate: `LevelChunk.setBlockState` (LevelChunk.java:267-350)

Every in-level block change funnels here. The order is the whole audit in miniature:

1. The section is written first; identical state (`oldState == state`) returns early (278-281).
2. `blockChanged = !oldState.is(newBlock)` (303). If the block changed, the old state had a BE,
   and the new state does not ask to keep it -> `preRemoveSideEffects` (301-312) then
   `removeBlockEntity` (314). `preRemoveSideEffects` is skipped when flag 256
   (`UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS`, Block.java:98) is set, and it is what drops
   container contents (BlockEntity.java:233-237), pops a jukebox record, stops a creaking
   heart, and `finalTick`s a moving piston (overrides at their BEs).
3. `affectNeighborsAfterRemoval` (317-319, only with flag 1 or piston-move 64), then a
   reentrancy guard: if the cell no longer holds `newBlock`, return (321-323).
4. `onPlace` (325-327, skipped by flag 512).
5. If the new state has a BE and the cell still holds `newBlock` (329): fetch the existing BE;
   if it is not valid for the new state, warn "mismatched block entity" and remove it (331-335);
   create if absent (337-341); otherwise `blockEntity.setBlockState(state)` refreshes the BE's
   *cached* state and rebinds its ticker (342-345).

Consequences that set the search space:

- **Same-block property changes always keep the BE** (blockChanged is false), so any fact
  encoded both in a block-state property and in BE memory can drift if one writer touches only
  one side. Cached-state refresh in step 5 keeps `getBlockState()` honest — except when
  `onPlace` itself rewrites the cell (see the hopper/sculk case below): the inner write creates
  the BE, then the outer call re-stamps the BE with the outer (stale) state at line 343.
- **Cross-block BE survival is opt-in and rare**: `shouldChangedStateKeepBlockEntity` defaults
  to false (BlockBehaviour.java:247-249) and is overridden exactly twice:
  `CopperChestBlock:125-127` (old is `copper_chests`) and `CopperGolemStatueBlock:125-127`.
  Those two tag families are the only blocks whose identity can change in place (oxidation,
  wax, axe scrape, lightning) without losing their BE.
- A BE type serves many blocks (`BlockEntityTypes.java`: SIGN 26 blocks, SKULL 14, BANNER 32,
  SHULKER_BOX 17, CHEST = chest + all copper chests, CAMPFIRE 2, BEEHIVE 2, BRUSHABLE 2,
  SHELF 13, COMMAND_BLOCK 3), and `type.isValid(state)` checks only that set
  (BlockEntity.java:64-66). But with no keep-BE override and no in-place transform between
  those blocks in survival, a wrong-block/same-type BE cannot be *made* by playing; it can
  only arrive from chunk data (another lane owns that) — where it dies: `loadStatic` calls
  `type.create`, whose constructor throws on an invalid state, caught and turned into "no BE"
  (BlockEntity.java:58-66, 168-190; promotion in LevelChunk.java:594-616).

### Creation, ticking, save/load

- `getBlockEntity(pos, CHECK)` promotes a pending (just-loaded) BE on first touch before
  answering (367-377), so a pending tag cannot outlive a block change at its cell.
- Ticking goes through `BoundTickingBlockEntity.tick`, which re-reads the chunk state every
  tick, refuses to tick a BE whose type is invalid for it (743-756), and passes the *live*
  state to the ticker. Code that consults `this.getBlockState()` (the cached copy) is the
  exception; `HopperBlockEntity` even caches `facing` separately and refreshes it in its
  `setBlockState` override (HopperBlockEntity.java:40-45, 87-90).
- Tickers are (re)bound in `updateBlockEntityTicker` from the BE's cached state
  (699-719). One block branches its ticker on a state property: `PotentSulfurBlock.getTicker`
  (139+) picks among null/nausea/waiting-countdown/launch by `STATE`, a property its own
  `updateShape` recompute drives (`validBlockState`, 50-88).
- Saving writes every BE in the chunk maps (SerializableChunkData.java:346-353;
  LevelChunk.java:451-466, pending ones verbatim with `keepPacked`). Reload therefore
  *repairs* type mismatches (they fail `create`) but preserves data mismatches inside a
  valid type — including property-vs-field drift where the palette stores the property and
  `saveAdditional` never stored the field (e.g. `BrushableBlockEntity.brushCount`, below).
- Item -> block data flow on placement (BlockItem.java:76-99): place, then `BLOCK_STATE`
  component applied with flags **2** (123-135: no neighbor or shape updates), then
  `BLOCK_ENTITY_DATA` if the type matches and the type is not op-only or the player is a
  gamemaster (151-174; op-only set at BlockEntityTypes.java:218), then implicit components
  (`applyImplicitComponents`, e.g. container contents). Survival-obtainable carriers are few
  (shulker box container, decorated-pot decorations, beehive bees + copied `honey_level`,
  copper-golem-statue pose + name) — checked against the generated loot tables.

### Pistons and moving blocks

- Pistons never move a BE block: `PistonBaseBlock.isPushable` ends `return
  !state.hasBlockEntity()` (256), after the `POPPED` case (244-251). A BE block with
  `PushReaction.POPPED` in a push line is destroyed with normal drops (`moveBlocks` 285-297,
  flags 18), never carried. Moved blocks are snapshotted before any write (274-278) and
  installed as `moving_piston` + `PistonMovingBlockEntity` (299-320, flags 324 = 256|64|4).
- A payload has **two different landing codes**. Natural landing
  (`PistonMovingBlockEntity.tick`, 294-327) recomputes shape, special-cases a payload that
  became air, *strips `WATERLOGGED`* (309-311), and places with flags 67. Early placement
  (`finalTick`, 262-283: 0-tick/spit, and `preRemoveSideEffects` on overwrite) recomputes
  shape and calls `setBlockAndUpdate` — **no waterlog strip, no can't-survive case**. The
  same block can therefore arrive in two different states depending on *how* its move ends.
- The moving BE saves `progressO`, not `progress` (345); harmless replay of half a tick.

### Joined containers

- Double chests: pairing lives in the `TYPE`+`FACING` properties, maintained by
  `ChestBlock.updateShape` (145-172) and placement (197-234), and consumed live by
  `DoubleBlockCombiner.combineWithNeigbour` — which additionally requires both halves to be
  the **identical Block object** (`neighbourState.is(state.getBlock())`, line 42) with
  complementary types and equal facing, else each half opens as a single 27.
- Copper chests keep that object identity true by force: a half whose partner changes
  *adopts the partner's block* in `CopperChestBlock.updateShape` (91-96), placement picks
  the least-oxidized block of the pair (59-78), and only the non-`RIGHT` half random-ticks,
  and never while open (WeatheringCopperChestBlock.java:25-31). Wax/scrape/lightning all go
  through `setBlock(...,11)`/`setBlockAndUpdate` (HoneycombItem.java:80,
  BlockTransformer.java:98, LightningBolt.java:193-239), so the partner is re-synced inside
  the same call. Any path that changed one half *without* a shape update to the other would
  leave a visual double chest that is two singles — including its openers counters and the
  copper golem's transport targeting.
- Shelves (new in the copper drop) are the second joined container: up to 3 powered shelves
  chain via `SIDE_CHAIN_PART`, maintained not by `updateShape` but by `ShelfBlock.onPlace`
  and `neighborChanged` calling into `SideChainPartBlock` (ShelfBlock.java:101-117, 271-277;
  SideChainPartBlock.java:26-93). The hotbar swap derives the chain at click time from the
  stored parts (`getAllBlocksConnectedTo`, 26-53), trusting them. `POWERED` is re-read only
  on neighbor updates — the same BUD-shaped staleness pistons have, now on a container.

### Per-family duplicated encodings (the drift surface)

| family | block property | BE side | who rewrites the property |
|---|---|---|---|
| brewing stand | `has_bottle[0..2]` | items 0-2 | BE ticker each tick (BrewingStandBlockEntity.java:175-188, flags 2) |
| chiseled bookshelf | `slot_*_occupied` x6 | items + `lastInteractedSlot` | only `setItem`/`removeItem` via `updateState` (35-51); `applyImplicitComponents` (123-126) does **not** |
| jukebox | `has_record` | record + song player | `setTheItem`, guarded by cached-state identity (JukeboxBlockEntity.java:41-46) |
| lectern | `has_book`, `powered` | book/page | block on take/put; BE `preRemove` drops the book **only if the property says so** (218-228) |
| brushable | `dusted` 0-3 | `brushCount` (**not saved**), loot table/item | `brush()`/`checkReset()` (77-83, 148-168) |
| furnace family | `lit` | burn timers | ticker (AbstractFurnaceBlockEntity.java:217-218) |
| crafter | `crafting`, `triggered` | `craftingTicksRemaining` | BE ticker clears `crafting` (242); block sets `triggered` |
| barrel | `open` | openers counter | BE (BarrelBlockEntity.java:131) |
|TrialSpawner/Vault | `trial_spawner_state`, `vault_state`, `ominous` | spawner data, rewarded-player set | the BE is the writer; `isOminous` is not saved and is re-derived from the property every tick (TrialSpawner.java:103-113, 268-269) |
| beacon/conduit | none | effects/levels, active scan | ticker rescans the pyramid/frame; illegal stored effect combos go dormant, not wrong (BeaconBlockEntity.java:233-243, 262-290) |
| sculk sensor/shrieker/catalyst | `phase`/`shrieking`/`pulse`, `can_summon` | vibration data, warning level | BE + listener; `can_summon` read from state |
| hopper | `enabled`, `facing` | cached `facing`, cooldown | block `checkPoweredState` (flags 2), BE reads live state in ticker |
| campfire | `lit`, `signal_fire`, `waterlogged` | cooking items/timers | placement, douse, `placeLiquid`; ticker keys off live `lit` |

### Falling blocks: the one block -> entity -> block transfer

`FallingBlockEntity.fall` snapshots the state (stripping `WATERLOGGED`), sets the origin to
its legacy fluid block, and spawns the entity — **without capturing the BE**
(FallingBlockEntity.java:94-105). The entity has a `blockData` slot and a careful
merge-on-landing (213-231) plus save/load of it (301-317), but no vanilla spawn path ever
fills it. The only Fallable BE blocks are suspicious sand/gravel
(`BrushableBlock.tick` calls `fall` then `disableDrop`, BrushableBlock.java:66-75), so a
brushable block that falls and lands intact keeps its `DUSTED` property (state) but gets a
fresh BE: loot table, unpacked item, and brush count all gone.

## The trade-offs

- **Useful vs merely wrong.** Most paths the code defends (type revalidation on every
  set/tick/load). The interesting residue is *data* drift inside a valid type: properties the
  ticker does not continuously recompute, fields never saved, and container relationships
  stored as properties (chest `TYPE`, shelf chain parts).
- **Controllability.** A mechanism that needs an update-chain overflow or chunk-file edits
  belongs to lanes 1/5; this lane only claims sequences made of placements, clicks, redstone,
  pistons, weather, and growth — at most borrowing a suppressed shape update as a *stated*
  precondition, never as the mechanism itself.
- **Persistence.** Anything whose wrong half lives only in memory (cached state, open menus,
  counters) must be re-checked after `save-all` + chunk reload; the official-server runs do
  exactly that (region readback).
- **Cost of the hunt.** Bridge (`door drive`) experiments are seconds; official-server
  confirmation is the slow step, so only source-proven candidates go there.

## Candidate families (the audit's "layouts")

1. **Falling brushable data loss + unsaved brush count** (source-proven). Remove the support
   under a suspicious sand/gravel: the treasure is destroyed even when the block lands
   intact, and a half-brushed block keeps its `dusted` stage with `brushCount` reset.
   Usefulness is negative (loss), but it is the cleanest block<->BE data disagreement in the
   game: the block claims `dusted=3` archaeology content that its BE has lost. Persistent
   until brushed (the next `brush()` recomputes the property from the fresh counter).
2. **Moving-piston payload: two landings, two states** (source-proven difference). A
   waterlogged pushable block (stairs/slab/leaves...) that is *spit/0-ticked* keeps phantom
   water; the same block landed normally is dried. Test both endings of one move, plus a
   can't-survive-at-destination payload for the missing air case in `finalTick`.
3. **Copper double chest desync** (machinery mapped; hypothesis: robust). Attack the sync
   invariants: oxidation with the menu open (the guard reads the *ticking half's* openers),
   waxing/scraping one half, lightning random-walk order, placing a second half of a
   different oxidation/wax state, halves straddling a chunk border with only one side in
   random-tick range. Measure: do the halves ever end as different blocks (two singles) or
   with `TYPE` no longer complementary? Also record the by-design oddity that placing a
   chest can de-oxidize/unwax its partner (least-oxidized rule) while keeping its inventory.
4. **Shelf chain staleness** (hypothesis: BUD-shelf exists). Power a 3-shelf chain, then
   remove the signal by a path that does not neighbor-update the shelves (the silent-power
   geometries of docs/mechanics.md adapted: a repeater switching only its front block's
   neighbors, a piston moving the powering block). A shelf that keeps `powered=true` and
   its chain part still hotbar-swaps; one that never notices power stays single-swap.
   Also probe chain repair on middle-shelf removal/replacement orders.
5. **Cached-state staleness via onPlace self-rewrite** (source-proven, inert today).
   `HopperBlock.onPlace`/`SculkSensorBlock.onPlace` rewrite their own cell; the outer
   `setBlockState` then stamps the BE with the stale outer state (LevelChunk.java:343).
   Verify in the bridge that `getBlockState()` disagrees with the chunk until the next
   state change, and that nothing observable reads the stale copy — a documented latent
   hazard, ranked below anything a player can spend.

Rejected on source evidence (in NOTES, with the lines): wrong-type BE by play (revalidated
everywhere), piston-carried BEs (immovable), orphan open-menu duplication (contents are
split to count 0 on drop; the orphan can only *lose* items for one tick), chiseled-bookshelf
hidden books via item components (real code gap — `applyImplicitComponents` never calls
`updateState`, and the block's clicks trust the property, so a filled-but-unflagged slot
both hides its book and voids it on insert — but no survival loot gives the shelf a
container component; contents drop loose on break), beacon effect illegality (dormant, not
wrong), trial-spawner/vault ominous drift (property is the source of truth), ProtoChunk
worldgen orphans (not player-controlled; dropped at promotion unless same-type).

## The decision

Run families 1 and 2 first (source-proven, cheap, and they exercise the two transfer paths —
entity and moving piston — end to end), then 3 and 4 as red-team experiments against the
sync machinery, then 5 as a bridge-only demonstration. Every positive result gets a
`door drive` scenario in `designs/corrupt-block-entity/` and an official-server run with the
new `mctas beaudit vanilla` scenario runner (RCON + region readback, so BE tags are read
from the saved world, not from a modded sandbox).

## Must-haves

- Survival sequences only for claims: the setup may use `data merge`/setblock in the
  *sandbox* to reach a state survival can reach (e.g. a loot-table-bearing suspicious
  sand), but the step that creates the mismatch must be a player action or game process.
- Each verified mechanism: bridge evidence, official-server evidence, save/reload verdict.
- Rust only for any tooling; no chunk-file corruption experiments (lane 5 owns triggers).

## Not goals

- No new judge, circuit kind, or site component. Nothing here is published; the branch is
  `explore/corrupt-block-entity` and the record is this brief + `designs/corrupt-block-entity/NOTES.md`.
- No community designs or internet sources; everything above is from the 26.3 source.
- Client-only visual glitches are reported as such, not counted as desyncs.

## The bar and the ceiling

The bar (vanilla's own guarantee, the "obvious design"): after any survival action sequence,
for every cell, the BE (if any) is of the type the block creates, its cached state equals the
chunk state, properties that mirror BE memory match it, joined containers agree with their
neighbors, and a chunk reload reproduces all of it. The ceiling (what a perfect transplant
bug would give): arbitrary BE data moved between blocks/positions — spawner configurations,
vault reward histories, container contents relabeled to another block type. The source says
the ceiling is unreachable by play (type checks on create/set/tick/load); this audit measures
how far below it the reachable residue sits, mechanism by mechanism.

*Outcome (same day): the residue is one useful mechanism — spit/0-tick placement keeps a
waterlogged payload's phantom water where natural landing dries it — plus a by-design free
de-oxidation when placing beside a copper chest, and one progress-only desync (unsaved
`brushCount` vs the `dusted` property). Copper pairs and shelf chains held under every
attack tried. Full results, sequences and evidence:
[designs/corrupt-block-entity/NOTES.md](../corrupt-block-entity/NOTES.md).* 
