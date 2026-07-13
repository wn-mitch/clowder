# 263 activation — Flee affordance axis (2026-07-13)

## Hypothesis

Activating `scoring.flee_affordance_weight=0.5` lets the Flee DSE price ActionAffordances' richer "can this cat actually escape this threat now?" signal instead of relying only on safety deficit, boldness, nearest-threat distance, and health. The original hard canary was `deaths_by_cause.ShadowFoxAmbush`, but the seed-42 baseline and treatment both had zero ambush deaths, so the measurable rerun used `negative_events_total` while keeping ShadowFoxAmbush as a hard survival gate.

## Prediction

| Field | Value |
|---|---|
| Constants patch | `scoring.flee_affordance_weight=0.5` |
| Metric | `negative_events_total` |
| Direction | decrease |
| Rough magnitude band | 10–1000% |

Spec: `docs/balance/263-activation/flee-affordance.yaml`

## Observation

Commands:

1. `just hypothesize docs/balance/263-activation/flee-affordance.yaml --slug 263-activation/flee-affordance --rationale ticket315_flee_affordance_activation` failed because `deaths_by_cause.ShadowFoxAmbush` was absent from both footers (zero deaths in both runs).
2. `just hypothesize docs/balance/263-activation/flee-affordance.yaml --slug 263-activation/flee-affordance --rationale ticket315_flee_affordance_fallback_metric` reused the same sweeps with fallback metric `negative_events_total`.

| Artifact | Path / result |
|---|---|
| Baseline sweep | `logs/sweep-baseline-263-activation/flee-affordance` |
| Treatment sweep | `logs/sweep-263-activation/flee-affordance-treatment` |
| Observed direction | unchanged |
| Observed delta | -6.6% |
| Concordance verdict | wrong-direction (prediction decrease vs observed unchanged/noise) |

## Concordance

**Verdict: inconclusive / not shippable.** The Flee lift did not produce a measurable ≥10% stress reduction on this seed, and the ambush-death canary was floored at zero in both sweeps. This axis was not activated in defaults.

## Decision

Keep `flee_affordance_weight` dormant until ticket 315 can be re-run with a measurable canary/batched gate that passes continuity.
