---
id: 512
title: remote starvation: Eat's spatial discount floor loses every softmax to Resting under critical hunger — no desperation lever lifts the distance discount as hunger goes critical
status: ready
cluster: balance
orchestration: substrate-sensitive
initiative: []
added: 2026-07-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [fluid-movement-phase2]
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why
Hazelkit-3 (juvenile kitten) starved at (107,8) — the map NE corner, ~77
tiles from stores — at tick 1305185 in `logs/tuned-42-f7e4be8f` (the 140
step-10 landing gate; Starvation==0 hard-gate violation). Colony food was
88–100% the whole decline. The 511 machinery all worked: elections ran,
`starvation_override` failed SelfGroom repeatedly, commitment released —
but each re-election landed in a pool where feeding COULD NOT win: Eat
scores `hunger_urgency × stores_distance` under CompensatedProduct, and
`eat.rs`'s `stores_distance` curve saturates to its `ClampMin(0.1)` floor
beyond `EAT_STORES_RANGE` (20 tiles). A starving cat 77 tiles out scores
Eat ≈ 1.0 × 0.1 = 0.1 while Resting (energy also low) scores ~0.5+; the
232 distress-sharpened softmax then picks Resting essentially every time
(276 of the kitten's final 300 plans were Resting). Real animals invert
this: at critical hunger, distance stops discounting — desperation
travels. The substrate has no lever expressing that.

## Hot context
- Run: `logs/tuned-42-f7e4be8f` (step-10 gate, FAIL on Starvation=1).
- Victim: Hazelkit-3, life_stage Kitten (juvenile band), wandered
  30,9 → 63,11 → 71,8 → 107,8 over ~8k ticks (Foraging/Exploring legs),
  hunger 0.587 → 0.0 over ~10k ticks with zero Eating elections.
- BegForFood also unwinnable: no adult within range at the corner.
- Cross-refs: 511 (weaned-kitten starvation complex — this is the
  remote-geometry sibling; 511's colony-local case is fixed), 507
  (welfare recalibration), the step-10 balance-doc iteration
  (docs/balance/fluid-movement-phase2.md Iteration 5).
- NOT step-10-specific: the discount shape predates Phase II; the
  walkabout trajectory exposed it. Any long-range wander (Exploring,
  post-flee displacement, herb-patch runs) can reproduce it.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L2 DSE scores | `src/ai/dses/eat.rs` | `stores_distance` = `Invert(Logistic(8,0.5))` post-`ClampMin(0.1)` over cost normalized by `EAT_STORES_RANGE=20`; CP composition multiplies hunger by ~0.1 beyond ~20 tiles | `[verified-correct]` (read this session) |
| L2 DSE scores | `src/ai/dses/eat.rs` | Eligibility = `require(HasStoredFood)` only — distance never gates eligibility, only score | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs` | 232 distress-sharpened temperature makes the argmax MORE likely under body distress — amplifies the Resting lock rather than breaking it | `[verified-correct]` (mechanism); exact temperature at death `[suspect]` |
| Urgency layer | `src/systems/goap.rs` (511) | Starvation urgency + `starvation_override` sentinel fire correctly; they force re-election but cannot change what wins | `[verified-correct]` (observed: 276 Resting re-plans) |
| Modifier layer | `goap.rs::evaluate_and_plan` bonus pipeline | No hunger-desperation modifier exists; threat adrenaline (`threat_proximity_adrenaline_flee`) is the precedent shape | `[verified-correct]` |

## Fix candidates

**Parameter-level options:**
- R1 — raise `ClampMin` floor (0.1 → 0.3): blunt; helps but Resting can
  still win; also flattens the near/far contrast that keeps colony-local
  cats eating locally. Rejected-by-default.
- R2 — widen `EAT_STORES_RANGE`: same blunting, same residual loss.

**Structural options:**
- R3 (**desperation modifier**) — a §3.5-style modifier-layer lift on
  feeding-family actions (Eat/Forage/Hunt/BegForFood) that scales with
  hunger criticality (0 below the critical threshold, ramping to
  neutralize the spatial discount at hunger → 0). Composes at the
  modifier layer per the single-axis-perception rule (the perception
  scalar `stores_distance` stays pure); surfaces in the L2 trace.
  Ethological: desperation reallocates travel budget toward food.
- R4 (**tier-suppression completion**) — Maslow tier-1-vs-tier-1
  arbitration: at critical hunger, Resting's energy-need input is
  suppressed by hunger criticality (starving animals sleep less).
  Touches the needs substrate more broadly; riskier blast radius.
- R5 (**belief/homing**) — "walk home when critical" as a substrate
  affordance (TravelTo(Stores) prefix already exists in Eating plans —
  the gap is purely that Eating never WINS; R5 alone doesn't fix the
  election, so it's a complement, not a fix).

## Recommended direction
R3 (desperation modifier). It is the smallest lever that changes WHO
WINS the election (the actual defect), keeps the perception scalar
orthogonal (feedback rule: compose at the modifier layer, never inside
the perception scalar), has a named precedent (threat adrenaline), and
ships trace-visible. R1/R2 are rejected as parameter blunting that
degrades the healthy near-field behavior to patch a far-field one.

## Out of scope
- BegForFood range/targeting (no adult in range is correct behavior —
  the fix is that Eat must win and carry the kitten home).
- Why the kitten wandered 77 tiles (Exploring/Foraging range tuning —
  legitimate free-range behavior under the 0.4.0 thesis).

## Verification
Seed-42 soak: Starvation == 0 with the step-10+ binary; scenario probe:
starving cat at >40 tiles from stocked stores must elect Eating within
N elections (add as a scenario preset — chain-rare events prefer
structural verification per the feedback memory).

## Log
- 2026-07-06: opened from the 140 step-10 landing-gate failure
  (`tuned-42-f7e4be8f`, Hazelkit-3). Layer walk done in-session; R3
  recommended. Landing decision for step 10 itself: the flee-travel-leg
  planner fix (this session) re-rolls the gate trajectory; if the gate
  soak still shows Starvation>0, 512-R3 blocks step 10; otherwise 512
  proceeds as an independent balance ticket.
