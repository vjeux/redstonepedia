# Carousel sleep yard + the one bed (brief farm-iron-carousel.md, part 3) — being built

vjeux's rules: one bed for all 21 villagers; each villager naps once a night, one at a time; no
giant loops (volume decides). Each track's hold end is a hump (a second braked slope on the hold
control, a flat top, a plain slope down): a returning cart stops on its two slopes, except the
night's napper, whose hump is powered as its batch comes back: its cart climbs over and runs on, in
its own cart, through a yard of switched plain-rail T junctions (J_12, J_34, J_56 in the wiring
columns at z 25; T_0, T_25, T_f at z 27, 29, 31; two merges for the levels) onto the bed line. An
activator drops the napper into the free cell beside the bed's head; it lies down; the hatch opens
and the dorm zombie wakes it; 60 gt later the park rail sends its cart back to take it, and it rides
the same way back over its hump onto its hold slopes. No loop: each trip goes out and back.

Tools: `mctas mobfarm hold --yard --out F` writes this design; `hold-bench F` runs the seven
batches and sends each villager to the bed once a night (a dispatch on every 4th line fall in deep
night), checking the release + hold contract and the sleep routine's (docs/tools.md).

Measured with ideal controls (100 seeds x 6 days): 12,600 naps, every villager once a night, 0
second naps, a visit 298..456 gt, the junctions clear of the return within 439 gt; REST 0, golem
spawn spots 0; 1,376 blocks, box 14 x 11 x 49.

Not in yet: each track's hold-end feed wiring (laid out under `--feeds`, being tuned: its column
climbs cut some riders' sight), day-1's nightly scheduler, the station and pad.
