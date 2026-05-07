---
id: 209
title: Positive colony_food_security axis on higher-tier DSEs
status: ready
cluster: balance
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: null
landed-on: null
---

## Why

Ticket 181 closed with the saturation axis on Hunt/Forage shipping
dormant: two iterations (0.20/0.15 and 0.10/0.07) both produced a
predator-exposure cascade — freed L3 bandwidth flowed to Patrol,
Patrol routed cats through ShadowFox territory, an ambush wave
thinned the labor pool, and the food economy collapsed via
delayed starvation rather than direct Hunt suppression. The
"suppress lower tier → higher tier elevates passively" Maslow-
ascent assumption is wrong for this softmax landscape.

This ticket implements the path-1 alternative documented in
`docs/balance/181-hunt-forage-saturation-tune.md` §Recommendation:
a *positive* `colony_food_security` axis added to higher-tier
DSEs (Groom / Mentor / Coordinate / Caretake) so freed bandwidth
has somewhere to flow that doesn't elevate Patrol's ecological
side-effects.

## Scope

- Add a `colony_food_security` axis with non-inverted curve
  (`Composite{Logistic(8.0, 0.5)}`, no `Invert`) to GroomOther,
  Mentor, Coordinate, and Caretake DSE compositions.
- Wire weights as `groom_food_security_weight`,
  `mentor_food_security_weight`, etc. in
  `src/resources/sim_constants.rs`. Ship dormant at 0.0 initially;
  the axis is wired but inert until a tuning iteration lifts it.
- RtEO auto-rebalance via `(1 - weight)` for each DSE so its other
  axes shrink proportionally — same pattern 176 used for Hunt /
  Forage.

## Out of scope

- Revisiting Hunt/Forage suppression — 181 closed that path.
- Changes to the `colony_food_security` scalar formula
  (`min(food_fraction, hunger_satisfaction)`) — separate ticket
  if the simple form proves insufficient.
- Repricing Patrol's predator-exposure cost — independent design
  alternative (181 §Recommendation path c); a separate ticket if
  desired.
- Multi-seed `just hypothesize` sweep on the new weights —
  follow standard balance-tuning discipline, single-seed first.

## Current state

Substrate work only. The Hunt/Forage saturation axis from 176 stays
inert at 0.0/0.0. The 181 cascade mechanism is documented in
`docs/balance/181-hunt-forage-saturation-tune.md` §Mechanism and in
the auto-memory entry "L3 freed-bandwidth flows to Patrol" — both
should be re-read before tuning the new positive-lift weights.

## Approach

1. **Stage A — wire the substrate.** Register a positive
   `colony_food_security` MarkerConsideration on the four
   higher-tier DSEs. Match the curve shape Hunt/Forage already use,
   inverted: `Composite{Logistic(8.0, 0.5)}` (no `Invert`) so the
   axis output rises with food security.
2. **Stage B — verify dormancy.** Run `just check`, `just soak 42`,
   `just verdict` with weights at 0.0. Footer should match the
   post-184 baseline (`logs/tuned-42-pre-181-iter2/` is preserved
   for this comparison).
3. **Stage C — tune.** Lift weights via the four-artifact
   methodology. Predict that higher-tier DSE action shares rise
   when fs is high, *without* Patrol absorbing freed bandwidth
   (because there's nowhere "freed" — the axis adds positive lift
   rather than suppressing rivals).
4. **Stage D — observe predator-exposure regression.** Even with
   the positive axis, Patrol's L2 score baseline is unchanged. The
   colony's ShadowFoxAmbush count should match the pre-181-iter2
   baseline (1 death vs iter-2's 6) because cats aren't being
   pushed into Patrol territory.

## Verification

- Hard survival gates pass: Starvation == 0, ShadowFoxAmbush <= 10,
  all six continuity canaries fire.
- Stage B: action distribution within ±2 pp of post-184 baseline
  for every action when weights are 0.0.
- Stage C: Mentor / Coordinate / GroomOther action shares lift in
  the predicted direction when food security is high; Patrol
  action share *unchanged* (key differentiator vs 181 mechanism);
  ShadowFoxAmbush deaths don't spike.
- Continuity canaries grooming and mentoring within ±20% of
  baseline before promotion.
- Drafted balance doc under `docs/balance/209-*.md` before any
  weight ships at non-zero.

## Log

- 2026-05-07: opened from 181's closeout. The cascade mechanism
  documented in 181's balance doc is the load-bearing prior — read
  it before drafting any iteration here.
