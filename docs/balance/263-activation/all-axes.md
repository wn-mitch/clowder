# 263 activation — all axes batched gate (2026-07-13)

## Hypothesis

Activating the four ticket-263 axes together should price the same substrate at all three consumers: Flee uses direct escape affordance, Patrol uses subjective threat recency at the perimeter, HuntTarget uses best current predation affordance, and EngagePrey uses Stalk-vs-Chase affordance asymmetry to choose its phase band. Because the individual Flee ambush canary floored at zero on seed 42, this batched equivalence gate used the healthy-colony stress aggregate as the measurable metric while keeping Starvation, ShadowFoxAmbush, continuity, and DSE-election share as hard gates.

## Prediction

| Field | Value |
|---|---|
| Constants patch | `flee_affordance_weight=0.5`, `patrol_threat_recency_weight=1.0`, `hunt_best_predation_weight=0.15`, `hunt_stalk_chase_affordance_bias=0.25` |
| Primary metric | `negative_events_total` |
| Direction | decrease |
| Rough magnitude band | 10–1000% |

Spec: `docs/balance/263-activation/all-axes.yaml`

## Observation

Command: `just hypothesize docs/balance/263-activation/all-axes.yaml --slug 263-activation/all-axes --rationale ticket315_batched_axis_activation`

| Artifact | Path / result |
|---|---|
| Baseline sweep | `logs/sweep-baseline-263-activation/all-axes` |
| Treatment sweep | `logs/sweep-263-activation/all-axes-treatment` |
| Observed direction | increase |
| Observed delta | +12.8% |
| Concordance verdict | wrong-direction |

Additional gate evidence:

- `just verdict --rationale ticket315_batched_hypothesis_gate logs/sweep-263-activation/all-axes-treatment/42-1` returned `concern`: survival pass, continuity `fail:play=0`, baseline status `no-baseline`.
- `just q --rationale ticket315_activation_absorption_and_canary_scan anomalies logs/sweep-263-activation/all-axes-treatment/42-1` reported `continuity/play=0`, `continuity/burial=0`, and `continuity/mythic-texture=0`.
- `just q --rationale ticket315_check_dse_election_share actions logs/sweep-263-activation/all-axes-treatment/42-1` showed the largest action share was `GroomOther` at 24.57%; no DSE/action absorbed more than 40% of elections.
- `logs/baselines/current.json` is absent in this session workspace, so `just verdict` could not perform footer drift against the active baseline registry.

## Concordance

**Verdict: reject / do not ship.** The batched activation moved the primary stress metric in the wrong direction and failed the activation-soak gate on the existing play continuity canary. No default constants were lifted.

## Decision

Ticket 315 is parked rather than implemented at this HEAD. The target-axis routing blocker from ticket 516 is resolved (`src/ai/target_dse.rs` routes every scalar through `fetch_target_scalar`), but the four-axis activation cannot honestly satisfy ticket 315 acceptance until the play canary/baseline-registry blockers are resolved and the batched stress gate is re-run.
