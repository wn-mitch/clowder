# 263 activation — Hunt stalk/chase affordance bias (2026-07-13)

## Hypothesis

Activating `scoring.hunt_stalk_chase_affordance_bias=0.25` lets `resolve_engage_prey` bias its stalk-start band from live Stalk-vs-Chase affordance asymmetry. High Stalk affordance widens the stalking band for unaware prey; high Chase affordance narrows it so cats commit to pursuit sooner against flushed prey. Pounce range remains invariant.

## Prediction

| Field | Value |
|---|---|
| Constants patch | `scoring.hunt_stalk_chase_affordance_bias=0.25` |
| Metric | `plan_failures_by_reason.EngagePrey: lost prey during approach` |
| Direction | decrease |
| Rough magnitude band | 10–1000% |

Spec: `docs/balance/263-activation/hunt-stalk-chase-bias.yaml`

## Observation

Command: `just hypothesize docs/balance/263-activation/hunt-stalk-chase-bias.yaml --slug 263-activation/hunt-stalk-chase-bias --rationale ticket315_hunt_resolver_bias_activation`

| Artifact | Path / result |
|---|---|
| Baseline sweep | `logs/sweep-baseline-263-activation/hunt-stalk-chase-bias` |
| Treatment sweep | `logs/sweep-263-activation/hunt-stalk-chase-bias-treatment` |
| Observed direction | decrease |
| Observed delta | -81.5% |
| Concordance verdict | concordant |

## Concordance

**Verdict: axis-level concordant, not shipped.** The resolver bias alone strongly reduced the predicted approach-loss metric, but ticket 315's batched activation gate failed and the activation soak was `concern` on `continuity_tallies.play=0`. This axis was not activated in defaults.

## Decision

Keep `hunt_stalk_chase_affordance_bias` dormant until the ticket-level batched activation passes continuity and stress gates.
