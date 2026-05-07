---
id: 213
title: tune groom_food_security_weight
status: blocked
cluster: balance
added: 2026-05-07
parked: null
blocked-by: [209]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: null
landed-on: null
---

## Why
209 wired the `FoodSecurityGroomLift` modifier (multiplicative
post-CP bonus on GroomOther: `score *= (1 + ramp · w)` over fs >=
0.5), dormant at weight 0.0. Different shape than the WS DSEs'
inner-axis pattern because GroomOther is `CompensatedProduct`
(adding a fifth axis at weight 0.0 would multiplicatively zero the
score). Tune to verify the multiplicative-bonus shape behaves
correctly.

## Scope
- Single-seed iteration with hypothesis under
  `docs/balance/213-groom-food-security.md`.
- Predict GroomOther share lifts modestly when food security is
  high; CP gates (warmth × phys_satisfaction × social_warmth_deficit)
  remain enforced.

## Out of scope
- Multi-seed sweep; single-seed first.

## Current state
209 substrate landed at SHA c970ad442163 with weight 0.0 (dormant).
GroomOther share post-209 baseline: 10.95% (recovered fully from
iter-1 broken state of 1.08%).

## Approach
Lift `groom_food_security_weight` from 0.0 → small positive value
(suggest 0.20 first iteration; multiplicative shape allows a
slightly larger weight than the inner-axis siblings since it
doesn't push other axes around).

## Verification
- `just verdict` exit 0 or 1.
- `just frame-diff` shows GroomOther's per-cat |Δ mean| score
  lifts in food-secure phases.
- Continuity canary `grooming` within healthy band.
- Survival gates pass.

## Log
- 2026-05-07: opened from 209 closeout.
