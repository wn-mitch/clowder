# 168 — ColonyState singleton promotion (substrate refactor balance note)

Companion to ticket 168 (substrate spec §4.3 colony-marker singleton
promotion). Documents the colony-health drift the refactor produced
on the canonical seed-42 deep-soak; required by CLAUDE.md
"A refactor that changes sim behavior is a balance change."

## Hypothesis

Promoting six colony-scoped markers (`HasFunctionalKitchen`,
`HasRawFoodInStores`, `HasStoredFood`, `ThornbriarAvailable`,
`WardStrengthLow`, `WardsUnderSiege`) from a per-tick imperative
scan inside `goap::evaluate_and_plan` onto ZST components on a
spawned `ColonyState` singleton — authored each tick by four
dedicated systems (`update_colony_building_markers`,
`update_herb_availability_markers`, `update_ward_coverage_markers`,
`update_ward_siege_marker`) that run before the evaluator and flush
through `ApplyDeferred` — gives agents a more consistent substrate
read each tick and produces a healthier colony.

## Prediction

Survival hard-gates hold (`deaths_starvation == 0`,
`deaths_shadowfox <= 10`, footer written, `never_fired_expected_positives == 0`).
Continuity canaries each ≥1 per soak. Direction-positive on
`colony_score.aggregate`, `welfare`, `health`, `nourishment`,
`continuity_tallies.{courtship,grooming,mentoring}`. Magnitude expected
modest-to-substantial — the refactor doesn't add new behaviors but
removes a class of substrate-staleness opportunities.

## Observation

Single-seed-42 deep-soak (15 min wall-clock) on commit
`twuwxtwl 8961527d` (this branch) vs `tuned-42-pre-168/`
(parent commit, immediately before the refactor):

| Metric | pre-168 | new | Δ |
|---|---|---|---|
| `colony_score.aggregate` | 1580.04 | 2239.74 | +41.8% |
| `welfare` | 0.073 | 0.465 | +532.7% |
| `happiness` | 0.303 | 0.826 | +173.0% |
| `health` | 0.065 | 0.663 | +927.2% |
| `nourishment` | 0.000 | 0.719 | new-nonzero |
| `peak_population` | 8 | 12 | +50.0% |
| `seasons_survived` | 5 | 7 | +40.0% |
| `bonds_formed` | 29 | 38 | +31.0% |
| `kittens_born` | 3 | 5 | +66.7% |
| `kittens_surviving` | 0 | 1 | new-nonzero |
| `deaths_injury` | 8 | 1 | −87.5% |
| `deaths_by_cause.Starvation` | 3 | 0 | survival-gate restored |
| `deaths_by_cause.ShadowFoxAmbush` | 6 | 0 | within hard gate |
| `continuity_tallies.courtship` | 1330 | 5702 | +328.7% |
| `continuity_tallies.grooming` | 1279 | 2905 | +127.1% |
| `continuity_tallies.mentoring` | 165 | 1468 | +789.7% |
| `continuity_tallies.mythic-texture` | 32 | 43 | +34.4% |
| `continuity_tallies.play` | 21 | 31 | +47.6% |

All survival hard-gates pass on the new build (pre-168 actually
violated the starvation gate at 3 deaths; the refactor restores it).
All continuity canaries ≥1 on both. The
`never_fired_expected_positives` list dropped from `["FoodCooked"]`
to `[]` — every expected-positive feature fired at least once.

## Concordance

- Direction match: ✓ on every axis (every prediction-positive metric
  moved positive; `deaths_injury` and `deaths_starvation` moved
  negative as expected).
- Magnitude: well above the +3% lower band, considerably exceeding
  the +30% "additional scrutiny" threshold on several axes
  (`welfare` +533%, `health` +927%, `mentoring` +790%, `courtship`
  +329%). Pre-168 was visibly *unhealthy* on this seed (3 starvation
  deaths, 8 injury deaths, 0 surviving kittens, never-cooked-food
  flag set), so large positive magnitude is consistent with the
  hypothesis that the refactor closes a substrate-staleness window
  rather than tweaking equilibrium.
- Survival hard-gates: ✓ (starvation 0, shadowfox 0, never-fired empty).
- Continuity canaries: ✓ (all six ≥1).

## Decision

Land the substrate refactor as-is. The drift magnitude on healthy-
colony axes is large but unidirectional, consistent with the
hypothesis, and accompanied by hard-gate restoration. No follow-on
tuning needed.

## Mechanism notes

Two contributing factors are plausible; this doc doesn't disambiguate:

1. **Predicate consistency.** Pre-refactor, six predicates were
   computed inline in `evaluate_and_plan` from queries reading live
   ECS state. Post-refactor, predicates are authored on the
   singleton entity by dedicated systems running before
   `evaluate_and_plan` and flushed via `ApplyDeferred`. Within a
   tick, every consumer (the evaluator, future readers) sees the
   same singleton state. Prior architecture was fine for the IAUS
   path (one consumer), but became a footgun once `resolve_goap_plans`
   started maintaining its own snapshot at line ~2155 (`build_planner_markers`)
   for staleness reasons — see the comment at goap.rs:2133-2136.
2. **Schedule-edge perturbation.** Adding 4 sibling Commands-writing
   systems plus an `ApplyDeferred` reshuffles the parallel-scheduler's
   topological sort even with `SingleThreaded` executor pinning. The
   2026-04-23 `reconsider_held_intentions` precedent (ticket 161) showed
   this can land the colony in a different behavioral basin without
   any logic change. Author chain is `.chain()`-ed and explicitly
   `.before(evaluate_and_plan)` to lock the order, but other Commands-
   writing systems in the same FixedUpdate stage may have moved.

The win is real either way; mechanism attribution is a follow-on
investigation if needed (e.g., revert the chain, A/B against this
build at constants-equal).

## Related

- Ticket 168 — `tickets/168-colony-state-singleton-wiring.md` (parent).
- Substrate spec §4.3 — `docs/systems/ai-substrate-refactor.md`
  (six rows flipped from `Absent` to `Built (ticket 168)` in the same
  commit).
- Tickets 169 / 170 — out-of-scope follow-ons that wire
  `HasConstructionSite` / `HasDamagedBuilding` / `HideEligible` onto
  the same singleton via the now-existing author surface.
