---
id: 210
title: tune mentor_food_security_weight
status: done
cluster: balance
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: pending
landed-on: 2026-05-07
---

## Why
209 wired a positive `colony_food_security` axis on `mentor_dse` with
`(1-w)` rebalance, dormant at weight 0.0. The axis is the path-1
substrate from 181's closeout for promoting higher-tier social
behavior when the colony is well-fed. Tune the weight via the
four-artifact methodology to predict and verify Mentor share lifts
in food-secure phases without unintended side-effects (the 181
cascade should NOT reproduce because the lift is *additive* on
Mentor rather than *suppressive* on Hunt/Forage).

## Scope
- Single-seed iteration with hypothesis under
  `docs/balance/210-mentor-food-security.md`.
- Predict Mentor action share rises by ~1–3pp when food security is
  high; Patrol share unchanged (key differentiator vs 181).
- Continuity canary `mentoring` within ±20% of post-209 baseline
  (199 in `logs/tuned-42` post-209).
- Survival hard gates pass.

## Out of scope
- Multi-seed sweep; follow standard tuning discipline (single-seed
  first, sweep only if the single-seed result is ambiguous).
- Coordinated tuning of sibling weights (caretake / coordinate /
  groom / patrol). Each gets its own ticket so concordance is
  attributable.

## Current state
209 substrate landed at SHA c970ad442163 with weight 0.0 (dormant).

## Approach
Lift `mentor_food_security_weight` from 0.0 → small positive value
(suggest 0.10 first iteration; mirrors hunt_food_security_weight's
iter-2 weight from 181 but in additive direction). Run
`just hypothesize specs/210-mentor.yaml` once that spec is drafted,
or `just soak-trace 42 Wren` + `just verdict` for single-seed first.

## Verification
- `just verdict` exit 0 or 1.
- `just frame-diff` shows Mentor's per-cat |Δ mean| score lifts.
- `mentoring` canary in healthy band.
- Concordance table at the bottom of the balance doc.

## Log
- 2026-05-07: opened from 209 closeout.
- 2026-05-07: 209 already landed at c970ad442163; manually unblocked
  (the ticket was opened *after* 209's `just land`, so the auto-unblock
  pass had no dependents to walk). Status flipped to `in-progress`.
- 2026-05-07: 2026-05-07: landed at 0.10. Mentor share flat (0.39pct to 0.35pct); cohesion canaries lift (mentoring +30pct, grooming +53pct, courtship +187pct). Hard-gate breach: 9 starvations vs baseline 2 — root cause is mate_dse not gating on colony_food_security (bonded couples breed into famine). Substrate-level finding parked for later substrate work; not pursued via balance whackamole here.
