---
id: 303
title: split cat_value into movement-intensity and residence axes (298 structural follow-on)
status: ready
cluster: balance
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [298-ward-placement-cat-value-coefficient.md]
landed-at: null
landed-on: null
---

## Why

298 ran the first non-threat-axis lever in the 285→298 sequence (the `+ 0.3 * cat_value` coefficient in `compute_ward_placement` at `src/systems/coordination.rs:1557`) and produced rank-changing placement on 2 of 3 seeds — but the metric movement was too small to justify shifting the default (`docs/balance/298-ward-placement-cat-value-coefficient.md` iter-1). 298's writeup identified the architectural cause: the existing `CatPresenceMap` populator at `src/systems/disposition.rs::cat_presence_tick` gates deposits on `Action::Patrol | Fight | Explore` — it **already encodes movement intensity**, not residential density. There is currently no signal in the scoring formula for "tiles where cats sleep, eat, and recover" — exactly the tiles where they're most vulnerable while immobilized. The ticket 298 scope drafted this as the structural alternative; 303 implements it.

## Scope

- Add a new `CatResidenceMap` resource alongside `CatPresenceMap` in `src/resources/`. Deposit on `Action::Sleep | Eat | Groom | Socialize` when the actor is on a structure or den tile.
- Decay rate matches `CatPresenceMap` for symmetry (use the same `scent_decay_rate` from `FoxEcologyConstants`).
- Read both maps in `compute_ward_placement` with separate coefficients: `ward_placement_cat_value_weight` (existing, movement-intensity) and a new `ward_placement_cat_residence_weight` (default 0.0 — dormant first-light).
- First-light activation soak per `feedback_dormant_substrate_activation_soak_first`: lift `ward_placement_cat_residence_weight` to a first-light value (suggest 0.3, mirroring the movement weight) and verify placement shifts onto den/structure-adjacent tiles distinct from the patrol-corridor tiles `CatPresenceMap` already favors.
- Add corresponding marker + InfluenceMap registry entries (per CLAUDE.md "InfluenceMap registry stubs are forbidden").

## Out of scope

- Tuning `ward_placement_cat_value_weight` further at this layer — 298's iter-1 already established the parameter sweep is too weak. 303 is structural, not a tighter probe on the same axis.
- Path A vs Path B placement split (the `WardPlaced.location` decomposition surfaced in 300 iter-3). That's ticket 301 / a separate structural refactor.
- The `distance_cost` term tuning (ticket 299).

## Current state

298 lands findings-only at W=0.3 (substrate-no-op promotion of the 0.3 literal). The `CatPresenceMap` populator gates on `Action::Patrol | Fight | Explore`. Reader is `compute_ward_placement` at `coordination.rs:1503`. No `CatResidenceMap` exists yet.

The architectural finding from 298 that motivates 303: across 285/296/297/300, four threat-axis-adjacent levers produced byte-identical placement on every seed tested. 298's cat_value coefficient bump was the first lever to produce rank-changing placement (2 events dropped on seed-42 at metric-irrelevant tile, 2 events added on seed-7 at fox-intercept tile +5.1% metric), but the magnitude was too modest to justify a default shift. The pattern suggests the formula's single cat-side signal is the binding constraint — splitting it into two orthogonal axes (movement vs. residence) is the next structural move.

## Approach

1. **Add `CatResidenceMap`** at `src/resources/cat_residence_map.rs` mirroring `CatPresenceMap`'s structure (same grid resolution, same deposit/decay API).
2. **Add populator** as a new system in `src/systems/disposition.rs`, registered in `SimulationPlugin::build()` adjacent to `cat_presence_tick`. Deposit gates on `Action::Sleep | Eat | Groom | Socialize` AND actor-on-structure-or-den (check via `tile_map.get(pos).structure.is_some() || actor has DenMember marker`).
3. **Add InfluenceMap impl + registry entry** in `populate_influence_map_registry`.
4. **Promote `ward_placement_cat_residence_weight`** to `SimConstants.scoring` with first-light default 0.0 (dormant). Doc-comment names the marker symmetry with 298.
5. **Update `compute_ward_placement`** at `coordination.rs:1503` to read both maps; modify scoring at `:1557` to include `+ w_residence * cat_residence_value` alongside the existing `+ w_cat_value * cat_value`.
6. **Unit tests** mirroring `ward_placement_dormant_when_weights_forced_to_zero` (residence weight at 0.0 must produce byte-identical placement to pre-303 — this is the substrate-no-op contract).
7. **First-light activation soak** per `feedback_dormant_substrate_activation_soak_first`: lift weight to 0.3, run `just soak-trace 42 <focal>`, run `just verdict`. Validates layer fires.
8. **Four-artifact hypothesize sweep** across seeds 42/99/7 to validate the metric movement is real (and larger than 298's modest +5.1% on seed-7).

## Verification

- `just check` + `just test` green.
- New unit test: residence weight at 0.0 → placement byte-identical to pre-303 (substrate-no-op contract).
- New unit test: residence weight at 0.3 with a synthetic colony where cats sleep/eat at a fixed den tile → ward placement shifts toward the den tile.
- `just soak-trace 42 <focal>` at residence weight = 0.3 produces visible shift of `WardPlaced` events toward sleep/eat cluster tiles (separate from the patrol-corridor tiles `CatPresenceMap` already favors).
- `just hypothesize` four-artifact sweep across seeds 42/99/7: predicts `shadow_foxes_avoided_ward_total` increases by ≥10% on at least one seed (load-bearing improvement over 298's +5.1% modest result).
- All continuity canaries hold; no `Starvation` or `never_fired_expected_positives` regression.

## Log

- 2026-05-12: opened as structural follow-on from ticket 298's iter-1 finding that the cat_value coefficient is too weak a lever to justify a default shift. 298 named this exact split (movement vs. residence) but with reversed labels — 303's scope corrects the framing per the disposition.rs::cat_presence_tick read.
