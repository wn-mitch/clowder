# 263 activation — Patrol threat-recency axis (2026-07-13)

## Hypothesis

Activating `scoring.patrol_threat_recency_weight=1.0` lets Patrol's CompensatedProduct read per-cat LocationBeliefs at the patrol perimeter anchor. Recently shocked or ambushed sectors become low-attractiveness patrol targets instead of ordinary perimeter work. The original hard canary was `deaths_by_cause.ShadowFoxAmbush`, but the seed-42 baseline and treatment both had zero ambush deaths, so the measurable rerun used the healthy-colony stress aggregate while keeping ShadowFoxAmbush as a hard survival gate.

## Prediction

| Field | Value |
|---|---|
| Constants patch | `scoring.patrol_threat_recency_weight=1.0` |
| Metric | `negative_events_total` |
| Direction | decrease |
| Rough magnitude band | 10–1000% |

Spec: `docs/balance/263-activation/patrol-threat-recency.yaml`

## Observation

Commands:

1. `just hypothesize docs/balance/263-activation/patrol-threat-recency.yaml --slug 263-activation/patrol-threat-recency --rationale ticket315_patrol_threat_recency_activation` failed because `deaths_by_cause.ShadowFoxAmbush` was absent from both footers (zero deaths in both runs).
2. `just hypothesize docs/balance/263-activation/patrol-threat-recency.yaml --slug 263-activation/patrol-threat-recency --rationale ticket315_patrol_threat_recency_fallback_metric` reused the same sweeps with fallback metric `negative_events_total`.

| Artifact | Path / result |
|---|---|
| Baseline sweep | `logs/sweep-baseline-263-activation/patrol-threat-recency` |
| Treatment sweep | `logs/sweep-263-activation/patrol-threat-recency-treatment` |
| Observed direction | decrease |
| Observed delta | -10.8% |
| Concordance verdict | concordant |

## Concordance

**Verdict: axis-level concordant, not shipped.** The Patrol axis alone cleared the fallback stress metric by a narrow margin, but the ticket-level batched gate failed and the activation soak was `concern` on `continuity_tallies.play=0`. This axis was not activated in defaults.

## Decision

Keep `patrol_threat_recency_weight` dormant until the ticket-level batched activation passes continuity and stress gates.
