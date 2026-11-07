# Block-entity desynchronization audit — NOTES (lane 3 of 5)

Brief: [designs/briefs/corrupt-block-entity.md](../briefs/corrupt-block-entity.md). Branch
`explore/corrupt-block-entity`, clone `~/agents/corrupt-block-entity/minecraft-tas` on
devvm69723 (shared box; source-first, load checked before heavy work). Minecraft Java 26.3,
all line citations to `mctas src` output.

## Tools

- `mctas src grep/show` — the ground truth (server jar decompile).
- `mctas door drive <script>.drive` — bridge experiments. BE-relevant commands:
  `place <design>` (raw paste + the design's `nbt:` lines merged), `data x y z <snbt>`
  (a real `data merge block`), `items x y z` (prints a BE's data), `probe_items`, `expect_state`.
- `mctas beaudit vanilla <scenario>` (this lane, `crates/beaudit`) — official-server runs:
  a line script (`cmd`, `tp`, `hand`, `use`, `break`, `sneak`, `gamemode`, `step`,
  `restart`, `expect-block`/`expect-data`/`expect-no-be`/`expect-cmd`, `print-block`/
  `print-data`, documented in `crates/beaudit/src/vanilla.rs`) on an unmodified server:
  a bot player clicks with real packets, blocks are read from region files after
  `save-all flush`, BE tags via `data get block`, and `restart` reboots on the saved
  world — persistence claims are checked against an actual reload. Also new in
  `door drive`: `expect_be`/`reject_be`/`expect_no_be`.

## Source map (what I read first, in order)

`LevelChunk.setBlockState` (the one gate), `BlockEntity` (validation, loadStatic,
preRemove), `BlockEntityTypes` (type->blocks), the two `shouldChangedStateKeepBlockEntity`
overrides, `PistonBaseBlock.isPushable/moveBlocks`, `PistonMovingBlockEntity`,
`FallingBlockEntity` + `BrushableBlock`, `ChestBlock`/`DoubleBlockCombiner`/`CopperChestBlock`,
`ShelfBlock`/`SideChainPartBlock`, `BlockItem` placement flow, then per-family BE files for
properties that mirror BE memory. Ranked findings are in the brief; result sections below are
filled in as experiments land.

## Rejected on source evidence (no experiment spent)

- **Wrong-type / wrong-block BE by play.** Every entry path revalidates:
  LevelChunk.setBlockState:331-335 removes an invalid existing BE; `setBlockEntity`
  (421-448) refuses a type-invalid BE; the ticker refuses to tick one (743-756);
  chunk-load promotion drops it (BlockEntity.loadStatic 168-190). BE types that serve many
  blocks (signs, skulls, banners, shulker colors, campfire/soul, beehive/nest, brushable
  sand/gravel) have no survival in-place transform between those blocks, and the only two
  keep-BE overrides are the copper families.
- **Piston-transferred BEs.** `isPushable` returns `!hasBlockEntity`; POPPED BE blocks
  (signs, banners, heads) are destroyed with drops, never carried (PistonBaseBlock 244-256,
  moveBlocks 285-297).
- **Orphan open-menu duplication.** Breaking an open container splits every stack to count 0
  while dropping it (Containers.java:22-48), `stillValidBlockEntity` fails once the BE is out
  of the chunk map, so a second viewer's menu shows emptied slots and closes next tick.
  Loss-only, 1-tick, needs a second player racing the breaker. Not controllable.
- **Chiseled bookshelf hidden books via item components.** Real gap, unreachable in survival:
  `ChiseledBookShelfBlockEntity.applyImplicitComponents` (123-126) copies a container
  component into the BE without calling `updateState` (35-51), while the block's clicks trust
  the `slot_*_occupied` properties (ChiseledBookShelfBlock.java:89-91, 110-112) — a filled
  slot flagged empty hides its book from taking and *voids* it on insert (`setItem`
  overwrites, 90-97). But the generated loot (blocks/chiseled_bookshelf.json) drops only the
  shelf item; contents fall loose via the default `preRemoveSideEffects`, so no survival item
  ever carries the component. Creative pick-block only. (The same shape in
  `ShelfBlockEntity` is inert: shelf rendering/comparator read the BE, no properties mirror it.)
- **Beacon illegal effect combos.** Stored primary/secondary can outlive the pyramid that
  allowed them, but `applyEffects` re-gates amplifier and secondary on current levels
  (272, 284). Dormant, not wrong.
- **Trial spawner / vault ominous drift.** `isOminous` is unsaved scratch re-derived from the
  block property every server tick (TrialSpawner.java:268-269); state enums live *in* the
  properties and are written only by the BEs.
- **Jukebox `has_record` guard.** `notifyItemChangedInJukebox` only syncs when
  `level.getBlockState(pos) == getBlockState()` (41-46) — with a single property that only it
  writes, cached and chunk state cannot part in survival. Its `setRemoved` (126-130) calls
  `level.gameEvent` with no null check; a removed-but-never-leveled BE would NPE, but no
  survival path constructs one.
- **Pending/keepPacked BEs and wrong-chunk relocation** (`BlockEntity.getPosFromTag`
  rewriting x/z to the reading chunk, 68-81) need crafted chunk data — lane 5's triggers.
- **ProtoChunk orphans.** Worldgen `setBlockState` does no BE bookkeeping
  (ProtoChunk.java:115-167); promotion revalidates each BE against the final state and drops
  mismatched ones (LevelChunk.java:143-147 + 421-448). Same-type transfers during generation
  exist in principle but are structure-fixed, not player-controlled.

## Candidate results

All scenario files are in `designs/corrupt-block-entity/scenarios/`. Bridge:
`mctas door drive --quiet designs/corrupt-block-entity/scenarios/*.drive` (5 files PASS).
Official: `mctas beaudit vanilla designs/corrupt-block-entity/scenarios/*.vanilla` (4 files PASS).

### 1. Moving-piston payload: phantom water — CONFIRMED, useful, persistent

A waterlogged pushable block arrives in a different state depending on *how* its move ends.
Natural landing (`PistonMovingBlockEntity.tick`) strips `WATERLOGGED` (309-311); early
placement by a sticky retract (`finalTick`, 262-283 — every spit/0-tick) does not.

- Sequence (bridge `payload-waterlogged.drive`, official `payload-waterlogged.vanilla`):
  waterlogged oak stairs in front of two sticky pistons; lane A steady lever (natural
  landing), lane B observer 2gt pulse (spit). Result in both worlds: A `waterlogged=false`,
  B `waterlogged=true` at its dry destination. After a full server restart on the saved
  world, both states are unchanged (palette-persistent).
- What it buys: portable phantom water — the block's fluid state is a water source at a dry
  cell (bucket-pickable once; hydrates farmland; hardens adjacent concrete powder; sponge
  food). The same payload normally pushed loses its water, so the spit is the only way to
  relocate a waterlogged block with its water. Controllability: any standard spit/0-tick
  gadget; robustness: any pushable waterloggable block (stairs, slabs, fences, leaves,
  mangrove roots...), any direction.
- The sibling divergence in the same code (natural landing's "became air → place then
  break-with-drop" vs `finalTick`'s silent set-to-air) has **no survival payload**: every
  block whose `updateShape` can return air (`grep 'return Blocks.AIR.defaultBlockState'`:
  bell, ladder, mossy carpet, coral, dripleaf, sea pickle, multiface, repeater...) is
  `PushReaction.POPPED` or a BE block, so it is destroyed or immovable, never carried.

### 2. Suspicious sand: falling is destruction (by design); brush progress desyncs on reload — CONFIRMED, minor

- Falls: `BrushableBlock.tick` spawns the falling entity with `disableDrop()`
  (BrushableBlock.java:73); `cancelDrop` makes the landing branch discard it outright
  (FallingBlockEntity.java:190, 244-247). There is no landed-but-lootless state: bridge and
  official runs (`brushable-fall.drive`, `brushable.vanilla` part A) both end with air, no
  BE, control untouched. So the block→entity transfer dropping the BE (`fall()` never fills
  `blockData`, 94-105 vs the dead restore at 213-231) is invisible: the block dies anyway.
- Reload: `brushCount` is not saved (`saveAdditional`, BrushableBlockEntity.java:217-223)
  but `DUSTED` is a block property. Official run part B: brush to `dusted=2`, restart the
  server on the saved world (`dusted=2`, loot already unpacked to `item`), one brush stroke
  → `dusted=1`. The visible stage overstates progress by up to the full 10 strokes after any
  unload; self-repairs on the first stroke. `hitDirection` is likewise load-only.
  Usefulness: none for the player (pure loss of progress), but it is a real, persistent
  property-vs-BE disagreement reachable by ordinary play near a chunk border.

### 3. Copper double chests: sync machinery holds — ROBUST (with one by-design oddity)

Bridge (`copper-chest-sync.drive`, `copper-chest-border.drive` — the latter straddles
x=15/16) and official (`copper-chest.vanilla`, bot with honeycomb/axe):

- One-sided wax, unwax, scrape, and a full identity swap (exposed pair -> waxed oxidized
  via one half): the partner always adopts the new block in the same call
  (`CopperChestBlock.updateShape:91-96`), both inventories survive every identity change
  (`shouldChangedStateKeepBlockEntity` + revalidation), types stay complementary, and
  breaking one half unlinks to `single`. Nothing desyncs; the
  `DoubleBlockCombiner` same-block requirement (DoubleBlockCombiner.java:42) remains
  unreachable by play.
- Oddity worth knowing (official run): *placing* a waxed exposed copper chest beside an
  unwaxed oxidized single produces an **unwaxed exposed pair** — the placed chest loses its
  wax and the existing chest is de-oxidized two stages for free, contents kept
  (`getLeastOxidizedChestOfConnectedBlocks`, CopperChestBlock.java:59-78). Persistent
  across restart. A cheap way to un-oxidize a filled chest without an axe: place any
  less-oxidized copper chest beside it, then break the newcomer.
- Survival detail found on the way: waxing/scraping a copper *chest* needs a sneak-click;
  a plain click opens the menu and consumes the interaction before the item runs
  (ServerPlayerGameMode.java:410-423).

### 4. Shelf chains: baseline correct; BUD-shelf not reachable — ROBUST

- Baseline (bridge `shelf-chain.drive`, official `shelf-swap.vanilla`): three powered
  shelves form right/center/left, a click swaps all nine slots with the hotbar in chain
  order (eastmost shelf <-> hotbar 0-2 for north-facing), unpowering the middle unlinks
  both ends, and parts + contents survive a restart.
- Staleness hunt (negative): a shelf's `POWERED` is refreshed only in `neighborChanged`,
  so I went through every block that overrides `getDirectSignal` looking for one that
  changes the power *through a conductor* without updating the powered cell's neighbours
  (the shelf is one of them). Every candidate covers the shelf: observers and diodes call
  `updateNeighborsInFront` (ObserverBlock.java:53, 80-86 — verified live: the shelf
  tracked an observer pulse through stone on both edges), plates/detector rails/sculk
  update the support's neighbours, torches update two deep, dust two deep, the lightning
  rod updates the cell it points into. Redstone blocks don't power through conductors at
  all (`getDirectSignal` 0). Two silent-power rigs were built and both tracked correctly;
  they were deleted rather than kept as passing negative tests. Remaining theoretical
  window: a shelf *placed* during a lightning rod's 8gt pulse (`getStateForPlacement`
  reads the live signal) keeps `powered=true` after the rod cools — placement inside a
  8gt weather event, never built.

### 5. Cached-state staleness via onPlace self-rewrite — SOURCE-CONFIRMED, inert

`HopperBlock.onPlace` (HopperBlock.java:93-97 -> `checkPoweredState` 118-123) and
`SculkSensorBlock.onPlace` (105-111) rewrite their own cell with a different state of the
same block. In `LevelChunk.setBlockState` the inner write creates the BE, then the outer
call stamps it with the outer state (343): a hopper placed while powered lives with
`getBlockState()` claiming `enabled=true` while the chunk holds `enabled=false` until the
next state change. Nothing observable reads the stale copy (the ticker gets the live state
every tick, LevelChunk.java:743-745; `HopperBlockEntity.facing` is identical in both
states), and chunk reload repairs it — a latent trap for the next Mojang feature that
trusts the cache, not a player-usable desync. No scenario can observe it from outside the
game, so none was kept.

### What did not get an experiment (and why)

- Chiseled-bookshelf hidden books: code gap confirmed, but no survival item carries the
  container component (loot drops the shelf alone; contents drop loose). Creative
  pick-block only. Left as a source note above.
- Copper-chest oxidation by random tick and by lightning walk: same `setBlockAndUpdate`
  into the adoption path already proven with tools; natural oxidation is ~days of game
  time. The open-menu oxidation guard reads the ticking half's own openers counter;
  opening a double chest registers openers on both halves (compound container), so the
  "other half open" hole needs a copper golem as the second opener — not chased.
- Chunk-border *ticking* edge (properties like `has_bottle` freezing while a cross-border
  hopper still inserts): real but self-repairing the moment the chunk ticks; vanilla
  chunk semantics, not corruption.

## Ranked outcome

1. **Phantom-water payloads** (spit a waterlogged block to move its water) — the only
   useful, persistent, controllable block/BE-state disagreement found. It is a difference
   between two landing codes inside one BE class.
2. **Free de-oxidation by placement** beside a copper chest — by-design sync rule with a
   player-usable side effect; contents and pairing always stay consistent.
3. **`dusted` vs unsaved `brushCount`** — persistent visual/progress disagreement, pure loss.
4. Everything else audited is defended in depth: type revalidation on create/set/tick/load,
   same-block keep-BE narrowness (two copper families), immovable BE blocks, adoption force
   in copper pairs, and notification discipline around conductor power. The audit's ceiling
   (transplanting BE data between blocks by play) stayed out of reach everywhere we looked.
