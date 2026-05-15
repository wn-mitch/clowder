---
id: 294
title: RecentAmbushMap retirement — colony Resource moves to per-cat LocationBeliefs.recency_of_threat_cue (258 follow-on)
status: ready
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception]
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

`RecentAmbushMap` (`src/resources/recent_ambush_map.rs`) is a **colony-wide** `Vec<f32>` deposit-and-decay grid: when a predator ambushes a cat, `recent_ambush_map.deposit(wl_pos.x, wl_pos.y, 1.0)` marks the spot for every cat in the colony. The all-cats-see-the-same-field shape is a load-bearing simplification that pre-dates C3 substrate — it conflates "an ambush happened here" with "every cat knows it happened here." Under C3, ambush memory is a per-cat belief: only cats within sensing range of the ambush (witnesses, plus possibly the victim if surviving) should hold the recency-of-threat-cue lift at that location. Other cats learn via colony knowledge promotion (291) — which requires the per-cat substrate first. Retiring the Resource moves ambush memory from a colony-uniform field to a per-cat `LocationBeliefs[bucket].recency_of_threat_cue`, with colony aggregation as a derived layer.

## Scope

- Add `WitnessableEvent::PredatorAmbush { predator: Entity, victim: Entity, position: Position, tick: u64 }` variant. Emit from `src/systems/wildlife.rs:2190` (current writer site).
- `belief_integrator::apply_observation` handles it: for each witness within sensing range, lift `LocationBeliefs[bucket(position)].recency_of_threat_cue` toward `OBSERVED_MAX` via EMA.
- Update the 6 reader sites: `src/systems/coordination.rs:191`, `src/systems/coordination.rs:1340`, `src/systems/coordination.rs:1474` (ward placement), `src/systems/disposition.rs:198/946/1584/4605/4612` (ScoringContext build + tests), `src/systems/goap.rs:1923` (ScoringContext build), `src/ai/scoring.rs:435/884–885` (recent_ambush_at_position scalar consumers) + 9 default-init sites. All currently read `colony.recent_ambush_map.get(pos.x, pos.y)`; change to read each cat's own `LocationBeliefs[bucket(pos)].recency_of_threat_cue.value`.
- Delete `src/resources/recent_ambush_map.rs`, the spawn-time resource insert, decay system, and InfluenceMap registry entry at `src/plugins/simulation.rs:153`.
- Replace InfluenceMap registry entry (228's per-cat-substrate boundary) with a per-cat adapter so `trace-*.jsonl` still surfaces ambush samples — pattern from `src/components/route_cost_field.rs` precedent.
- Decide on the witness-range default: ward-placement scoring currently sees ALL ambushes (colony-shared); the new per-cat shape gates by sensing range (`WITNESS_RANGE=10` Manhattan, per 258's integrator). Ward placement may need to read an aggregated view — derive via 291's helper or expose a `colony.aggregated_recent_ambush(pos)` derived from belief agreement.

## Out of scope

- Per-cat InfluenceMap surface refactor (228 covers the substrate-vs-search-state boundary). This ticket adapts to it.
- `LocationBeliefs.recency_of_threat_cue` decay tuning — picked up by 258's tunables, fine-tuned here against the legacy half-life of `RecentAmbushMap`.
- ColonyKnowledge restructure (291) — sequenced before this ticket for the aggregated-view helper.

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). 258's plan-agent flagged RecentAmbushMap as the *wrong* proof choice for the proxy retirement because its retirement is itself a balance change with a real hypothesis to test — the colony-shared-field → per-cat-belief pivot is "all cats see the same ambush field" becoming "each cat has its own ambush belief." That hypothesis lives in this ticket.

Per the 258 audit, RecentAmbushMap has the widest reader fan-out of the four typed-failure proxies: ward placement, ScoringContext build, InfluenceMap surface, 9 default-init sites. The ward-placement read is the load-bearing-and-asymmetric one — wards are colony infrastructure and should arguably read the aggregated view, not any one cat's belief.

## Approach

Sequenced behind 291 (aggregated-view helper) and 258 (substrate). Four commits:

1. **Emit + integrator wiring** for `PredatorAmbush`. Substrate populates per-cat. Run null-drift soak — legacy reader still drives ward placement and scoring.
2. **Aggregated-view helper**: extend 291's mental-model-agreement function to expose `colony.aggregated_location_belief(facet_slot, bucket) -> f32`. Ward placement and ScoringContext-build sites read this instead of `recent_ambush_map.get(...)`.
3. **Cutover**: delete RecentAmbushMap Resource, decay system, InfluenceMap registry entry. Hypothesize cycle.
4. **Balance doc** capturing the four artifacts.

Hypothesis: per-cat ambush belief aggregated via belief-agreement preserves ward-placement rates within ±15% and ShadowFox-banishment rate within ±20%. (Wider hypothesis band than 290/292 because the colony-shared → per-cat shape is a meaningful architectural shift.) **Prediction**: `deaths_by_cause.ShadowFoxAmbush ≤ 10` (hard survival gate) holds; ward-placement spatial distribution shifts modestly toward areas where MORE cats have witnessed ambushes.

## Verification

- `just check` clean.
- `cargo test wildlife` + `cargo test belief_integrator` — tests pass.
- `just soak 42` + `just verdict` — survival gates hold (especially ShadowFoxAmbush ≤ 10).
- `just hypothesize docs/balance/294-recent-ambush-map-retirement.yaml` — concordance pass on ward-placement-rate + ShadowFox-banishment-rate.
- `just q trace <run-dir> <focal>` — confirm per-cat ambush samples surface in the new InfluenceMap adapter.

## Log

- 2026-05-11: opened as 258 follow-on. Per 258's plan-agent: this is the widest-blast-radius retirement of the four proxies; sequenced last. Sibling proxies: 290 (RDF), 292 (RTF), 293 (HuntingPriors).
