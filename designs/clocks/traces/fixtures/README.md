# Exact clock-trace source fixtures

These are historical test apparatus, **not archived or published clock recommendations**.
When an archive design is stripped down, its original exact recording still includes every
removed inert block. Repointing that recording at the smaller core is wrong even when the
boat follows the same trajectory: the block snapshot differs at frame zero.

`redsim-fuzz trace-fixtures --binary <saved-pristine-redsim-fuzz> --write` restores a missing
recording's source design byte-for-byte from its last git revision before deletion, checks
the complete original recording with the pristine model, and only then changes the header's
`design` path. It never changes tick records or writes the archive. `sources.tsv` records
provenance. Without `--write`, it only checks candidates in a scratch directory.

The positive/negative control for this repair was `clk-misc-boat-bouncer-c3`:
- The stripped core differs at tick 0 at the two removed stone cells, `(0,1,-1)` and `(0,1,3)`.
- The pre-strip original at `d9414c058^:designs/clocks/archive/clk-misc-boat-bouncer-c3.txt`
  matches all 176 recorded frames with the pristine `35bcee3c7` binary.
- Its restored bytes have git blob hash `c30ba857c2f690eb09663b2e0528cfc92ba60707`.

Publication isolation: `site::clocks::export` reads only verdict rows under
`designs/clocks/archive` (`crates/site/src/clocks.rs:15,120–124`), as does
`clock::catalog::catalog` through `read_rows`. This fixture directory has no archive
verdict/caption table and is outside that input root. Do not add these fixtures to an
archive, a catalog, a pick list or `crates/site/published.tsv`.
