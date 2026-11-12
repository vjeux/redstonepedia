# Carousel S3 assembly v1: the night run and the brief's checks, as pasted (no repair)

The file: `wip/carousel-s3-v1.txt` on explore/farmopt-assembly (commit 18a5be16f5), the S3 design frame (loop + (6,0,6)).
How it was built and what each part is (wired, stand-in, missing): the file's own header.

## Result in one line

The night failed at the joints between parts, before any part's inside was tested: only the initial batch moved; the corner T
(undriven stub) leaves R1 with 0 landings, the register drains from slot 1, 0 golems in 130 slots, 0 visits by built track.

## Stand-ins on every result below

- the register's advances: runner-emulated (day-1's controller is in the file but its slot feeders were taken out: its register sits
  1-2 cells off S3's approach-5 register);
- the stands' meter, H and head logic: the meter's emulation (`--meter-hold`, field 9 = 2 for B);
- R1's escapement timing: runner-set (`--fifo-intake=6,11 --esc-adm=7,12 --adm-k2 13 --p-always`);
- R1's corner T at loop (9,13,25): its drive is a stub, never driven, so it rests south_west (B's route);
- the golem input: runner-pulsed (frame 93).

## Findings

1. Only the initial batch moved: A's stand released its 3 at slot 0 +14 (occupancy 111; crest +28/+41/+56, gaps 13/15; 3 take-offs
   at x 9, min gap 12 gt), then 0 landings on R1 (the undriven corner T in B's shape).
2. From slot 1 the release group holds 0 carts and the stand 0 (every later REL occupancy 000; queue slots empty): the register drained.
3. 1 cart off the rails at slot 0 +48: cart 19 at design (6.5,1,39.3), register row A at loop z 33, just past R1's tail box: likely
   the joint between R1's tail (r1src-z25, sized for a 16-slot register) and this file's 18-slot register.
4. 0 golems in 130 slots (the stand empty at every summon tick after +15).
5. Visits: 0 complete; night 1: 0 of 21 riders asleep; 0 visits by built track (the W siding's entry and exit join day-1's old loop,
   which S3 and R1 replaced). The runner moved no one. The bench's bed claim at slot 0 +0 hands the one bed to every rider.
6. Router S: 132 shape changes, 0 with a cart in its cell. The 3 liveness clocks run (period 30 gt, 439 edges each).
   0 merges, 0 backward entries (nothing reached the stands to test them); B got no train.
7. Junction check: FAIL at 3 of 5 (router S stored north_south, which re-settles to south_west at its first update; two of day-1's
   NSE junctions at design (0,3,37)/(0,3,40) stored SouthEast with their signal on, which re-settle to NorthEast). `mobfarm paste`
   now settles junctions by default; the file as committed is the unsettled one.
8. Flood: run as a whole farm (the file holds the siding's bed). It stops at its first 6 FAILs at an off-pad spawn spot on B's stand
   roof under the register, design (4.5,-2,33.5): its golems touch a register cart, stand pinned, or walk out of the build. The pad
   portals' golems count as "vanished" in a whole-farm flood (the kill is in the Nether part).
9. Declared states (the unintended-power check): 122 mismatches, most "declared two ways" (the queue options against the header's
   lines, from R1's and the register's different slot counts).
10. Borders (placement at chunk lines): not run (9fc3155f's `mobfarm borders` hasn't landed). Known from its rows: approach 5's dip line
    and A's departure fail the placement rule at z 9|10; the register's crossings at z 25|26 and 41|42 rely on day-1's liveness gate,
    which isn't wired.

## Lessons for the one-chunk build (ironspec Nov 11 16:01)

- Joints first: each interface between parts (switch drives, slot counts, entry and exit cells, who drives what) declared once, with
  an owner.
- The declared-states check passes on the skeleton, with no "declared two ways", before any part is wired in.
- Each joint gets a test on the skeleton as soon as both sides exist: the T's drive, R1's tail against the register's slot count,
  the siding's entry and exit against the loop.
- Paste with the junction-settle default.

## The night run

`mctas mobfarm queue-loop-run wip/carousel-s3-v1.txt --summon-at 93 --release-start 4 --release-stagger 4 --release-offs 42,48,44
--advance-at 32 --advance-every 24 --pulse 10 --pocket-release 14 --meter-hold 12,12,0,0,10,0,40,-1,2,12,0,0,1 --fifo-intake=6,11
--esc-adm=7,12 --adm-k2 13 --p-always --visits-observe --cycles 130 --seed 1` on S3's runner (explore/farmopt-s3 92d039c028), its
whole output:

```
0 settled snapshots, 0 clean 3-place moves
slot-boundary order: 1 clean slots (every queue cart 3 slots on, the tail the stand's last batch, the stand the last train)
the re-packing tail (1 pockets): carts held at the slots' starts 129/1/0/0/0/0/0/0 (0..7); 0 advances found slot 0 empty (a hole into the register)
stand margins: gap12 0 0 in 0 0 clear 0 0 arrive 0 0 slots 0
stand margins at G1: cart 2 reaches G1's face - gt after G1 shuts (slots 0)
departures at the take-off (z 11 on x 9): 2 gaps under 30 gt, 4..6 gt: 0; 7 gt (the band's edge, 6.75, inside a tick): 0; min 12
trains at G2's plane (slot +gt of the first crossing: gaps gt): 0+33:
approach arrivals (129 slots): cart 1 at G2 - G1 - G0 -; cart 2 at G2 - G1 -; cart 3 at G2 -
meter (S3(ii)'s hold, emulated): 65 releases, 0 trains metered in; events: 0+0 S north_south, 0+2 S north_west, 0+14 REL (occupancy 111), 0+36 SE, 1+2 S south_west, 2+2 S north_west, 2+14 REL (occupancy 000), 3+2 S south_west, 4+14 REL (occupancy 000), 6+14 REL (occupancy 000), 8+14 REL (occupancy 000), 10+14 REL (occupancy 000), 12+14 REL (occupancy 000), 14+14 REL (occupancy 000), 16+14 REL (occupancy 000), 18+14 REL (occupancy 000), 20+14 REL (occupancy 000), 22+14 REL (occupancy 000), 24+14 REL (occupancy 000), 26+14 REL (occupancy 000), 28+14 REL (occupancy 000), 30+14 REL (occupancy 000), 32+14 REL (occupancy 000), 34+14 REL (occupancy 000), 36+14 REL (occupancy 000), 38+14 REL (occupancy 000), 40+14 REL (occupancy 000), 42+14 REL (occupancy 000), 44+14 REL (occupancy 000), 46+14 REL (occupancy 000), 48+14 REL (occupancy 000), 50+14 REL (occupancy 000), 52+14 REL (occupancy 000), 54+14 REL (occupancy 000), 56+14 REL (occupancy 000), 58+14 REL (occupancy 000), 60+14 REL (occupancy 000), 62+14 REL (occupancy 000), 64+14 REL (occupancy 000), 66+14 REL (occupancy 000)
meter Dh 3 (the stand's 3rd cart in): gt before its batch's golem summon tick (the last before the stand's next REL) -
meter events after the first 40: 68+14 REL (occupancy 000), 70+14 REL (occupancy 000), 72+14 REL (occupancy 000), 74+14 REL (occupancy 000), 76+14 REL (occupancy 000), 78+14 REL (occupancy 000), 80+14 REL (occupancy 000), 82+14 REL (occupancy 000), 84+14 REL (occupancy 000), 86+14 REL (occupancy 000), 88+14 REL (occupancy 000), 90+14 REL (occupancy 000), 92+14 REL (occupancy 000), 94+14 REL (occupancy 000), 96+14 REL (occupancy 000), 98+14 REL (occupancy 000), 100+14 REL (occupancy 000), 102+14 REL (occupancy 000), 104+14 REL (occupancy 000), 106+14 REL (occupancy 000), 108+14 REL (occupancy 000), 110+14 REL (occupancy 000), 112+14 REL (occupancy 000), 114+14 REL (occupancy 000), 116+14 REL (occupancy 000), 118+14 REL (occupancy 000), 120+14 REL (occupancy 000), 122+14 REL (occupancy 000), 124+14 REL (occupancy 000), 126+14 REL (occupancy 000), 128+14 REL (occupancy 000)
meter margins: H closes, gt until a cart touches its face -; Dh edges, gt since Dh was last off -; G0 shut after the last departing cart cleared it -; the first cart at G0's face after G0 shut -, at G1's after G1 shut -, at G2's after G2 shut -
meter H per opening: the released cart's Dh edge after the opening - gt; the follower short of H's plane when H shut - blocks (ironfarm's escapement: >= 0.66); carts through H per opening 0: 1, 1: 0, 2+: 0
meter H's fixed-opening window per opening (gt after the opening: H's cell clear of the released cart .. the follower within 0.66 of the plane): ; common window 0..0 (0 gt)
meter spacing (consecutive rising edges under 100 gt): Dh -, Darr -, D1 -, D2 -, crest 28..28 (n 1)
meter H shuts: carts in H's box at the shut 0/1/2+: 0/0/0
meter T margin (T's cell from z 4): held carts' rest z at each H1 -; backward crossings into T's cell: 0 ()
meter T bar (box edges = centre + 0.49, T's cell from z 4): farthest backward box edge in the reversal rows -; stopped carts on the descent: 0 stops, the highest box edge -; the longest stopped stack 0 carts; departing carts back through D2: 0; SE fired 1 times
meter backward entries through a stand gate or H: 0; retry openings: 0
meter H's frozen-cart chunk check (a cart in or next to H's cell, its box against the farm's chunk lines x = -4 + 16k, z = 10 + 16k; 0 cart-ticks): least clearance A -, B (A + 0,-27,+32) -
meter the stand's summoning moment (the run's world-time multiples of 100 before this stand's RELs): each train's Dh3 against it (gt before it; negative: late): ; carts at rest in the pockets at each: 1+94 0/3, 3+94 0/3, 5+94 0/3, 7+94 0/3, 9+94 0/3, 11+94 0/3, 13+94 0/3, 15+94 0/3, 17+94 0/3, 19+94 0/3, 21+94 0/3, 23+94 0/3, 25+94 0/3, 27+94 0/3, 29+94 0/3, 31+94 0/3, 33+94 0/3, 35+94 0/3, 37+94 0/3, 39+94 0/3, 41+94 0/3, 43+94 0/3, 45+94 0/3, 47+94 0/3, 49+94 0/3, 51+94 0/3, 53+94 0/3, 55+94 0/3, 57+94 0/3, 59+94 0/3, 61+94 0/3, 63+94 0/3, 65+94 0/3, 67+94 0/3, 69+94 0/3, 71+94 0/3, 73+94 0/3, 75+94 0/3, 77+94 0/3, 79+94 0/3, 81+94 0/3, 83+94 0/3, 85+94 0/3, 87+94 0/3, 89+94 0/3, 91+94 0/3, 93+94 0/3, 95+94 0/3, 97+94 0/3, 99+94 0/3, 101+94 0/3, 103+94 0/3, 105+94 0/3, 107+94 0/3, 109+94 0/3, 111+94 0/3, 113+94 0/3, 115+94 0/3, 117+94 0/3, 119+94 0/3, 121+94 0/3, 123+94 0/3, 125+94 0/3, 127+94 0/3, 129+94 0/3
meter router S (the switch rule: never a shape change with a cart's box in S's cell): 132 changes, 0 with a cart in the cell
meter P2 (a REL held while a cart is in transit from H to its count edge; the latch's live-time timeout): 0 RELs held, 0 transit timeouts
meter clock 1 (3,-13,2): 439 rising edges, period 17..30 gt, on 20.0 gt per edge, first edges 0+0, 0+17, 0+47, 0+77, 1+7, 1+37
meter clock 2 (12,-14,22): 439 rising edges, period 30..30 gt, on 20.0 gt per edge, first edges 0+2, 0+32, 0+62, 0+92, 1+22, 1+52
meter clock 3 (3,-18,38): 439 rising edges, period 17..30 gt, on 20.0 gt per edge, first edges 0+0, 0+17, 0+47, 0+77, 1+7, 1+37
meter departure at the crest (each REL: carts' crossing gt after REL, then the gaps): +28/+41/+56 gaps 13,15
meter DEP_X probe (a detector at (xb,1,14) on A's branch, the centre crossing z 15.29 northward; gt after the cycle's tick, each cart): 
meter speed rule (a moving cart's stored speed the gt before it enters a resting cart's push range: centres within 1.18 across, 1.0 up): 2-3 slopes: 2 contacts, fastest 0.147; 1 slope: 3 contacts, fastest 0.108; flat: 1 contacts, fastest 0.000; 0 at H's queue (fastest there 0.000); the fastest cart 20 onto 19 at 0+66 stored 0.147 (2-3 slopes) closest 1.131, the resting cart at 0.50,1.00,33.20; cart 19 onto 0 at 0+90 stored 0.108 (1 slope) closest 1.223, the resting cart at 0.50,0.58,34.49; cart 20 onto 19 at 1+19 stored 0.049 (1 slope) closest 1.368, the resting cart at 0.50,1.00,33.47; cart 20 onto 18 at 2+61 stored 0.005 (1 slope) closest 1.117, the resting cart at 0.50,2.18,31.88; cart 19 onto 0 at 1+44 stored 0.000 (flat) closest 1.066, the resting cart at 0.50,0.28,34.78; closest centres 1.066
meter H arrivals (stored speed the gt before contact, by rest place and pair; R ridden, E empty, mover then resting cart): -
meter hold arrivals (reversals toward T on the descent): 0 rows; stored speed at the reversal up to 0.000, visible up to 0.000; contact z 99.00..0.00; back travel up to 0.000 blocks, the farthest back z 0.00 (0 rows reach z 4); cell crossings back up to 0
meter arrivals at a waiting cart (first contact on the hold's column: a push, or a dead stop at a rider's box): 0 rows (0 onto an empty cart); stored speed before contact up to 0.000 (onto empty carts 0.000, onto ridden 0.000); closest centres -; merged 0; the worst: 
meter arrivals by rest place: place 1: none; place 2: none; place 3: none; each arrival's stored speed: -
B: meter (S3(ii)'s hold, emulated): 65 releases, 0 trains metered in; events: 1+26 REL (occupancy 000), 3+26 REL (occupancy 000), 5+26 REL (occupancy 000), 7+26 REL (occupancy 000), 9+26 REL (occupancy 000), 11+26 REL (occupancy 000), 13+26 REL (occupancy 000), 15+26 REL (occupancy 000), 17+26 REL (occupancy 000), 19+26 REL (occupancy 000), 21+26 REL (occupancy 000), 23+26 REL (occupancy 000), 25+26 REL (occupancy 000), 27+26 REL (occupancy 000), 29+26 REL (occupancy 000), 31+26 REL (occupancy 000), 33+26 REL (occupancy 000), 35+26 REL (occupancy 000), 37+26 REL (occupancy 000), 39+26 REL (occupancy 000), 41+26 REL (occupancy 000), 43+26 REL (occupancy 000), 45+26 REL (occupancy 000), 47+26 REL (occupancy 000), 49+26 REL (occupancy 000), 51+26 REL (occupancy 000), 53+26 REL (occupancy 000), 55+26 REL (occupancy 000), 57+26 REL (occupancy 000), 59+26 REL (occupancy 000), 61+26 REL (occupancy 000), 63+26 REL (occupancy 000), 65+26 REL (occupancy 000), 67+26 REL (occupancy 000), 69+26 REL (occupancy 000), 71+26 REL (occupancy 000), 73+26 REL (occupancy 000), 75+26 REL (occupancy 000), 77+26 REL (occupancy 000), 79+26 REL (occupancy 000)
B: meter Dh 3 (the stand's 3rd cart in): gt before its batch's golem summon tick (the last before the stand's next REL) -
B: meter events after the first 40: 81+26 REL (occupancy 000), 83+26 REL (occupancy 000), 85+26 REL (occupancy 000), 87+26 REL (occupancy 000), 89+26 REL (occupancy 000), 91+26 REL (occupancy 000), 93+26 REL (occupancy 000), 95+26 REL (occupancy 000), 97+26 REL (occupancy 000), 99+26 REL (occupancy 000), 101+26 REL (occupancy 000), 103+26 REL (occupancy 000), 105+26 REL (occupancy 000), 107+26 REL (occupancy 000), 109+26 REL (occupancy 000), 111+26 REL (occupancy 000), 113+26 REL (occupancy 000), 115+26 REL (occupancy 000), 117+26 REL (occupancy 000), 119+26 REL (occupancy 000), 121+26 REL (occupancy 000), 123+26 REL (occupancy 000), 125+26 REL (occupancy 000), 127+26 REL (occupancy 000), 129+26 REL (occupancy 000)
B: meter margins: H closes, gt until a cart touches its face -; Dh edges, gt since Dh was last off -; G0 shut after the last departing cart cleared it -; the first cart at G0's face after G0 shut -, at G1's after G1 shut -, at G2's after G2 shut -
B: meter H per opening: the released cart's Dh edge after the opening - gt; the follower short of H's plane when H shut - blocks (ironfarm's escapement: >= 0.66); carts through H per opening 0: 1, 1: 0, 2+: 0
B: meter H's fixed-opening window per opening (gt after the opening: H's cell clear of the released cart .. the follower within 0.66 of the plane): ; common window 0..0 (0 gt)
B: meter spacing (consecutive rising edges under 100 gt): Dh -, Darr -, D1 -, D2 -, crest -
B: meter H shuts: carts in H's box at the shut 0/1/2+: 0/0/0
B: meter T margin (T's cell from z 4): held carts' rest z at each H1 -; backward crossings into T's cell: 0 ()
B: meter T bar (box edges = centre + 0.49, T's cell from z 4): farthest backward box edge in the reversal rows -; stopped carts on the descent: 0 stops, the highest box edge -; the longest stopped stack 0 carts; departing carts back through D2: 0; SE fired 0 times
B: meter backward entries through a stand gate or H: 0; retry openings: 0
B: meter H's frozen-cart chunk check (a cart in or next to H's cell, its box against the farm's chunk lines x = -4 + 16k, z = 10 + 16k; 0 cart-ticks): least clearance A -, B (A + 0,-27,+32) -
B: meter the stand's summoning moment (the run's world-time multiples of 100 before this stand's RELs): each train's Dh3 against it (gt before it; negative: late): ; carts at rest in the pockets at each: 1+6 0/3, 3+6 0/3, 5+6 0/3, 7+6 0/3, 9+6 0/3, 11+6 0/3, 13+6 0/3, 15+6 0/3, 17+6 0/3, 19+6 0/3, 21+6 0/3, 23+6 0/3, 25+6 0/3, 27+6 0/3, 29+6 0/3, 31+6 0/3, 33+6 0/3, 35+6 0/3, 37+6 0/3, 39+6 0/3, 41+6 0/3, 43+6 0/3, 45+6 0/3, 47+6 0/3, 49+6 0/3, 51+6 0/3, 53+6 0/3, 55+6 0/3, 57+6 0/3, 59+6 0/3, 61+6 0/3, 63+6 0/3, 65+6 0/3, 67+6 0/3, 69+6 0/3, 71+6 0/3, 73+6 0/3, 75+6 0/3, 77+6 0/3, 79+6 0/3, 81+6 0/3, 83+6 0/3, 85+6 0/3, 87+6 0/3, 89+6 0/3, 91+6 0/3, 93+6 0/3, 95+6 0/3, 97+6 0/3, 99+6 0/3, 101+6 0/3, 103+6 0/3, 105+6 0/3, 107+6 0/3, 109+6 0/3, 111+6 0/3, 113+6 0/3, 115+6 0/3, 117+6 0/3, 119+6 0/3, 121+6 0/3, 123+6 0/3, 125+6 0/3, 127+6 0/3, 129+6 0/3, 131+6 0/3
B: meter router S (the switch rule: never a shape change with a cart's box in S's cell): 0 changes, 0 with a cart in the cell
B: meter P2 (a REL held while a cart is in transit from H to its count edge; the latch's live-time timeout): 0 RELs held, 0 transit timeouts
B: meter departure at the crest (each REL: carts' crossing gt after REL, then the gaps): 
B: meter DEP_X probe (a detector at (xb,1,14) on A's branch, the centre crossing z 15.29 northward; gt after the cycle's tick, each cart): 
B: meter speed rule (a moving cart's stored speed the gt before it enters a resting cart's push range: centres within 1.18 across, 1.0 up): 2-3 slopes: 2 contacts, fastest 0.147; 1 slope: 3 contacts, fastest 0.108; flat: 1 contacts, fastest 0.000; 0 at H's queue (fastest there 0.000); the fastest cart 20 onto 19 at 0+66 stored 0.147 (2-3 slopes) closest 1.131, the resting cart at 2.50,9.00,2.80; cart 19 onto 0 at 0+90 stored 0.108 (1 slope) closest 1.223, the resting cart at 2.50,8.58,1.51; cart 20 onto 19 at 1+19 stored 0.049 (1 slope) closest 1.368, the resting cart at 2.50,9.00,2.53; cart 20 onto 18 at 2+61 stored 0.005 (1 slope) closest 1.117, the resting cart at 2.50,10.18,4.12; cart 19 onto 0 at 1+44 stored 0.000 (flat) closest 1.066, the resting cart at 2.50,8.28,1.22; closest centres 1.066
B: meter H arrivals (stored speed the gt before contact, by rest place and pair; R ridden, E empty, mover then resting cart): -
B: meter hold arrivals (reversals toward T on the descent): 0 rows; stored speed at the reversal up to 0.000, visible up to 0.000; contact z 99.00..0.00; back travel up to 0.000 blocks, the farthest back z 0.00 (0 rows reach z 4); cell crossings back up to 0
B: meter arrivals at a waiting cart (first contact on the hold's column: a push, or a dead stop at a rider's box): 0 rows (0 onto an empty cart); stored speed before contact up to 0.000 (onto empty carts 0.000, onto ridden 0.000); closest centres -; merged 0; the worst: 
B: meter arrivals by rest place: place 1: none; place 2: none; place 3: none; each arrival's stored speed: -
stand braked at - after its slot's start; tail slots 1-2 braked at -, slot 0 at - after the train's departure
zombies alive at the end: 15
PANIC at night (day time 13,670..22,330, 181860 rider-ticks): 90.48% of rider-ticks, the least-panicking rider 0.00%; calm most at 3 0 35 (8660), 14 -1 40 (8660)
the pad: 0 golems over 130 slots (0 slots with a golem: 0.00%), summoned at slot +0..+0; 0 through the portal, 0 died in the Nether (0 alive at the end); pulses at 15,6,4: 0 with a golem, 0 without; iron into the Nether part's containers: +0 ingots
summons onto the pad out of turn (the summoner not on a stand cell or its pocket): 0 of 0
pad stats: golems 0 with 0 slots 130 crossed 0 died 0 alive 0 pulses 0 spurious 0 iron 0 out_of_turn 0
summon ticks: stand riders willing 0/1/2/3: 131 0 0 0; panicking 0/1/2/3: 131 0 0 0; riders at the stand 0/1/2/3: 131 0 0 0; unwilling: never slept 0, slept 24,000 gt ago or more 0, a golem seen lately 0
summon stats: 131 0 0 0 131 0 0 0 131 0 0 0 0 0 0
power-on: day time at the start (as the bench's); the first golem at +never gt; the first release (G0 open) at +15 gt; all releases before the first locked one (G0 open 1..20 gt after the pad's pulse), 0 of 1 releases locked
power stats: -1 -1 15 -1 0 1
riders panicking on the summon tick (world time a multiple of 100): 2490 of 2751 rider-ticks (90.5%); not panicking (no summon from them): 261 (9.5%)
riders walking with a reachable path while their cart touches another (they push it): 20355 rider-ticks (first: rider e19 slot 0 +83 at 6.50,1.00,39.24; rider e19 slot 0 +84 at 6.50,1.00,39.24; rider e19 slot 0 +85 at 6.50,1.00,39.24; rider e19 slot 0 +86 at 6.50,1.00,39.24)
cant-reach: the oldest CANT_REACH_WALK_TARGET_SINCE where HOME's release can run (REST, no WALK_TARGET, HOME held) 1308 gt (rider e12 at 3.50,0.74,35.68, slot 12 +3); margin to 1200: -108; HOME releases: 2 (rider e12 slot 12 +4; rider e17 slot 17 +65)
home log: 2 changes: e12 lost at slot 12 +4; e17 lost at slot 17 +65
bed tickets: 6,-21,23 1 free at the start
stamp erasures (a rider's CANT_REACH gone: the walk it had the tick before): 537 (another walk in panic: 519, the oldest 1441 gt; another walk in rest: 18, the oldest 2052 gt); stamps set: in idle 2, in panic 523, in rest 17
stamp erasures by cell, the 16 most: 14 -2 39: 220 (another walk in panic 220); 6 1 39: 28 (another walk in panic 28); 3 0 26: 27 (another walk in panic 27); 3 0 29: 27 (another walk in panic 27); 6 1 38: 27 (another walk in panic 27); 6 0 52: 26 (another walk in panic 26); 6 0 43: 25 (another walk in panic 25); 3 0 38: 24 (another walk in panic 24); 6 0 40: 24 (another walk in panic 24); 6 0 49: 24 (another walk in panic 24); 6 0 46: 21 (another walk in panic 21); 6 2 38: 18 (another walk in panic 18); 3 0 35: 17 (another walk in rest 17); 5 0 54: 15 (another walk in panic 15); 3 0 41: 7 (another walk in panic 7); 3 0 32: 3 (another walk in panic 3)
stamps' oldest age per rider (any activity): the oldest 9306 gt (OVER the bar: the bar is 1,000); the 8 oldest: e17 9306 gt (at 14.5,-0.7,40.3 slot 131 +49 in idle); e18 1440 gt (at 6.5,2.0,38.0 slot 69 +47 in panic); e12 1308 gt (at 3.5,0.7,35.7 slot 12 +3 in rest); e19 871 gt (at 6.5,1.0,40.0 slot 56 +15 in panic); e15 857 gt (at 3.5,0.7,26.7 slot 71 +33 in panic); e20 737 gt (at 6.5,1.2,38.8 slot 99 +34 in panic); e14 642 gt (at 3.5,0.7,29.7 slot 124 +3 in panic); e1 500 gt (at 6.5,0.7,43.3 slot 98 +61 in panic)
near release (REST, HOME held, a CANT_REACH stamp >= 1,000 gt, a WALK_TARGET or not): 364 rider-ticks, at most 1 at once (slot 11 +32: e12 at 3.5,0.7,35.7 (1237 gt, walk target set)); by rider: e17 292, e12 72; HOME releases: 2
freeze exposure (deep night: calm in REST, a live CANT_REACH streak, HOME held): at most 0 at once; ticks with 2 or more: 0; the most exposed: e0 0.00%, e1 0.00%, e2 0.00%, e3 0.00%, e4 0.00%, e5 0.00%
freeze exposure map (block cell: exposed rider-ticks, the longest streak, the share panicking within 20 gt before), the 16 most: -
golem locks outside the summoning group (GOLEM_DETECTED_RECENTLY, 599 gt, set on a rider off the stand: a summon's 10-box or its own golem sensor): 0 (10-box 0, sensor 0); stand turns lost to them (a rider at the stand at a summon tick still so locked): 0
rider pushes on other carts (counted at the push: a rider's LivingEntity.pushEntities moving a cart not its own, AbstractMinecart.push past its 0.01): 5567 (by the pusher's walk state: reaching 3645, partial 1506, a walk target with no path 416, no walk target 0); the largest 0.0500 b/t; the most in one tick 1 (slot 1 +59: e17 at 14.50,-0.66,40.28 on cart #41 (0.788 apart, reaching)); the cells: 6 0 40: 5566; 14 -1 40: 1; other mobs' pushes on carts 0
the rider pushes' harm (per pushed cart from the push: its displacement along the push; an episode with a cart-on-cart impact in it is the impact's, counted apart): 1891 episodes; the peak 3.515 b (slot 1 +82: cart #41 at 15.50,5.96,41.10); back within 23 gt at most, 0 not back within 200 gt; 1 crossed a cell boundary backward (slot 1 +82: cart #41); 0 episodes with an impact (see the arrivals)
riders touching another cart's box (each pushes that cart away every tick, walking or not; LivingEntity.pushEntities): 0 rider-ticks (following a reaching path 0, a partial path 0, a walk target with no path 0, no walk target 0); within 0.2 of one: 38913 (reaching 14146, partial 11368, no path 8384, no walk 5015); the most touching at once 0; the cells with the most touching ticks: -; riders' walk states, every tick: reaching 145588, partial 68152, a walk target with no path 45646, no walk target 16764
visit: bed claims: e0 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,0.7,40.3; e1 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,0.7,43.3; e2 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,0.7,46.3; e3 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,0.7,49.3; e4 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,0.7,52.3; e5 took the bed (6,-21,23) at slot 0 +0 riding at 5.7,0.7,54.5; e6 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,53.7; e7 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,50.7; e8 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,47.7; e9 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,44.7; e10 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,41.7; e11 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,38.7; e12 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,35.7; e13 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,32.7; e14 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,29.7; e15 took the bed (6,-21,23) at slot 0 +0 riding at 3.5,0.7,26.7; e16 took the bed (6,-21,23) at slot 0 +0 riding at 13.9,-1.8,6.5; e17 took the bed (6,-21,23) at slot 0 +0 riding at 9.9,-0.8,6.5; e18 took the bed (6,-21,23) at slot 0 +0 riding at 5.9,0.2,6.5; e19 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,2.2,37.9; e20 took the bed (6,-21,23) at slot 0 +0 riding at 6.5,3.2,36.9; e12 let its bed go at slot 12 +4 riding at 3.5,0.7,35.7; e17 let its bed go at slot 17 +65 riding at 14.5,-0.7,40.3
bed: never taken
visits: 0 complete; night 1: 0 of 21 riders asleep, 0 visits
carts and riders: 21 and 21 at the start, 21 and 21 at the end (21 seated)
merged carts (horizontal distance under 0.01, boxes paired by the push; the game never separates them): 0 
curve spacing (G1's bar: each curve's passages, the follower's box front at the cell's near edge, its centre within 0.99 of the curve's centre, minus the leader's clear, its centre 0.98 past along the exit; least first): (9,-3,0) 3 passes, least margin 8 gt at t 29 (leader 0.35, follower 0.63 b/gt); (8,3,38) 4 passes, least margin 9 gt at t 123 (leader 1.99, follower 1.99 b/gt); (9,3,38) 4 passes, least margin 9 gt at t 121 (leader 2.07, follower 2.07 b/gt); (9,13,25) 4 passes, least margin 9 gt at t 90 (leader 1.85, follower 2.02 b/gt); (-5,-18,31) 438 passes, least margin 21 gt at t 13 (leader 0.23, follower 0.23 b/gt); (-5,-13,-5) 438 passes, least margin 21 gt at t 13 (leader 0.23, follower 0.23 b/gt); (-3,-18,33) 438 passes, least margin 21 gt at t 28 (leader 0.23, follower 0.23 b/gt); (-3,-13,-3) 438 passes, least margin 21 gt at t 28 (leader 0.23, follower 0.23 b/gt); (6,-14,15) 438 passes, least margin 21 gt at t 13 (leader 0.23, follower 0.23 b/gt); (8,-14,17) 438 passes, least margin 21 gt at t 28 (leader 0.23, follower 0.23 b/gt); (-5,-18,33) 439 passes, least margin 24 gt at t 4 (leader 0.38, follower 0.28 b/gt); (-5,-13,-3) 439 passes, least margin 24 gt at t 4 (leader 0.38, follower 0.28 b/gt); (-3,-18,31) 438 passes, least margin 24 gt at t 19 (leader 0.28, follower 0.28 b/gt); (-3,-13,-5) 438 passes, least margin 24 gt at t 19 (leader 0.28, follower 0.28 b/gt); (6,-14,17) 439 passes, least margin 24 gt at t 4 (leader 0.38, follower 0.28 b/gt); (8,-14,15) 438 passes, least margin 24 gt at t 19 (leader 0.28, follower 0.28 b/gt)
curve contact (ticks on which two carts' boxes touch, both centres within 1.5 of a curve's centre; must be 0): 0
riders in the pressed queue (landing to P) with a walk target, rider-ticks by path: reaching 27527, partial 25918, no path 10394 (slot 0 +0: cart 0 at z 34.32 partial; slot 0 +0: cart 1 at z 37.32 reaching; slot 0 +0: cart 19 at z 31.89 partial; slot 0 +1: cart 0 at z 34.32 partial; slot 0 +1: cart 1 at z 37.32 reaching; slot 0 +2: cart 1 at z 37.32 reaching; slot 0 +7: cart 0 at z 34.32 no path; slot 0 +8: cart 0 at z 34.32 no path; slot 0 +8: cart 1 at z 37.32 no path); riders whose box touches another cart's, walking or not (contact alone pushes: LivingEntity.pushEntities, bridged in gate-escg6l-v3; must be 0): 0 (not walking 0)
backward on P while it is powered (admission control): 0 ticks
admission margins: the fastest front at a G0 opening -1.000 b/t, 0.000 short of G0's box (); the launched cart's rider and the next cart on P at least - apart (); the admitted cart's front reached P's end at least - gt after the launched cart's rider left G_P's plane ()
detector edges against carts entering (equal in normal flow for every edge-latched detector): 2 detectors, 1 short: -3,4,6 0:0, 6,-1,0 1:2 SHORT
the jump onto the landing (the bar: 5 gt apart): 0 landings, the closest 0 gt apart, 0 under 5 gt; landings with a cart at rest on the landing cells: 0
arrivals (a landed cart's first impact): 0 (met a cart pressed at G0 0, rolling 0, at rest short of G0 0, on P 0); the free roll 0.00..0.00 cells; the hardest impact 0.000 b/t closing; impacts in all 0
carts pushed back across a cell boundary in the tail (the bar: 0, whatever the cause): 0
the front at each G0 opening (gt pressed at G0 before it opens, negative when it reached G0 after; the bar: 2): 390 openings, the least -10 gt (slot 2 +45); 1 under 2 (slot 2 +45: the front reached G0 10 gt after it opened); 388 with the queue empty, 0 whose front never reached G0 within 40 gt
the queue's occupancy: at most 1 carts downstream of the curve (slot 0 +0) and 1 upstream (slot 2 +40)
each batch's carts at the openings that admit them (the bar: pressed at G0 2 gt before): 0 batches, the least -; 0 carts under 2; 0 not admitted by the end
launches into a full slot 0 (G_P opening while a cart rests on slot 0's cell): 8 (slot 0 +62, slot 0 +86, slot 1 +38, slot 1 +86)
the x-queue's flats (curve and transition cell): the longest rest there 0 gt
detector timing (off -> next cart's entry, the edge margin; merged = entered while still on): -3,4,6 min gap - merged 0, 6,-1,0 min gap - merged 1
train gaps at the plane x 0.5 (z 0.5, y 0.0; crossings eastward, gt): 1 trains (1x1)
stand batch panicking on the slot tick (3 at rest): 3 in 1, 2 in 0, 1 in 0, 0 in 0 of 1 slots
declared states (rails, gates, boosters as the options declare them; tick 0 and boosters at t 20): 122 mismatches (6,0,40: declared two ways: 6,0,40 rail (options) and 6,0,40 powered_rail[powered=false]<6,-1,40 (header); 6,0,43: declared two ways: 6,0,43 rail (options) and 6,0,43 powered_rail[powered=false]<6,-1,43 (header); 6,0,46: declared two ways: 6,0,46 rail (options) and 6,0,46 powered_rail[powered=false]<6,-1,46 (header); 6,0,49: declared two ways: 6,0,49 rail (options) and 6,0,49 powered_rail[powered=false]<6,-1,49 (header); 6,0,52: declared two ways: 6,0,52 rail (options) and 6,0,52 powered_rail[powered=false]<6,-1,52 (header); 3,0,52: declared two ways: 3,0,52 powered_rail[powered=false]<3,-1,52 (options) and 3,0,52 rail (header))
stand departures: 3 carts, gap min 13 max 16 gt (gaps under 60 gt), 0 in contact with the one before; most in one window 3
departure corner (9,-3,0) entered: cart 0 +25..+25 (margins 4/5), cart 1 +38..+38 (margins 7/2), cart 2 +54..+54 (margins 13/-4)
back through a gate: 0
carts off the rails: 1 (slot 0 +48: cart 19 off the rails (6.50,1.00,39.31)) (1 declared flights landed as declared); riders not seated: 0
carts at rest on T: 0 (a 0, b 0, c 0, contact 0, other 0)
climb reversals: 0
meetings on the climb: 0 (slowest met -); on T: 0 (slowest met -); below T on the descent: 0 (slowest met -)
meetings by the met cart's place: flat run below the climb 0 (slowest -), climb 0 (slowest -), T 0 (slowest -), descent 0 (slowest -), hump-down 0 (slowest -), hump-up 0 (slowest -), run 0 (slowest -)
meetings on the climb, on T, below T by cause (frozen/over T/head-on/stalled/catch-up/other, when in a freeze/after a thaw/other): climb 0/0/0/0/0/0 when 0/0/0; T 0/0/0/0/0/0 when 0/0/0; descent 0/0/0/0/0/0 when 0/0/0
meetings by cause (frozen/over T/head-on/stalled/catch-up/other, when in a freeze/after a thaw/other): flat 0/0/0/0/0/0 when 0/0/0; climb 0/0/0/0/0/0 when 0/0/0; T 0/0/0/0/0/0 when 0/0/0; descent 0/0/0/0/0/0 when 0/0/0; hump-down 0/0/0/0/0/0 when 0/0/0; hump-up 0/0/0/0/0/0 when 0/0/0; run 0/0/0/0/0/0 when 0/0/0
carts on the release run upstream of the line z 9|10 at once: at most 2
pushed back over T (backward crossings onto the climb): 0; fastest: stored 0.000, visible 0.000
hump: highest backward climb on its falling face 0.00 above the dip (h0: no hump; its top is S's flat at the run's level, below the dip, and beyond it the run's boosters); backward onto its top or beyond: 0
approach profile (forward crossings: +gt after the slot's start, stored and visible b/t): G2 x 0.5 by train place: 1st +133..+133
approach profile summary (plane:place:stored min:max:+gt min:max): G2:1:0:0:133:133
contacts at a waiting cart by its place (a mover's centre within 1.0 horizontally of a cart under 0.01 b/t or frozen): none
waiting-cart contact summary (n/stored max/at 0.6+/at 1.0+/closest/onto an empty cart max/merged): flat 0/0.000/0/0/9.000/0.000/0; climb 0/0.000/0/0/9.000/0.000/0; T 0/0.000/0/0/9.000/0.000/0; descent 0/0.000/0/0/9.000/0.000/0; hump-down 0/0.000/0/0/9.000/0.000/0; hump-up 0/0.000/0/0/9.000/0.000/0; run 0/0.000/0/0/9.000/0.000/0
backward travel on the approach: largest 0.00 blocks; fastest backward: stored 0.000, visible 0.000 b/t; backward onto the run's boosters (z >= 20): 0
departure: meetings in the dip: 0 (slowest met -); reversals onto a slope: 0; meetings on the rise: 0 (slowest met -)
latch sets: 0 with REL off in 0 slots (1 more under REL, overridden); out of turn (REL off, before +55) 0; more than one with REL off in a slot 0
T3 (the guard's set: C's timer or C') lit 1 times at +0..+0; slots with more than one 0
divert corner (departures of 3, 1 slots): cart 0 in +25 out +27 (from G0's opening: in 11 out 13); cart 1 in +38 out +40 (from G0's opening: in 24 out 26); cart 2 in +54 out +56 (from G0's opening: in 40 out 42)
stand bursts: 3x1 (size x count); the biggest: 3 carts (slot 0 +25..+54, gaps 13/16, top 1.04/1.11/1.27)
problem slots: 0x1 1x1 2x2 3x2 4x2 5x2 6x2 7x2 8x2 9x2 10x2 11x2 12x2 13x2 14x2 15x2 16x2 17x2 18x2 19x2 20x2 21x2 22x2 23x2 24x2 25x2 26x2 27x2 28x2 29x2 30x2 31x2 32x2 33x2 34x2 35x2 36x2 37x2 38x2 39x2 40x2 41x2 42x2 43x2 44x2 45x2 46x2 47x2 48x2 49x2 50x2 51x2 52x2 53x2 54x2 55x2 56x2 57x2 58x2 59x2 60x2 61x2 62x2 63x2 64x2 65x2 66x2 67x2 68x2 69x2 70x2 71x2 72x2 73x2 74x2 75x2 76x2 77x2 78x2 79x2 80x2 81x2 82x2 83x2 84x2 85x2 86x2 87x2 88x2 89x2 90x2 91x2 92x2 93x2 94x2 95x2 96x2 97x2 98x2 99x2 100x2 101x2 102x2 103x2 104x2 105x2 106x2 107x2 108x2 109x2 110x2 111x2 112x2 113x2 114x2 115x2 116x2 117x2 118x2 119x2 120x2 121x2 122x2 123x2 124x2 125x2 126x2 127x2 128x2 129x2
261 problems:
  slot 0: the release group holds 0 carts and the stand 3 at the slot's start
  slot 1: the release group holds 0 carts and the stand 0 at the slot's start
  slot 2: the release group holds 0 carts and the stand 0 at the slot's start
  slot 2: order: queue slot 3 holds None, not Some(68); queue slot 4 holds None, not Some(39); queue slot 5 holds None, not Some(37)
  slot 3: the release group holds 0 carts and the stand 0 at the slot's start
  slot 3: order: queue slot 3 holds None, not Some(68)
  slot 4: the release group holds 0 carts and the stand 0 at the slot's start
  slot 4: order: queue slot 3 holds None, not Some(68)
  slot 5: the release group holds 0 carts and the stand 0 at the slot's start
  slot 5: order: queue slot 3 holds None, not Some(68)
  slot 6: the release group holds 0 carts and the stand 0 at the slot's start
  slot 6: order: queue slot 3 holds None, not Some(68)
  slot 7: the release group holds 0 carts and the stand 0 at the slot's start
  slot 7: order: queue slot 3 holds None, not Some(68)
  slot 8: the release group holds 0 carts and the stand 0 at the slot's start
  slot 8: order: queue slot 3 holds None, not Some(68)
  slot 9: the release group holds 0 carts and the stand 0 at the slot's start
  slot 9: order: queue slot 3 holds None, not Some(68)
  slot 10: the release group holds 0 carts and the stand 0 at the slot's start
  slot 10: order: queue slot 3 holds None, not Some(68)
```

## The junction check: `mctas mobfarm junction-check wip/carousel-s3-v1.txt` (main)

```
rail junctions (static, the game's settle order): 5 junctions, FAIL at 3
  (0,3,37) (arms NSE): stored SouthEast; the game settles SouthEast unpowered, NorthEast powered; signal on; can be powered through: the block (0,2,37) (Repeater at (-1,2,37)): re-settles to NorthEast at its first update
  (0,3,40) (arms NSE): stored SouthEast; the game settles SouthEast unpowered, NorthEast powered; signal on; can be powered through: the block (0,2,40) (Repeater at (-1,2,40)): re-settles to NorthEast at its first update
  (3,1,18) (arms NSW): stored NorthSouth; the game settles SouthWest unpowered, NorthWest powered; signal off; can be powered through: the block (3,0,18) (RedstoneBlock at (2,0,18), Repeater at (4,0,18)): re-settles to SouthWest at its first update
```

## The flood: `mctas mobfarm exit wip/carousel-s3-v1.txt --seed 1` (main 35e4a4899+; the file holds a bed, so the whole-farm flood)

```
refused: 13 villagers share a 10-block box and the villager at 6.5 0.7 40.3 can see the one at 6.5 0.7 43.3: gossip could spawn golems (not modeled)
87 spawn spots, 623 positions, 10 runs, worst life 0 gt, 10.0 s
golems stood on cells within 0 -17 0 .. 17 3 55
FAIL a golem from 4.5000 -2.00 33.5000 walking still: pinned at 4.50 -2.00 33.50: it stood still 100 gt out of the lava (no current carries it)
FAIL a golem from 4.5000 -2.00 33.5000 walking walk yaw 90 x1: gt 10: a golem touches a vehicle at 3.5 0.7 32.7
FAIL a golem from 4.5000 -2.00 33.5000 walking walk yaw 135 x1: gt 13: a golem touches a vehicle at 3.5 0.7 32.7
FAIL a golem from 4.5000 -2.00 33.5000 walking walk yaw 180 x1: gt 10: a golem touches a vehicle at 3.5 0.7 32.7
FAIL a golem from 4.5000 -2.00 33.5000 walking walk yaw 315 x1: gt 257: a golem left the build at 18.8 -31.6 50.3
FAIL a golem from 2.5000 -5.00 42.5000 walking still: the golem vanished without dying
```
