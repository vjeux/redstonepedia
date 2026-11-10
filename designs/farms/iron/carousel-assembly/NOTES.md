# Carousel assembly v0 (farmopt, WIP)

The three finished parts in one design on ironfarm's paired lanes (x 0,1 | 3,4 | 6,7 | 9):
ironfarm's release + hold, farmopt's station + 16-wide portal pad + Nether part (kill, sorter,
crafter, double chest), day-1's counter v7. The sleep routine is stood in by the benches'
once-a-night LAST_SLEPT (the real-game gate has no stand-in: it runs once the routine lands).

| file | what | written by |
|---|---|---|
| carousel-v0a.txt | step a: counter v7 under the stand (y -11..-9), the pad's pulse wired to its input; the counter's lines drive the slope controls IDEALLY in the bench (no line wiring) | `mctas mobfarm assembly --out F` |
| carousel-v0b-wip.txt | step b WIP: counter v7 in the yard space (z 30..42), the pulse wire along x 13, every line wired (fan-out, inverters, stand towers and torch chains, delivery cells at z 20 for the yard's hold feeds) | `mctas mobfarm assembly --wired --out F` |

## Runs

- v0a, the running preview: `mctas mobfarm station-bench carousel-v0a.txt --from 1 --to 1 --days 1
  --timeline DIR` (the counter in the loop: its real ring, its fallback and the pad's real
  pulse; each line switches its lane's slope controls ideally). 4 seeds x 1 day: a golem in 99.79%
  of 960 slots, one advance per slot, the batch at the stand a mean 25.8 gt before its slot, no
  violations. Static: `mctas mobfarm station-check carousel-v0a.txt` PASS.
- v0b, the wiring layout (static): `mctas mobfarm station-check carousel-v0b-wip.txt` PASS (0
  strays, held riders' searches empty); `mctas mobfarm assembly-check carousel-v0b-wip.txt`
  (the redstone alone): the counter cycles in order with the real pulse wire (397 advances in
  400 slots, 0 out of the cycle, never two lines high), the delivery cells follow their lines in
  2-6 gt. KNOWN DEFECT: the stand torch chain puts torches in the stand control cells, and an
  ascending rail needs a sturdy block at its high side (BaseRailBlock.shouldBeRemoved), so the
  three stand slopes pop off at the paste: a run of v0b shows the carts failing at the stand.
  The fix (each control a solid block fed NOT line: level 0 from a torch below, level 2 from a
  repeater on its north; level 1 has no free face) waits on ironfarm's stand geometry.

## Box

- v0a: 1,321 overworld blocks + 122 Nether, box x -4..14 y -11..9 z -6..25 = 12,768.
- v0b WIP: 1,978 + 122, box x -4..13 y -5..9 z -6..42 = 13,230 (day-1's counter + scheduler block
  under the yard replaces v7 and the hold delivery cells; ironfarm's yard sets z to 48).
