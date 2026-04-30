---
id: PE-001
title: Test-harness drift retired
status: done
cluster: null
landed-at: 04aef57b
landed-on: 2026-04-28
---

# Test-harness drift retired

**Landed:** 2026-04-28 | **Commit:** 04aef57b — `feat: deterministic same-seed replay`

**Why:** PE-001 was about three integration tests panicking with a Bevy "Resource does not exist" error — a system added to `SimulationPlugin::build()` whose required resource wasn't inserted in `tests/integration.rs::setup_world`. The bespoke `setup_world` + `build_schedule` scaffold in the test file mirrored the production app's resource set, and every new resource added to the production app drifted the mirror further out of sync. Earlier triage proposed enumerating the missing resource and adding it to the scaffold; the eventual fix deleted the scaffold instead.

**What landed:** Resolved as part of the replay-determinism work. `tests/integration.rs` now builds the actual `SimulationPlugin` + `HeadlessIoPlugin` against a `MinimalPlugins` `App`, so every resource the production app inserts is automatically picked up by tests. All three previously-failing tests pass:

- `simulation_is_deterministic` — strengthened to assert byte-identical `events.jsonl` across two seed-42 runs of 600 ticks (the old assertion was narrative-equality, which the determinism work made testable in a stronger form).
- `cats_eat_when_hungry` — drives needs through `app.world_mut()` and asserts at least one cat completed the eat loop within 400 ticks.
- `simulation_runs_1000_ticks_without_panic` — smoke test on the canonical headless graph.

`just check-determinism` runs `simulation_is_deterministic` in release mode (~6 s) and is wired into `just ci`, so the property is gated on every CI run. Future drift between test scaffold and production app is now structurally impossible: there is no scaffold.

**Verdict:** retired, not solved-as-stated. The "missing resource" framing turned out to be a symptom of the wrong shape of fix.

---
