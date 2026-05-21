---
id: 433
title: 431 Stage F follow-on — colony marker-snapshot hoist (cross-system dedupe)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: c1072e46
landed-on: 2026-05-20
---

## Why

Ticket 431's original Stage F proposed building `ColonyMarkerSnapshot` once per FixedUpdate so per-cat `MarkerSnapshot::new()` (at `goap.rs:1477`) becomes a thin per-cat overlay. The 431 closeout layer-walk surfaced that the marker snapshot is already amortized across cats *within* one `evaluate_and_plan` run — the per-cat for-loop reuses the system-level `markers` variable. Hoisting *only* the colony-marker portion to a separate populator system would move the same work to a different system without saving CPU.

The meaningful win lives at the cross-system level: multiple systems re-derive the same colony state (`kitten_snapshot`, `cat_positions`, marker presence, food/store fractions, etc.) per tick. A `WorldSnapshots` resource populated once per FixedUpdate and consumed read-only across all per-tick systems would dedupe that work. This is the substrate intent that 431 §Out of scope flagged as "ticket 432's WorldSnapshots" but which was never actually opened — id 432 ended up holding the unrelated Stage C follow-on instead, so this ticket 433 carries the original Stage F + WorldSnapshots intent.

## Scope

- Inventory the cross-system duplications: which per-cat / per-tick computations get re-derived by multiple systems within one FixedUpdate.
- Design a `WorldSnapshots` Resource (or set of Resources) carrying the deduped read-only state.
- Author a `populate_world_snapshots` system that runs once per FixedUpdate at the head of the chain.
- Refactor consumers (`evaluate_and_plan`, `caretake_targeting`, etc.) to read from the snapshot.

## Out of scope

- 431's original Stage F-as-narrowly-defined (just `ColonyMarkerSnapshot` hoist) — superseded by the broader `WorldSnapshots` framing.
- Per-cat snapshots that don't have cross-system reuse (e.g. per-cat `MarkerSnapshot` overlays specific to one cat's eligibility filters).

## Current state

Opened 2026-05-20 as the deferral home for 431 Stage F's substrate intent. Blocked by 431 only for the binary-commit-truth tooling that perf verification depends on; the audit + design phase can start immediately.

## Approach

Audit phase: read the system schedule chains in `src/plugins/simulation.rs` and identify the systems that build the same intermediate state per tick. Snapshot candidate fields (initial):
- Colony-wide marker booleans (`HasStoredFood`, `HasGarden`, …) — built in `evaluate_and_plan:1478+` but also relevant to `update_colony_building_markers` and downstream cat-side queries.
- Kitten snapshot (`KittenState { entity, pos, hunger, mother, father }`) — built in `evaluate_and_plan:1493+`; potentially reusable by `caretake_targeting`.
- Cat positions Vec — built per-system in multiple places.

Design phase: define `WorldSnapshots` shape, decide whether one Resource or several, decide invalidation strategy (rebuilt every tick unconditionally, vs. event-driven invalidation).

## Verification

- Flamegraph before/after: catalog row #3's `evaluate_and_plan` (24.37% inclusive) should drop by the amount of duplicated work hoisted.
- `just verdict` semantic-pass against the post-431 baseline.

## Log

- 2026-05-20: opened as the proper home for 431 Stage F's substrate intent after the 431 closeout analysis showed the narrow "marker-snapshot-only hoist" wasn't a meaningful win. The broader cross-system dedupe was always the right framing; 431 §Out of scope had referenced it as "ticket 432" but that id ended up assigned to the Stage C follow-on instead.
- 2026-05-20: WorldSnapshots resource + colony-marker / food-fraction hoist. Audit + design doc at docs/systems/world-snapshots.md names the four cross-system duplications surveyed (colony markers, cat_positions, food_fraction, kitten_snapshot) and the caveat that gates each. First concrete hoist: colony markers + food_fraction read by evaluate_and_plan instead of inline colony_state_query + food.fraction(). Soak verdict 'concern' against the stale baseline (095-phase-1a-shadow); welfare +0.6%, fulfillment +8%, shelter -9%, schedule-edge perturbation from the new populate_world_snapshots sibling explains the drift (memory learning_bevy_schedule_edge_perturbation).
