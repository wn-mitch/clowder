---
id: 211
title: tune coordinate_food_security_weight
status: done
cluster: balance
initiative: []
added: 2026-05-07
parked: null
blocked-by: [209]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: e10188a986bc
landed-on: 2026-05-07
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
- 2026-05-07: 210 (sibling, Mentor) landed at bdfec651cd28 with weight 0.10 — Mentor share flat but cohesion canaries lifted (mentoring +30%, grooming +53%, courtship +187%); 9 starvations vs baseline 2 surfaced as substrate-level mate-dse food-security gating issue, parked separately. 211 unblocked; iter-1 drafting at weight 0.10 over post-210 baseline.
- 2026-05-07: iter-1 landed at weight 0.10. Coordinate share +1.86pp (2.36% → 4.22%); Mentor cross-leak detector OK (+0.29pp); never_fired_expected_positives empty; canaries non-zero modulo pre-existing burial=0. Side-effects surfaced as follow-ons: 225 (Patrol +1.45pp share rise with per-cat L2 score -22.4% — softmax-mass redistribution mechanism, not 181 cascade), 226 (GroomOther -5.05pp share with per-cat +56.9% L2 score — demographic shift hypothesis), 227 (focal-cat process-discipline: Wren is never a coordinator so frame-diff coordinate row was structurally absent; convention update needed for coordinator-DSE tuning).
