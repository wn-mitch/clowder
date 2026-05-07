---
id: 211
title: tune coordinate_food_security_weight
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
Sibling to ticket 210. 209 wired a positive `colony_food_security`
axis on `coordinate_dse` with `(1-w)` rebalance, dormant at weight
0.0. Tune the weight to predict and verify Coordinate share lifts
in food-secure phases.

## Scope
- Single-seed iteration with hypothesis under
  `docs/balance/211-coordinate-food-security.md`.
- Predict Coordinate action share rises modestly when food
  security is high; survival gates pass.

## Out of scope
- Multi-seed sweep; single-seed first.
- Sibling weight coordination (210 / 212 / 213).

## Current state
209 substrate landed at SHA c970ad442163 with weight 0.0 (dormant).

## Approach
Lift `coordinate_food_security_weight` from 0.0 → small positive
value (suggest 0.10 first iteration). Single-seed soak +
`just verdict`.

## Verification
- `just verdict` exit 0 or 1.
- `just frame-diff` shows Coordinate's per-cat |Δ mean| score lifts.
- Survival gates pass; six continuity canaries non-zero.

## Log
- 2026-05-07: opened from 209 closeout.
