# Ten boats stacked by collision

Requested Nov 7, 2026: stack ten boats on top of one another.

## Observed result

The official Java 26.3 game code dropped ten empty oak boats into a supported vertical
stack. The final bottom coordinates after 1,200 game ticks were:

```
y = [1.0, 1.5625, 2.125, 2.6875, 3.25, 3.8125, 4.375, 4.9375, 5.5, 6.0625]
x = z = 0.0 for every boat
```

RedSim matched all 1,201 recorded frames exactly: positions, motion, rotations, leash
state, blocks and random state. An initial exploratory recording and the kept recording
both passed. The kept recording used the foundation at `2d8143b29681c2686e0cdda79f7e6c57bd1a88b8`.

## Setup and limits

The fixture commands initially place one boat at each integer y from 1 through 10,
all at x=z=0, above a stone floor at y=0. The gaps require actual gravity-driven movement
before the stack forms; this is not a stationary/frozen-entity test. No leads, passengers,
NoGravity, noPhysics, injected velocity, corrective teleports or forced riding links are
used during the run. The scenario supplies no events after initial creation.

This proves physical empty-boat stacking in the tested arrangement. It is not a survival
assembly demonstration, a ridden/control test, a perturbation-robustness claim or a flyer.
The initial placement uses fixture commands. No parameter search or model change was
needed. This supplemental user-requested test is separate from the 16-case foundation
calibration matrix.

## Reproduce

From the repository root, after the documented setup preflight:

```
target/release/mctas boat-leads record designs/boat-leads/stack/ten-boat-stack-control.json --out-dir target/ten-boat-stack
target/release/mctas boat-leads compare designs/boat-leads/stack/ten-boat-stack-control.trace
```

The trace's referenced floor fixture is kept beside it. Recorded frames were not edited.
