# Legacy gate evidence and provenance

## What was run

Recorded Nov 7, 2026, on base `35bcee3c777a01d6102d95ab28ff8c127034a005`, with the
boat physics later committed as `a8ec0374cff4895f0a4db3ef861c5d10b5f56cbe`.
`traces-before.txt` is the saved pristine model; `traces-after.txt` is the new model.
Both replay all 190 kept exact traces with identical results.

The archive inventory is all 43 boat/raft designs identified by entity contents.
`archive-before.txt` and `archive-after.txt` are byte-identical: 39 positive records and
4 existing invalid records, already hidden in the registry for non-free outputs.
Nothing was re-tapped, re-baselined or newly hidden to make this pass.

`boat-game-verification.txt` is the complete official-code run at four positions and four
rotations per design: **688 builds, not 688 passing builds**. It includes all failures.
The 39 positive records reproduce their registered robustness flags and have model
agreement at the base build. In particular, the already `rotation-dependent` p186/96ead5
record is periodic in 8/16 builds, not 16/16. The four hidden negatives fail as expected.
The original aggregate command returned nonzero; that result was not presented as all
valid. The old capture has no separate child-exit-code file; its full output is retained.

The ring's game check rejects its initially occupied pressure-plate output; the model
rejects later boat entry at the same output cell. Both are the already registered non-free
output condition. The raw messages remain distinct. The auditor only accepts the
recognized occupied/entered forms at the same coordinate and an existing `no output`
hidden reason. It rejects unrelated/new failures, missing/duplicate names, changed
robustness flags and incomplete robust builds. Every record, including dependent and
negative records, must report all 16 builds; 1/1 robust, 8/8 dependent and 0/1 negative
reports are rejected even though their individual counters are internally consistent.
The runner explicitly requests the same four positions used by that audit contract.

## Narrow reuse across the main rebase

The recording above is **old-base evidence, not a newly executed recording** after rebasing
onto `d845f712d81fb6eace9e05c81790d96221d028ad`.

The entire intervening diff adds only `crates/trends`, its standalone package entry in
Cargo.lock, and search-demand/tool documentation. The new trends package uses the existing
workspace anyhow/clap/serde_json dependencies, with no extra features or existing package
version changes. This command showed no changes to the relevant source and inputs:

```
git diff --exit-code 35bcee3c777a01d6102d95ab28ff8c127034a005..d845f712d81fb6eace9e05c81790d96221d028ad -- bridge crates/redsim crates/clock crates/clock-cli crates/circuit crates/mctas crates/mctas-cli designs Cargo.toml
```

The coordinator approved reuse of this expensive game recording on that narrow basis,
with strict audit of all results. Workspace build, unit/integration/fn_length tests,
full/quick selftests, the 16 exact boat fixtures and 190 legacy comparisons must still be
run on the rebased tree. If the input/source/dependency equivalence fails, reuse is invalid.

## Inspect or reproduce

`mctas boat-leads legacy-audit --out-dir designs/boat-leads/evidence/legacy`
validates these **saved reports**; it does not execute a new game run.

`mctas boat-leads legacy --baseline <pristine-redsim-fuzz> --out-dir <new-directory>`
runs the complete gate anew, with at most two official-game workers by default. It preserves
a normal nonzero exit for expected negative inputs as evidence, then checks every record
against the existing registry rather than calling all records valid.
