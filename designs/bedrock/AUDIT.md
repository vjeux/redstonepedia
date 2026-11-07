# Audit: play-time writes that could remove bedrock (MC 26.3)

Nov 6 2026 (audit sub-agent of track bedrock). Every code path that changes an existing block
during play was checked for whether a survival player can make it act on bedrock at a position of
their choice (Overworld floor y -64..-60, Nether floor y 0..4, Nether roof y 123..127). Line
numbers are the 26.3 decompiled listing (`mctas src show <Class>`).

Result: only `PortalForcer.createPortal` (NOTES.md section 1) and the piston retract
(NOTES.md section 2) can.

| path (file:line) | what it writes | guard | bedrock in survival? |
|---|---|---|---|
| **`PortalForcer.createPortal` 137-144** (frame bottom row) | obsidian at y-1 of the chosen site | site check `canHostFrame` 171: bottom row only `isSolid()` | **yes** (Overworld floor, Nether floor) |
| `PortalForcer` 121-134 (fallback platform, no site found) | obsidian / air, unconditional | y clamped to [max(minY+1, 70), maxPlaceableY-9] (108-115) | no: far from bedrock |
| `PortalForcer` vs the Nether roof | the frame | maxPlaceableY = 127 (59); y+4 <= 127 (77): frame bottom <= y 122 | no: roof bedrock starts at y 123 |
| `PortalForcer` 141, frame sides and top | obsidian | cells must pass `canPortalReplaceBlock` (canBeReplaced, no fluid; 158-161, 175) | no |
| `PortalShape` 190 (lighting a portal) | portal blocks, flags 18 | empty cells only | no |
| `TrunkPlacer.placeBelowTrunkBlock` 61-68 | dirt below the trunk | blocklist `cannot_replace_below_tree_trunk` does not list bedrock | no: saplings need `supports_vegetation` soil (`VegetationBlock` 19-20, `AzaleaBlock` 31, `MangrovePropaguleBlock` 45), so bedrock is never under a trunk; 2x2 trees check all four saplings (`TreeGrower` 182-193) |
| tree trunk / leaves / roots (`TrunkPlacer` 84, `FoliagePlacer` 175, `RootPlacer` 57, `MangroveRootPlacer` 124) | logs, leaves, roots | `validTreePos`: air or `replaceable_by_trees` (`TreeFeature` 85-87) | no |
| tree decorators (alter ground, vines, cocoa, beehive, attached, place on ground, pale moss, shelf) | podzol / attached blocks | rule tags, air or `isReplaceable` | no |
| `HugeFungusFeature` 45, 73-79, 91-97 | stem, hat | base must be nylium; cells `canBeReplaced` or plants | no |
| `AbstractHugeMushroomFeature` 39-79 | stem, cap | air/leaves box check; writes only to air or `replaceable_by_mushrooms` | no (a mushroom can sit on bedrock but will not grow) |
| mushroom spread, chorus, bamboo, cactus, sugar cane, vines, kelp, cave vines | plants | `isEmptyBlock` / air / water | no |
| `SpeleothemBlock` (dripstone, sulfur spike) 377-392 | tips | air or water | no |
| `PointedDripstoneBlock` 103 | mud -> clay | `sourceState.is(MUD)` | no |
| moss / pale moss patch (`VegetationPatchFeature`, `BonemealableFeaturePlacerBlock` 37) | moss | `moss_replaceable` (no bedrock) | no |
| grass / nylium bone meal | plants | origin air; `ONLY_IN_AIR` | no |
| sculk (`SculkBlock` 40/76-80, `SculkVeinBlock` 108) | sculk | `sculk_replaceable` | no |
| `MultifaceSpreader`, `CreakingHeartBlockEntity` 275 | multiface, resin | air / water source | no |
| `FlowingFluid.spreadTo` 267-276 | fluid | `canHoldFluid`: containers or `washed_away_by_fluids` | no |
| lava, fire spread, lightning, small fireball, end crystal, explosion fire | fire / stone | air or water only | no |
| explosions (`ServerExplosion`) | air | blast resistance (bedrock 3,600,000) | no |
| wither, ender dragon | air | `wither_immune`, `dragon_immune` | no |
| `FallingBlockEntity` 191-201 | its block | `canBeReplaced` on the old cell | no |
| `Enderman` 485 / 628 | place / take | target air; take needs `enderman_holdable` | no |
| `BlockItem` / `BlockPlaceContext.canPlace`, scaffolding, beds | placement | `canBeReplaced` | no |
| `DoubleHighBlockItem` 17-19 | the upper half, flags 27 | `getStateForPlacement` requires the cell above `canBeReplaced` | no |
| buckets, powder snow, bone meal on water | fluid, plants | `canBeReplaced` / empty / full water | no |
| dispenser behaviours (skull, pumpkin, wax, mud, fire, shulker box) | blocks | empty / waxable / `convertible_to_mud` / canBeReplaced | no |
| snow and ice, frost walker | ice, snow, frosted ice | water source / air | no |
| `EnderEyeItem` 50-62 | end portal over the 3x3 interior | the frame can not be placed in survival | no |
| golems, silverfish, dragon egg, ravager, mob goals, turtle, bee, copper golem, statue, dried ghast, crafter, trial spawner, vault | their own blocks | pattern / infested / air / own state | no |
| mangrove leaves, rooted dirt, mossy carpet, hanging moss, stems, budding amethyst | a neighbour | air or `canBeReplaced` | no |
| `DragonRespawnStage` 57-62 | `removeBlock` in a 21^3 box per spike, no guard | End only; spike tops are rebuilt | no (End only) |
| `EnderDragonFight` 563, `EndPodiumFeature`, `EndGatewayFeature`, gateway block entity | unconditional | End only, fixed places | no |
| chained-update limit (`CollectingNeighborUpdater` 68-84) | (drops updates) | a piston left extended while unpowered is an ordinary state; a piston placed later is never extended | no new route |
| head removal without `affectNeighborsAfterRemoval` | | runs only with flag 1 or 64 (`LevelChunk` 317-318); the only head removal without flag 1 is `PistonBaseBlock.moveBlocks` 261-262, reached only by an already-headless piston | no new route |
