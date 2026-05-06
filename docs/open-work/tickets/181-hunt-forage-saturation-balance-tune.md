---
id: 181
title: Balance-tune Hunt/Forage colony_food_security saturation weights (176 follow-on)
status: parked
cluster: balance
added: 2026-05-05
parked: 2026-05-05
blocked-by: [183]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: null
landed-on: null
---

## Why

Ticket 176 stage 5 (`75586184`) wired a `colony_food_security`
saturation axis into Hunt and Forage DSEs at default-zero
weight (`hunt_food_security_weight = 0.0`,
`forage_food_security_weight = 0.0`). The substrate is in
place: scalar plumbed via `ctx_scalars`, axis added with the
canonical `Composite{Logistic(8, 0.5), Invert}` curve, weights
auto-rebalance via `(1 - saturation_weight)` so the RtEO sum
stays 1.0 at any setting.

What's missing: a balance-tuning pass that lifts the weights
from 0.0 to a value that meaningfully suppresses Hunt/Forage
elections in a well-fed colony, so L3 bandwidth flows to
higher-tier DSEs (groom / mate / mentor / coordinate) per the
Maslow-ascent design.

## Direction

Per CLAUDE.md balance-tuning discipline:

1. Hypothesis: setting `hunt_food_security_weight = 0.20`
   (and forage = 0.15) should reduce Wren-style cats' Hunting
   PlanCreated count by ~30-40% in a well-fed seed-42 soak,
   and lift Grooming / Mating / Mentoring counts proportionally.
2. Run `just hypothesize` against this prediction with the
   four-artifact methodology.
3. Iterate weights based on observation; document final values
   in `docs/balance/`.

## Investigation hooks

- `just q trace logs/tuned-42 --cat=Wren` — focal trace shows
  per-tick L2 hunt/forage breakdown with the new fifth axis
  visible. With weight 0.0 the axis output column should always
  read 0; with weight > 0 it should rise as colony food security
  climbs and drop as it falls.
- `just frame-diff` between the default-zero soak and a tuned
  soak — per-DSE drift attribution.

## Out of scope

- The substrate scalar / axis wiring — already in place.
- Changes to the saturation curve shape — start with the
  Composite{Logistic, Invert} default; tune weights first.
- Replacing the simple `min(food_fraction,
  hunger_satisfaction)` formula with starvation-recency-aware
  variants — separate balance ticket if the simple form proves
  insufficient.

## Verification

- Hypothesis-prediction-observation-concordance docs (per
  CLAUDE.md balance discipline) showing the predicted shifts
  occur within ~2× magnitude.
- Survival hard-gates pass at the new weights.
- Continuity canaries (courtship, grooming, mentoring) ≥ 1.

## Log

- 2026-05-05: opened by ticket 176's closeout. Saturation axis
  wired in stage 5; this ticket lifts the weights.
- 2026-05-05: iteration 1 ran with `hunt_food_security_weight=0.20`
  / `forage_food_security_weight=0.15`. Forage % dropped as
  predicted (-8.6 pp), but Hunt % ROSE (+2.6 pp, wrong direction)
  and the freed bandwidth flowed to **Patrol (+15 pp)**, not
  higher-tier DSEs. Continuity canaries collapsed: grooming -34%,
  mentoring -83%, mythic-texture -100%. colony_score nourishment
  axis crashed to zero (-100%); aggregate -22%; seasons_survived
  4 → 2. **Structural model error, not a tuning miss.** Constants
  reverted to 0.0/0.0. Full numeric breakdown in
  `docs/balance/181-hunt-forage-saturation-tune.md`. Parked behind
  follow-on ticket 183 (paired-axis design or Patrol-collision
  investigation). Soak archives: `logs/tuned-42-pre-181/` (baseline,
  weights at 0.0) and `logs/tuned-42/` (iteration 1, weights
  0.20/0.15 — kept for reference, do not promote).
