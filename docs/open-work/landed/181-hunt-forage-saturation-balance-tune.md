---
id: 181
title: Balance-tune Hunt/Forage colony_food_security saturation weights (176 follow-on)
status: done
cluster: balance
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: pending
landed-on: 2026-05-07
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
  weights at 0.0) and `logs/tuned-42-pre-184/` (iteration 1, weights
  0.20/0.15 — kept for reference, do not promote).
- 2026-05-06: **unparked.** Ticket 183 closed by 184's fix
  (`4db67313`); the iteration-1 collapse was an artifact of
  `CanHunt` over-gating on `Injured`, not a substrate-design
  problem. With the over-gating removed, post-184 seed-42 soak
  shows the substrate-design assumption holds: higher-tier
  DSEs recover bandwidth naturally (continuity courtship 0 →
  1405, grooming 188 → 678, mentoring 21 → 409). Iteration 2
  retests 0.20/0.15 weights against the now-stable post-184
  baseline (`logs/tuned-42` at SHA `4db67313`); expected
  outcome shifts now that Patrol's spurious +15pp gain is
  gone. Likely no second iteration needed — the post-184 soak
  already demonstrates 41% of ticks at stockpile ≥15, peak
  50/50, all canaries firing — but the four-artifact
  methodology still applies if weights move.
- 2026-05-07: **iteration 2 ran and REVERTED.** Weights selected
  from prior-soak data, not from `just hypothesize`: 0.10 / 0.07
  (Hunt / Forage), recalibrated downward from iter-1's 0.20/0.15
  because post-184 fs ≈ 0.985 makes the same numerical weight
  ~49× more effective than iter-1's fs ≈ 0.008. `just verdict
  logs/tuned-42` failed: 1 starvation death (Wren, tick 1,255,465),
  6 ShadowFoxAmbush deaths (vs baseline 1), aggregate −33.3%,
  courtship −98.5%, Patrol +6.86 pp (guard rail was ≤+1 pp).
  Predicted Hunt down — observed +0.87 pp (wrong direction,
  same as iter-1). Predicted Groom/Mentor/Coord up — all three
  observed *down*.
  **The 2026-05-06 reframe was falsified.** The structural
  finding from iteration 1 survives recalibration; the 184 fix
  removed the over-gating but did not change the L3 softmax
  topology. **Mechanism (newly characterized in iter-2):** the
  collapse is a second-order ecological cascade — Patrol absorbs
  freed bandwidth → Patrol routes cats through ShadowFox
  territory → ambush wave (5 deaths in 12,335 ticks early in
  the run) thins labor pool → surviving cats live in chronic
  adrenaline_flee preemption (15,614× modifier preemptions vs
  baseline 0; Wren's plan-churn cadence 3.65 ticks) → stockpile
  drain outpaces input → starvation 24,000 ticks after the
  ambush wave. The iter-1 nourishment=0.000 was the same cascade
  amplified by the 184 over-gating bug. Full mechanism in
  `docs/balance/181-hunt-forage-saturation-tune.md` §Mechanism.
  Constants reverted to 0.0/0.0. Soak archive:
  `logs/tuned-42/` (iter-2, weights 0.10/0.07, do not promote;
  Simba focal trace ends tick 1,221,820 — focal cat died).
  Baseline preserved at `logs/tuned-42-pre-181-iter2/` (weights
  0.0/0.0, post-184 healthy).
  **Next session direction:** the recommended path is no longer
  weight-tuning. Either (a) pair saturation suppression with a
  positive-lift `colony_food_security` axis on higher-tier DSEs
  (Groom / Mentor / Coordinate / Caretake) so freed bandwidth
  has somewhere to flow that doesn't elevate Patrol; (b)
  decouple Patrol from predator-exposure as a separate system;
  or (c) price predator-exposure cost into Patrol's L2 so the
  softmax doesn't naively elevate it. Path (a) opened as
  ticket 209. Existing food-related scenarios
  (`hunt_acquisition_to_kill`, `hunt_deposit_chain`,
  `hunt_deposit_chain_injured`, `picking_up_scavenging`,
  `modifier_preempts_hunt`, `farming_cycle`) all run cleanly
  under reverted constants — no codebase regression to chase.
- 2026-05-07: landed; iter-2 cascade documented in balance doc, ticket 209 opens path-1 paired-axis follow-on
