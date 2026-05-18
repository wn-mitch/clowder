---
id: 293
title: HuntingPriors retirement — per-location belief moves to LocationBeliefs.perceived_violence_capability + colony absorption rebuild (258 follow-on)
status: ready
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`HuntingPriors` (`src/components/hunting_priors.rs`) is a per-cat `Vec<f32>` belief grid sized to the world, with `record_failed_search` decreasing belief by `-tiles_searched / 2000.0`. It encodes "this region has yielded no prey after X effort" — exactly what `LocationBeliefs[bucket].perceived_violence_capability` (or a new `prey_yield` facet) should hold under C3. The legacy structure also feeds `ColonyHuntingMap` (`src/resources/colony_hunting_map.rs:133::absorb`) which aggregates priors across the colony — that's the per-location-knowledge promotion shape that 291 is restructuring colony-wide. Retiring HuntingPriors closes the loop: per-cat per-location prey-yield belief lives on `LocationBeliefs`, colony aggregation derives from agreement across `LocationBeliefs` (sharing infrastructure with 291).

## Scope

- Decide the facet semantic: extend `MentalModel` with a `prey_yield: Facet` field (range [0,1], high = "this location has prey", low = "this location is empty") OR repurpose `perceived_violence_capability` as the inverse-prey signal. Default: **add `prey_yield` as a 7th facet** — it's a different perceptual axis from violence capability per CLAUDE.md pillar-3 "orthogonal axes, not louder single alarms."
- Add `WitnessableEvent::HuntSearchYieldedNoPrey { actor: Entity, position: Position, tiles_searched: u64, tick: u64 }` variant. Emit from `src/systems/disposition.rs:920` and `src/systems/goap.rs:2506` (current `record_failed_search` writer sites).
- `belief_integrator::apply_observation` handles the new variant: when `actor == witness`, lower `LocationBeliefs[bucket(position)].prey_yield` via EMA, weighted by `tiles_searched`.
- Symmetric positive emit: `WitnessableEvent::HuntCaughtPreyAt { actor, position, prey_kind, tick }` should lift `prey_yield` at that bucket. (Hunt success already emits `WitnessableEvent::Hunt` but doesn't currently update LocationBeliefs — extend.)
- Rebuild `ColonyHuntingMap::absorb` to derive from per-cat `LocationBeliefs[bucket].prey_yield` agreement (shares infrastructure with 291's mental-model-agreement derivation function).
- Rewrite `best_direction()` and any callers consuming `HuntingPriors` to read from `LocationBeliefs`.
- Delete `src/components/hunting_priors.rs`, spawn-time insert at `src/plugins/setup.rs:97`, mod re-export.

## Out of scope

- 291 (ColonyKnowledge restructure) — this ticket shares the agreement-derivation infrastructure 291 builds. **Sequencing**: 291 lands first; this ticket's `ColonyHuntingMap::absorb` rewrite leverages 291's helper.
- ScoringContext fields that read `ColonyHuntingMap::highest_nearby` (those keep working — the underlying data is just newly-sourced).

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). 258's plan-agent inspection of HuntingPriors:

> HuntingPriors is the wrong proof: the user says it's dormant, but `src/steps/disposition/groom_other.rs:6` and `socialize.rs:6` and `src/resources/colony_hunting_map.rs:12` all read it — it's an active write/learn-from pathway with a colony absorption layer that would need to be re-implemented or abandoned during retirement.

So it's NOT dormant — the colony absorption layer is the load-bearing read. This ticket explicitly rebuilds that absorption layer atop the new substrate before deleting the proxy.

The substrate side: `LocationBeliefs` (`src/components/beliefs.rs`) exists, keyed `LocationKey = (i32, i32)` 5-tile bucketed, but currently only the `recency_of_threat_cue` facet is written (by `WitnessableEvent::FleeFrom`). The `prey_yield` facet is new to this ticket.

## Approach

Sequenced behind 291 to share the agreement-derivation infrastructure. Five commits:

1. **Add `prey_yield` facet** to `MentalModel` + `BeliefAxisTunables` (likely `slow()` shape for stable spatial knowledge). Update belief_integrator tests.
2. **Emit + integrator wiring** for `HuntSearchYieldedNoPrey` and `HuntCaughtPreyAt`. Substrate populates; no reader cutover yet. Run null-drift soak.
3. **Rebuild `ColonyHuntingMap::absorb`** to derive from per-cat `LocationBeliefs[bucket].prey_yield`. Use 291's agreement helper. Hold legacy `HuntingPriors::absorb` in parallel for one soak iteration.
4. **Cutover**: delete `HuntingPriors`, switch readers to `ColonyHuntingMap` (already derived from substrate). Hypothesize cycle.
5. **Balance doc** capturing the four artifacts.

Hypothesis: per-cat `LocationBeliefs.prey_yield` aggregated to colony level preserves the hunting-search efficiency (prey caught per search-tick) within ±10% drift.

## Verification

- `just check` clean.
- `cargo test colony_hunting_map` + `cargo test belief_integrator` — refactored tests pass.
- `just soak 42` + `just verdict` — survival canaries hold; `mythic-texture` continuity preserved; `prey caught per search-tick` derived footer field stable.
- `just hypothesize docs/balance/293-hunting-priors-retirement.yaml` — concordance pass.
- `just frame-diff` against a focal Hunt-DSE trace — confirm L2 score-axis distribution unchanged.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed ** 62** (done, belief-perception, score 0.88 (cross-cluster)) — Prey-species split — per-species scent maps (§5.6.3 row #5)
- ✓ landed **  7** (done, ai-substrate, score 0.87) — Deliberation-layer (Cluster C)
- ✓ landed **263** (done, ai-substrate, score 0.87) — 256-cluster DSE consumers wire belief + affordance axes (Flee, Patrol, Hunt wit…

<!-- linkages:end -->
## Log

- 2026-05-11: opened as 258 follow-on. Sequenced behind 291 (agreement-derivation infrastructure shared). Sibling proxies: 290 (RDF), 292 (RTF), 294 (RecentAmbushMap).
