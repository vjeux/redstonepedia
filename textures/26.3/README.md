# Game textures (Minecraft 26.3)

The 3D viewer draws builds with the game's own textures and block models.
These files are a vendored copy of the game's assets for the judge's version
(26.3), taken from misode/mcmeta — the community mirror block viewers use —
before its versioned branches (`26.3-atlas`, `26.3-assets`, `26.3-summary`)
were deleted (2026-10-20). The site used to fetch them from that mirror at
run time; when the branches went away, every texture on the site broke, so
they now live here and are served with the site, same origin.

Do not hand-edit these files. To move to a new game version, replace this
whole directory with that version's files and update the version in
`crates/mcview/src/atlas/vanilla.rs`, `crates/site/src/pages.rs`
(`data-mcview-textures`) and `crates/site/src/generate.rs`.

Contents (all of 26.3, from mcmeta):

- `atlas.png`, `data.min.json` — the block atlas and its index.
- `entities.png`, `entities.min.json` — the entity atlas and its index.
- `items.png`, `items.min.json` — the item atlas and its index.
- `block_definition.min.json`, `model.min.json` — every block definition
  (blockstate) and every block model.
- `particle/explosion_*.png` — the explosion's 16 particle sprites.

Provenance: Mojang's game assets, via misode/mcmeta. The site is a fan
project; these assets belong to Mojang.
