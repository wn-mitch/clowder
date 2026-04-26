---
id: PE-001
title: Test harness drift
status: done
cluster: null
added: 2026-04-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: a869ef1
landed-on: 2026-04-21
---

## Original state (2026-04-19)

`cargo test` failed three integration tests with a Bevy "Resource does not
exist" panic:
- `cats_eat_when_hungry`
- `simulation_is_deterministic`
- `simulation_runs_1000_ticks_without_panic`

Reverting the 2026-04-19 balance change did not fix them — a system had
been added to the schedule whose required `Resource` was not inserted in
`tests/integration.rs::setup_world`. `just check` (cargo check + clippy)
passed green; only `cargo test` was broken.

## Resolved (2026-04-21, `a869ef1`)

`tests/integration.rs::setup_world` had drifted from
`src/plugins/setup.rs::build_new_world`. Four resources (`ColonyCenter`,
`ForcedConditions`, `ColonyScore`, `UnmetDemand`) and four components on
spawned cats (`GroomingCondition`, `PendingUrgencies`, `SensorySpecies`,
`SensorySignature`) were inserted by the running sim but not by the test
harness, panicking the disposition / weather systems under multi-threaded
schedule execution. Commit `a869ef1` adds the missing insertions; all three
integration tests run green.

The structural cause — two separate world-construction sites that had to be
kept in lockstep by hand — was retired four days later by ticket 030
(`b9129a1`, 2026-04-25), which unified the headless and windowed pipelines
on a single `App + SimulationPlugin` and removed the manual mirror that
made this drift class possible in the first place.
