# 263 activation — Hunt best-predation affordance axis (2026-07-13)

## Hypothesis

Activating `scoring.hunt_best_predation_weight=0.15` adds the per-target `hunt_best_predation_affordance` axis, backed by ticket 314's cat-vs-prey ActionAffordances writer and ticket 516's target-scalar routing fix. HuntTarget should prefer prey that are currently catchable by Stalk, Chase, or Pounce while the existing yield/calm/cooldown axes retain most of the WeightedSum mass.

## Prediction

| Field | Value |
|---|---|
| Constants patch | `scoring.hunt_best_predation_weight=0.15` |
| Metric | `plan_failures_by_reason.EngagePrey: lost prey during approach` |
| Direction | decrease |
| Rough magnitude band | 10–1000% |

Spec: `docs/balance/263-activation/hunt-best-predation.yaml`

## Observation

Command: `just hypothesize docs/balance/263-activation/hunt-best-predation.yaml --slug 263-activation/hunt-best-predation --rationale ticket315_hunt_target_affordance_activation`

| Artifact | Path / result |
|---|---|
| Baseline sweep | `logs/sweep-baseline-263-activation/hunt-best-predation` |
| Treatment sweep | `logs/sweep-263-activation/hunt-best-predation-treatment` |
| Observed direction | decrease |
| Observed delta | -88.7% |
| Concordance verdict | concordant |

## Concordance

**Verdict: axis-level concordant, not shipped.** The HuntTarget axis alone strongly reduced the predicted approach-loss metric, but ticket 315's batched activation gate failed and the activation soak was `concern` on `continuity_tallies.play=0`. This axis was not activated in defaults.

## Decision

Keep `hunt_best_predation_weight` dormant until the ticket-level batched activation passes continuity and stress gates.
