# Boat/lead vanilla evidence driver

This standalone crate runs the four approved Phase B mechanics probe groups. It does not link a new entity model into redsim or change the shared Sandbox judge. The official server jar is unchanged. Its optional Java agent inserts read-only phase-observer calls; a separate bootstrap-visible helper uses reflection so it can see the official bundler's isolated classes.

The actor is reused from `explore/bedrock-yt` commit `4b8f8f96cb111838adbfadcdeed0e9e57e4563a7`. Its only new operations are read-only crosshair entity identity and `probe_state`. Real key/use handlers perform mounting, attachment, and movement. No artificial player trajectory is provided.

```
cargo build --release
cargo test --release -p boat-lead-vanilla
target/release/boat-lead-vanilla audit
target/release/boat-lead-vanilla run SCENARIO.json FRESH_OUTPUT_DIR
target/release/boat-lead-vanilla summary TRACE.jsonl
target/release/boat-lead-vanilla budget TRACE.jsonl BUDGET.jsonl
target/release/boat-lead-vanilla report OUTPUT_DIR
```

`generate DIR` writes only the fixed approved pair/control scenarios. `controls OUTPUT_ROOT` runs the four fixed no-lead/lower-seat controls sequentially. `dock OUTPUT_DIR` creates and runs the normal-ticking physical-dock setup control approved by the coordinator. These commands are not a parameter search.

Prerequisites: official Java 26.3 cache, pinned Fabric loader cache, Xvfb, software Vulkan (`mesa-vulkan-drivers` and `vulkan-loader`), ffmpeg. The driver reuses `mctas::vanilla::VanillaServer` through an additive explicit survival profile: no protocol bot, `gamemode=survival`, `allow-flight=false`. Existing callers retain their original profile. Two heavy processes at once: the server and genuine client; recording uses one encoding thread. Processes are scoped to the run and stopped on return/error.

Outputs include copied scenario, interventions, raw phase records, exact sampled attachment coordinates, current controller/authority/holder identities, contact and loaded-chunk flags, observed pre/post leash and gravity phases, read-only force-budget diagnostics, screenshots, and real gameplay video. Errors or missing traces fail the run. Never infer flight from a source calculator or a single rendered frame.

Important: `/tick freeze` does **not** freeze a root carrying a player (`TickRateManager.isEntityFrozen`). Mixed-pair frozen assembly has a recorded pre-launch transient; use the normal-ticking dock control rather than assuming equal initial states. Commands arranging the fixture and removing its supports are explicit interventions. None of these fixture runs alone establishes a survival assembly recipe or 1200-tick sustained flight.
