---
id: 183
title: Paired-axis lift on higher-tier DSEs OR Patrol-collision investigation (181 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: null
landed-on: null
---

## Why

Ticket 181 iteration 1 falsified the substrate-level assumption
behind the `colony_food_security` saturation axis: that
suppressing Hunt+Forage in well-fed periods would naturally lift
higher-tier DSEs (groom / mate / mentor / coordinate) via L3
softmax rebalance.

Empirical result (seed-42 soak, weights 0.20/0.15):
- Forage suppressed as predicted (-8.6 pp)
- **Patrol absorbed +15 pp** of the freed bandwidth
- GroomOther / Coordinate / Mentor all *dropped*
- colony_score nourishment crashed to zero
- Continuity canaries grooming -34%, mentoring -83%,
  mythic-texture -100%

Full numeric breakdown:
[`docs/balance/181-hunt-forage-saturation-tune.md`](../../balance/181-hunt-forage-saturation-tune.md).

This ticket investigates the structural cause and proposes one
of two redesigns. Until 183 lands, ticket 181's weights stay at
zero and the saturation axis is inert.

## Direction

Two candidate paths — pick one (or sequence them) after a layer-walk
audit:

### Path A: Paired axis on higher-tier DSEs

Add a positive-curve `colony_food_security` axis to GroomOther,
Coordinate, Mentor, and Mate DSEs. Symmetric to Hunt/Forage's
inverted suppression: when food security is high, *boost* these
DSEs (not just remove suppressors). Same scalar, opposite curve.

**Hypothesis to test:** the L3 softmax landscape pins higher-tier
DSE scores below Patrol's baseline because their score formulas
don't respond to `colony_food_security` at all. Rebalancing requires
*both* a suppressor on Tier-1 AND a lifter on higher tiers.

### Path B: Patrol-collision investigation

Patrol absorbed the freed bandwidth. Audit Patrol's score
composition (`src/ai/dses/patrol.rs`) — does its baseline sit just
below Hunt/Forage's, so that any reduction in Hunt+Forage hands
selection to Patrol? If yes, the substrate works as designed but
Patrol needs its own rebalance (perhaps a `colony_safety`-style
saturation axis of its own) before saturation suppression can
benefit higher-tier DSEs.

**Hypothesis to test:** Patrol's L2 score is the immediate runner-up
to Hunt+Forage in this softmax temperature; freeing bandwidth from
Forage cascades to Patrol regardless of higher-tier DSE state.

## Investigation hooks

- `just q trace logs/tuned-42 Wren --layer=L2 --top-dses=15` —
  full L2 ranking from the iteration-1 soak. If Patrol's avg score
  is close to Hunt/Forage's, Path B is implicated.
- `just inspect Wren` against `logs/tuned-42` — full personality +
  decision history under the iteration-1 weights.
- Audit `src/ai/dses/patrol.rs`, `groom_other.rs`, `mentor.rs`,
  `coordinate.rs` for whether they read `ctx_scalars["colony_food_security"]`.
  Currently only Hunt and Forage do (per stage 5).

## Out of scope

- Re-running the 0.20/0.15 weight test — the directional miss is
  decisive; reproducing it on more seeds adds nothing.
- Changing the saturation curve shape on Hunt/Forage — that's
  ticket 181's scope, parked behind this one.
- Fixing Patrol's score composition without first deciding Path A
  vs Path B.

## Verification

- A clear layer-walk audit (L1 markers → L2 axes → L3 softmax →
  Action→Disposition) classifying the structural defect: is it
  missing-axis-on-higher-tier (Path A) or Patrol-runner-up
  (Path B)?
- Whichever path is chosen, a fresh hypothesis-prediction-
  observation-concordance thread (CLAUDE.md balance discipline)
  before constants change.
- 181 unparks (or rescopes) once 183 lands.

## Log

- 2026-05-05: opened from ticket 181 iteration-1 closeout.
