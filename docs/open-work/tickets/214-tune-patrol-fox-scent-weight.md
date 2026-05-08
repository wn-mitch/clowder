---
id: 214
title: tune patrol_fox_scent_weight
status: parked
cluster: balance
added: 2026-05-07
parked: 2026-05-07
blocked-by: [209]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: null
landed-on: null
---

## Why
209 wired the `fox_scent_level` cost axis on `patrol_dse`'s
CompensatedProduct composition, conditionally-added when weight >
0 (CP semantics make a weight-0 axis multiplicatively zero the
product, so the axis is only registered when balance-tuning lifts
the weight). This is path-c from 181's closeout — pricing
predator-exposure into Patrol's L2 score so cats avoid routing
through ShadowFox territory.

## Scope
- Single-seed iteration with hypothesis under
  `docs/balance/214-patrol-fox-scent.md`.
- Predict ShadowFoxAmbush deaths drop further when Patrol is
  suppressed in fox-scent-heavy zones; Patrol share decreases
  modestly; cats route around fox territory rather than through.

## Out of scope
- Multi-seed sweep; single-seed first.
- Replacing existing `FoxTerritorySuppression` modifier — that
  still applies to Hunt/Forage/Patrol/Wander multiplicatively as a
  separate damp. The new axis is L2-internal to Patrol.

## Current state
209 substrate landed at SHA c970ad442163 with weight 0.0 (axis not
registered at default). Post-209 baseline: ShadowFoxAmbush 3 (vs
older baseline 8); Patrol share 13.14%.

## Approach
Lift `patrol_fox_scent_weight` from 0.0 → small positive value
(suggest 0.20 first iteration; the axis is a CP gate, so even
modest weight has multiplicative impact). Single-seed
`just soak-trace 42 Wren` + `just verdict` + `just frame-diff`.

## Verification
- `just verdict` exit 0 or 1.
- ShadowFoxAmbush count <= 3 (post-209 baseline) or lower.
- Patrol action share decreases by 1–3pp; Hunt/Forage absorb the
  freed bandwidth (the *intended* path for path-c).
- Survival gates pass.

## Log
- 2026-05-07: opened from 209 closeout.
- 2026-05-07: parked. Investigation surfaced two structural problems
  with shipping the tune as written. (1) Double-pricing — the new L2
  axis reads `fox_scent_level` (cat-position scent), the same scalar
  `FoxTerritorySuppression` already prices multiplicatively post-CP;
  lifting the weight stacks on the same signal. (2) Destination-
  awareness gap — the ticket prediction "cats route around fox
  territory rather than through" implies destination-aware pricing,
  but the current axis is a `ScalarConsideration` reading cat-position
  only; the patrol.rs comment (lines 107-109) reserves the slot for
  "a destination-aware refinement once the SpatialConsideration variant
  lands" and that variant never landed. The structural fix lives at
  the pathfinder layer, not the DSE-score layer: A* needs to be
  scent-aware so cats route around fox territory rather than damp
  their Patrol score after-the-fact. See the new
  `pathfinder-risk-awareness` cluster (tickets 222 / 223 / 224) for
  the substrate refactor. Once 223 lands, both this axis and
  `FoxTerritorySuppression`'s damping branch are subsumed by A*-level
  path-cost; 214 likely retires entirely. Re-evaluate after 224 lands.
