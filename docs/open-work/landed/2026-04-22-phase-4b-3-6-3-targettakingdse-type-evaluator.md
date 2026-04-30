---
id: 2026-04-22
title: "Phase 4b.3 — §6.3 `TargetTakingDse` type + evaluator"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4b.3 — §6.3 `TargetTakingDse` type + evaluator

Foundation for §6 target-taking scoring. No DSE ports yet — the
scope is the type, the evaluator, and the registration surface.

- New `src/ai/target_dse.rs` with:
    - `TargetTakingDse` struct per §6.3 — id, candidate_query,
      per-target considerations, composition, aggregation,
      intention factory.
    - `TargetAggregation` enum — `Best` (default), `SumTopN(n)`
      for threat aggregation, `WeightedAverage` for rank-decayed
      sums.
    - `ScoredTargetTakingDse` output — per-candidate scores
      (unsorted), winning target, aggregated score, emitted
      intention; `ranked_candidates()` sorts descending for trace
      emission.
    - `evaluate_target_taking` evaluator — per-candidate score via
      per-target considerations, compose, aggregate. Scalar names
      prefixed `target_` dispatch through a target-scoped fetcher;
      everything else reads the scoring cat.
- `DseRegistry.target_taking_dses` retyped from
  `Vec<Box<dyn Dse>>` to `Vec<TargetTakingDse>`.
  `add_target_taking_dse` registration method on
  `DseRegistryAppExt` takes `TargetTakingDse` by value.
- 6 unit tests: empty-candidate short-circuit,
  `Best`/`SumTopN`/`WeightedAverage` aggregation semantics,
  per-candidate spatial sampling, ranked-candidates helper.

No live-sim behavior change — nothing registers a target-taking
DSE yet. Pure foundation; per-DSE ports follow.
