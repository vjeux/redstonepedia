# Carousel bed siding (vjeux's single lane; ironfarm's hand-over 2026-11-10 07:11)

The one shared bed. A napper's cart alone leaves its batch at the stand's departure and runs into
the siding. The napper is dropped beside the bed, naps, is woken by the dorm zombie through the
hatch, and re-boards the same cart. The cart then waits on the exit slot until ironfarm's release
pulse sends it out to rejoin its batch on the release run (~600 gt after it left).

Status, in vjeux's terms (07:29: "you can't say it's working when nothing is wired"):
- **Wired:** `carousel-siding-wired.txt`, real redstone.
  - The bench touches no control.
  - It does two things in place of ironfarm's loop: it teleports each arriving cart onto a 6-rail
    approach at a drawn speed, and it places the 10-gt release as a block at the release input.
  - The riders wait in a bench hold (21 carts, over 16 from the bed) made to panic all night by
    hold zombies, as the loop's riders must.
- **Real build, but a bench design.** The approach, the run-out past the exit and the hold are the
  bench's. It does not play in the viewer yet (no drive file).
- **Missing:** ironfarm's two junctions (the entry off the departure run, the merge into the
  release run) and the zombies along the loop.
- **Also kept:** `carousel-siding-bench-driven.txt`, the same siding with the controls switched
  by the bench (set_block): the first bench, before the circuit.

Tools: `mctas mobfarm siding [--wired] --out F`, `siding-check F` (every rider/napper cell's golem
search, the siding's own block count and box), `siding-bench F [--wired] --from --to --nights
--speed a,b --gap a,b --release a,b [--visits N] [--trace --trace-window a,b]`.

## Interface (local frame: x east, z south, y up)

| what | cell | |
|---|---|---|
| entry | (0,0,0) | plain east_west rail; the cart arrives moving +x, 0.10 to 1.2 b/t stored all work (ironfarm's cap ~1.0) |
| release input | (4,-2,0) | the slot slope's support; strongly power it for 10 gt (ironfarm) |
| exit | (6,-1,0) | plain east_west rail; the cart leaves moving +x at 0.28-0.44 b/t stored (median 0.29) |
| box | x -7..6, y -4..4, z -3..9 | 234 blocks (the mechanical part alone: 60) |

Rules for the loop:
- Arrivals at least ~270 gt after the previous visit's arrival; the ≥ 4-release spacing gives 400.
- Every rider panics all night except the napper in the cage.
- No rider calm within 16 of the bed while a napper sleeps: ValidateNearbyPoi forgets the bed for
  good, and the sleeper keeps the ticket.

## How it works

- **Track.**
  - A T at (1,0,0) with its stem south: powered south_west (entry to the bed line), unpowered
    south_east (the bed line to the exit branch).
  - The bed line, from the T south: an always-on booster (1,0,1), so slow arrivals still park
    against the stop; the free cell (1,0,2); the activator (1,0,3); a slow run; the park rail
    (1,0,6) against a cobblestone.
  - The bed: head (0,0,2), foot (0,0,3), facing north, in ironfarm's glass cage. The iron hatch
    is at (2,1,2) and the dorm zombie in a boat at (4.5,1,2.5).
  - The exit branch, from the T east: (2,0,0), then a detector rail at (3,0,0), then the slot (a
    braked powered rail sloping down east at (4,-1,0)), a plain flat, and the exit.
- **Circuit.**
  - The nap is sensed by an observer in the cage wall south of the foot (0,0,4). Its pulse is
    stretched by a 4-tick repeater into an RS-NOR latch west of the siding (y -3).
  - Latch set means a visit is in progress. Torches under the T's support, the activator's
    support and the hatch's control turn the T to the exit branch, turn the activator off (the
    empty cart meets the napper over it) and shut the hatch (the zombie is seen and the napper
    wakes, 16-35 gt later).
  - The latch is reset by the detector rail: only the returning cart passes it. The detector sits
    two rails from the T because its power turned the T under a cart still on it.
  - The kick: a wall torch under the park rail's support on a block BK. BK is fed by "not visit"
    through five 4-tick repeaters (the cart meets the napper 71+ gt after the drop, past the
    60-gt boarding cooldown), OR by a tripwire across the slot (the slot is taken, so the kick
    waits for the release).

## Evidence

Wired, `siding-bench --wired`, 100 seeds × 6 nights:
- Settings: visit gaps 400-600 gt, release 560-610 gt after arrival, arrival speed 0.15-1.1 b/t.
- **15,490 visits**: all dropped, napped, woken, re-boarded their own cart, ready on the slot and
  released. 0 problems.
- Ready 115-290 gt after arrival (median 145, p99 263); 0 over the 500 gt limit.
- Naps last 16-35 gt. The re-board comes 71+ gt after the drop.
- Waiting riders panic 99.50% of night ticks; the only calm ticks are the first ≤ 8 gt after the
  cart stops.
- Exit speed 0.28-0.44 b/t.

The same bench with the controls bench-driven gave 15,490 of 15,490 as well (ready 119-277 gt).

`siding-check`: 0 golem-valid spots in the searches from every rider and napper cell
(entry, T, bed line, free cell, branch, slot, exit).

## What failed on the way (and why)

- **Slow arrivals stopped short of the cobblestone.** The kick then lost the park rail's length,
  and the empty cart met the napper under 0.1 b/t (no pickup). The always-on booster at (1,0,1)
  fixed it for every speed from 0.10 to 1.2.
- **The held riders were calm at REST.** They gave up HOME after 1200 gt out of reach and took
  the bed's one ticket, or backed off 40-400 gt after failed paths. Hold zombies fixed it. A
  released rider teleported back within 16 of the bed while the next napper slept forgot the bed
  for good.
- **The circuit (three bugs, all fixed):**
  - The detector rail beside the T turned it under the cart.
  - Dust beside a block a repeater powered strongly fed the visit line back to itself and latched
    it.
  - Moving the dorm zombie one block east put the sleeping napper's sight line on the shut
    hatch's top, and a nap never ended.
