# Terrain samples

Real terrain for the farm judges: the official 26.3 server generates a seed's area once, offline;
`mctas terrain` reads its region files back into `.mcterrain` files (palette + run lengths, biomes,
heightmaps, the saved light, structure pieces, entities); redsim builds worlds from them
(`redsim::terrain::build_world`) with exact light, biomes and natural spawning. The rules and the
validation results are in the brief: [designs/briefs/terrain-and-spawning.md](../briefs/terrain-and-spawning.md).

## The standard samples (seed 2)

| file | crop | what is there | the cached world (re-made by) |
|---|---|---|---|
| `village-edge.mcterrain` | 120 x 120 columns (x -432..-313, z 190..309), the full height y -64..319 (698 KB) | plains east of a plains village: bells (-411, 70, 241/249), 8 villagers asleep in their beds, the iron golem, 2 cats | `2_-416_240_r192` (`mctas terrain gen --seed 2 --at=-416,240 --radius 192 --margin 2 --days 1 --keep`) |
| `plains.mcterrain` | 96 x 96 (x -328..-233, z 182..277), full height (431 KB) | open plains | the same |
| `forest.mcterrain` | 96 x 96 (x -64..31, z -48..47), full height (381 KB) | forest and a river | `2_16_0_r80` (`mctas terrain gen --seed 2 --at=16,0 --radius 80 --margin 2 --days 1 --keep`) |

`redsim::terrain::samples::STANDARD` lists them with each one's spot-search start. A design's spot
(`mctas terrain spot <sample> --size X,Z --below N`; `terrain::place::best_spot`) is deterministic,
so the judge, the viewer and the game check put the farm at the same place.

A re-made world is the same terrain but not the same day-1 details (the one game day of the `gen`
moves villagers and grows crops differently each run): the real-game check uses the cached world the
committed crop was cut from. If a cache is lost, re-make it and re-cut the crop (the spots stay).

## Commands

- `gen --seed S --at=X,Z --radius R [--days N] [--keep] [--world W]`: generate (or re-read a kept
  world) into `$MCTAS_HOME/terrain/S_X_Z_rR.mcterrain`; `find --seed S --what structure:village_plains`.
- `info FILE [--palette] [--locate TEXT] [--ticking]`, `map FILE --around=X,Z`, `crop FILE --around=X,Z --radius R --out F`.
- `blockinfo FILES... [--design D] [--state S]`: the natural-state table (`crates/redsim/src/terrain/blocks.tsv`),
  the game's own answers per state (the bridge's `blockinfo` op); `biomes`: `biomes.tsv` and
  `structures.tsv` from the game's data (spawn lists, tags, resolved colours, structure spawn overrides).
- `light FILE [--column=X,Z]`: redsim's light engine vs the saved light; `probe --world W --at=X,Y,Z [--times T,...]`:
  the running server's light; `chunk --world W --chunk=X,Z`: a saved chunk's sections.
- `paste-check FILE --world W --design D --at=X,Z`: a design pasted in redsim and in the official
  server (strict writes), every block and light value compared.
- `biome-check FILE --world W`: redsim's biome lookup vs `execute if biome`.
- `spawn-check AREA --world W --afk=X,Y,Z [--nights N --seeds N --ticks T --day-time D --fill T --trace --game-dump F --out F]`:
  natural spawning, the official server vs redsim; `spawn-explain FILE --type T --at=X,Y,Z`.
- `hostile FILE --design D --at=X,Z --afk=x,y,z`: the worst-case checks for natural mobs (C1-C16).
