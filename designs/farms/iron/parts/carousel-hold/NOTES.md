# Carousel release + hold (brief farm-iron-carousel.md, part 2)

Seven batches of three villagers, one batch a lane, one villager a cart, each cart between two
braked one-rail slopes: the stand slope at the station end (z k+1 for level k), the hold slope at
the far end (z 21 - k, one nearer on lanes 2, 3, 6). The counter's line for a batch powers its hold
slopes and unpowers its stand slopes: the batch rolls to the station, stops on its stand slopes for
its slot (the golem summon) and rolls back to its hold when the line falls. Zombies in boats beside
every level keep every rider panicking, at the station, on the way and at the hold (two at each hold
end on paired lanes, one a block higher, so the staggered ends do not cut the parked riders' sight).

Paired lanes (ironspec's ruling, Nov 10): x 0,1 | 3,4 | 6,7 | 9, a wiring column beside each lane
(x -1, 2, 5, 8): every control has a side neighbour for a direct feed.

Tools: `mctas mobfarm hold --out F` writes this design (`--unpaired` the pitch-1 stack);
`mctas mobfarm hold-bench F --from 1 --to 100 --days 6 [--jitter 20]` runs the seven batches with
the counter's lines ideal and checks every villager every tick (docs/tools.md).

Measured (100 seeds x 6 days): 100/100 clean at jitter 0..20 and 0..45, arrival margin min 29 / 4
gt, REST 0, golem spawn spots 0; 908 blocks, box 14 x 11 x 26.
