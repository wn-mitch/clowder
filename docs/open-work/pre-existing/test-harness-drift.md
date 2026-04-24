---
id: PE-001
title: Test harness drift
status: blocked
cluster: null
added: 2026-04-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Current state

**Status:** pre-existing.

`cargo test` fails three integration tests with a Bevy "Resource does not
exist" panic:
- `cats_eat_when_hungry`
- `simulation_is_deterministic`
- `simulation_runs_1000_ticks_without_panic`

Reverting the 2026-04-19 balance change does not fix them — a system was
added to `build_schedule()` (in `src/main.rs` or `SimulationPlugin::build()`)
whose required Resource isn't inserted in `tests/integration.rs::setup_world`.

**`just check` (cargo check + clippy) passes green.** Only `cargo test` is
broken.

**To pick up:** enable a debug feature (or patch a local build of bevy_ecs)
to surface the actual system name and missing-Resource type, then add the
insertion to `setup_world`.
