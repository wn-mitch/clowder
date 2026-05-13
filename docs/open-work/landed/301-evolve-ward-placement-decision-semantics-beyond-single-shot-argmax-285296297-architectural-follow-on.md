---
id: 301
title: evolve ward placement decision semantics beyond single-shot argmax (285+296+297 architectural follow-on)
status: done
cluster: buildings-zones
initiative: []
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md, 297-fox-patrol-topology-axis.md]
landed-at: 5a2893faca1d
landed-on: 2026-05-13
---

## Why

Three independent threat-axis levers have now been ruled out as moving `compute_ward_placement`'s argmax: 285 (anchor-weight magnitude), 296 (Logistic curve shape), and 297 (a new orthogonal `fox_intercept` input axis). Six unique constant changes across three seeds (42, 99, 7) produced byte-identical ward placements (`docs/balance/297-fox-patrol-topology-axis.md`). 297 iter-2's architectural conclusion: the threat-axis composition `(fox_scent.max(corruption) + L(ambush) + L(carcass) + L(fox_intercept)).min(1.0)` is **rank-preserving for the argmax once any threat input saturates on enough tiles** — the argmax is then decided by `+ 0.3 * cat_value − distance_cost + jitter`, with jitter ∈ [0, 0.05) doing real tie-breaking work between saturated tiles.

This ticket targets the deepest structural lever named in 297 iter-2: the **placement decision composition itself**. The three sibling follow-ons (cat_value coefficient — 298, distance_cost — 299, candidate-generation step — 300) are PARAMETER-LEVEL tweaks to the same single-shot-argmax composition — they shift WHICH saturated tile wins, but not WHETHER one tile wins per wake. This ticket changes the composition: a descending-residual or budget-greedy scheme PROGRESSIVELY EATS coverage as it places, so successive picks naturally spread across the threat surface instead of co-locating in the same hot cluster. This is the most impactful axis remaining and also the highest-risk.

## Scope

1. Refactor `compute_ward_placement` to support a non-argmax composition (one of the three named alternatives below).
2. Wire the new semantics into `assess_colony_needs`'s per-wake decision at `coordination.rs:481`.
3. Behavioral unit tests verifying spread-vs-clustering invariants per option.
4. Three-seed soak validation against the current single-shot argmax.
5. Balance writeup `docs/balance/301-ward-placement-decision-semantics.md` comparing semantics, mirroring 297 iter-2's framing.
6. Per CLAUDE.md "Bugfix discipline": structural-option menu with three named alternatives, picked on layer-walk evidence — not on aesthetic preference.

## Out of scope

- `+ 0.3 * cat_value` coefficient (sibling ticket 298).
- `DIST_PENALTY_PER_TILE` distance_cost (sibling ticket 299, `coordination.rs:1428`).
- `CANDIDATE_STEP` candidate-generation grid (sibling ticket 300, `coordination.rs:1421`).
- Threat-axis inputs (ruled out across 285+296+297).
- Placement timing/cadence: the every-20-ticks coordinator wake stays as-is.
- Per-cat ward placement: this remains coordinator-driven via `DirectiveKind::SetWard`.

## Current state

`compute_ward_placement` at `coordination.rs:1390-1502` is a single-shot argmax. Score formula at `:1494`:

```rust
let threat = (fox_scent.max(corruption) + ambush_lift + carcass_lift + fox_intercept_lift).min(1.0);
let unaddressed_threat = (threat - coverage).clamp(0.0, 1.0);
let score = unaddressed_threat + 0.3 * cat_value - distance_cost + jitter;
```

Called from `assess_colony_needs` at `coordination.rs:481` gated by `ward_strength_low && thornbriar_available`. Each invocation returns ONE `Position`; the coordinator queues a single `SetWard` directive (`:505-511`). Multiple placements per wake are not supported. Wake cadence is `cc.assess_interval` (~20 ticks). Across the 900s soaks in 297 iter-2, `wards_placed_total` reached 9-14 per seed — the coordinator does fire repeatedly, but each wake re-scores all candidates from scratch against the same `ward_coverage` field (which only updates after the previous placement materializes through the cat→Ward pipeline, ~lag-bounded).

## Approach

1. **Layer-walk audit per CLAUDE.md "Bugfix discipline."** Mark each layer `[verified-correct]` or `[suspect]`:
   - L1 input field (`PlacementMaps`, `ward_coverage`)
   - L2 candidate generation (`coordination.rs:1431-1443`)
   - L3 per-candidate scoring (`:1494`)
   - L4 selection rule (single-shot argmax at `:1496-1500`) — **suspect** per 297 iter-2.
   - L5 directive emission (`:481-512`) — single `SetWard` per wake.

2. **Structural-option menu** — name three alternatives, pick one:
   - **SPLIT (descending-residual single-pass).** Keep one placement per wake, but inside `compute_ward_placement` simulate K rounds: pick top tile by `unaddressed_threat`, stamp simulated coverage onto an in-function copy of the threat field, re-rank, pick again. Return the K-th pick (or the round-0 pick conditioned on already-placed wards from prior wakes). Smallest surface change; preserves the directive contract.
   - **EXTEND (budget-greedy K-placement).** Coordinator queues K `SetWard` directives per wake. Greedy-submodular: each pick maximizes marginal coverage of remaining un-warded threat. Touches `DirectiveQueue` consumer pipeline; new K-budget constant.
   - **RETIRE (threshold + priority-queue).** Replace argmax entirely: enumerate every candidate above an `unaddressed_threat` threshold, emit one directive per (or top-N by threshold-excess). No argmax at all; coverage emerges from threshold geometry.

3. **Land behind a `SimConstants` flag.** Add `scoring::ward_placement_semantics: SingleShotArgmax | DescendingResidual | BudgetGreedy` with default `SingleShotArgmax` — preserves byte-identical pre-ticket behavior (ticket-061-precedent: any change near the wildlife schedule perturbs seed-42; this flag-gates the perturbation). Unit-test each branch.

4. **First-light.** Lift the flag to the chosen new semantics for a single `just soak-trace 42 Wren`. Spatial check: verify wards no longer co-locate in the same threat-saturated cluster on seed-42 (the seed where 297 iter-2 showed single-cluster lock-in).

5. **Four-artifact.** Three-seed `just hypothesize` comparing `SingleShotArgmax` vs the chosen new semantics across 42 / 99 / 7. Pre-register: expect `wards_placed_total` same or slightly higher; expect `shadow_foxes_avoided_ward_total` to lift on seed-42 (the cluster-bound seed); expect continuity canaries to hold (mirroring 296/297 first-light pattern).

6. **Decision.** Land the chosen semantics as default if four-artifact concordant; otherwise findings-only with the flag preserved at `SingleShotArgmax` and a balance doc explaining which mechanism failed and why (per 297 iter-2's "what iter-2 is NOT" framing).

This ticket is structurally heavier than 298 / 299 / 300 (those are parameter sweeps; this is a control-flow change touching either the scorer body, the directive surface, or both). Estimate accordingly when sequencing the four follow-ons.

## Verification

- `just check` + `just test` green.
- Behavioral unit tests per semantic option's spread invariant — e.g., descending-residual: synthetic threat-spike map, place 3 wards, assert pairwise Manhattan ≥ some spread minimum (no co-location).
- `just soak-trace 42 Wren` spatial check confirms predicted spread pattern fires.
- `just hypothesize` four-artifact across 42 / 99 / 7.
- Continuity canaries hold across the refactor (courtship, friendship, etc., per the 297 iter-2 methodology note).
- Default-flag soak (`SingleShotArgmax`) produces byte-identical output to pre-ticket head — verifies the flag-gating is a true no-op at default.

## Log
- 2026-05-12: opened as lever #4 of four follow-on tickets from 297's iter-2 architectural finding. Deepest structural lever — changes composition rather than parameters. Sequence after 298/299/300 land or in parallel if surface investigation proceeds independently.
- 2026-05-13: 2026-05-13: landed as substrate-no-op (flags default to dormant). First-light soak (seed-42) anti-concordant on shadow_foxes_avoided_ward_total — score formula lacks topology-aware input. Follow-ons: 311 (chokepoint scenario), 312 (corridor perception axis), 313 (cat_value / distance_cost re-examination). See docs/balance/301-ward-placement-decision-semantics.md.
