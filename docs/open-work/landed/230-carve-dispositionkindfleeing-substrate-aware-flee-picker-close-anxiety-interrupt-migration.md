---
id: 230
title: Carve DispositionKind::Fleeing + substrate-aware flee picker (close anxiety-interrupt migration)
status: done
cluster: pathfinder-risk-awareness
added: 2026-05-08
parked: null
blocked-by: []
supersedes: [203]
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 748c90d7
landed-on: 2026-05-10
---

## Why

The post-228 soak (seed 42, commit `bfa6b545` dirty, `logs/tuned-42/`) failed
the `Starvation == 0` hard gate with 6 adult starvation deaths plus 2 kitten
starvations. `/diagnose-collapse` traced the cause to **plan-thrash on flee**:
after the first ambush death (Wren at tick 1270197), `AcuteHealthAdrenalineFlee`
preempted plans on every surviving cat 39,536× across the run (>99.99% of all
interrupts). The proximate sequence — verified per-cat for Calcifer (first adult
to starve at tick 1287237):

| Window | Plans/tick | Interrupt rate | HuntAttempt | Final hunger |
|---|---|---|---|---|
| Pre-Wren-ambush (1240k–1270k, 30kt) | 1 / 23.7t | 9.6% | 170 | — |
| Post-Wren-ambush (1270k–1287k, 17kt) | 1 / **1.21t** | **86.5%** | 10 | 0.087 |

The flee target picker at `src/systems/disposition.rs:280-291` is the load-bearing
defect: a naive vector projection (`target = pos + (pos - threat) / |pos - threat| × flee_distance`),
substrate-blind, oblivious to whether the projected tile is *also* in a fox-scent
corridor. After fleeing into another scent zone, the modifier re-fires, the cat
re-flees to another projection, and so on — chronic preemption masquerading as
"fleeing". Cats stop hunting (170→10 attempts), inventory plans (`PickUpItemFromGround`,
`RetrieveRawFood`) fail 21,000+ times to "inventory full", food stockpile
collapses 0.55→0.00 in season 4, and starvation cascades.

**The architectural fix is to retire the last anxiety-interrupt arm.**
Tickets 106/107/108/119 already retired Starvation, Exhaustion, CriticalHealth,
and CriticalSafety arms in favor of substrate-driven modifiers. Only the
`ThreatDetected` arm (`disposition.rs:255–293`) remains, and Flee/Hide/Idle are
still in the "anxiety-interrupt class" with no `DispositionKind`. Carving
`DispositionKind::Fleeing` finishes that migration: Flee gets a goal, plan
template, completion proxy, and substrate-aware target picker via 228's
`RouteCostField`.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 substrate | `src/components/route_cost_field.rs`, `src/systems/goap.rs:1648-1698` | `RouteCostField` is built per-replan with fox-scent + corruption overlays weighted by per-cat boldness; reaches every passable tile within `MAX_COST_BUDGET=600` | `[verified-correct]` |
| L2 modifier | `src/ai/modifier.rs:1143-1264` `AcuteHealthAdrenalineFlee` | `preempts_in_flight` returns `true` whenever `flee_lift > 0` (no commitment guard); fires 39,536× in 100k ticks | `[verified-defect]` (over-preempting) |
| L2 modifier | `src/ai/modifier.rs::ThreatProximityAdrenalineFlee` | Reads `threat_proximity_derivative`; ticket 108's substrate replacement for the retired CriticalSafety interrupt arm | `[verified-correct]` |
| L2 DSE scores | `src/ai/scoring.rs:1530` | `Action::Flee` is scored at L2 alongside other actions; lifted by the modifiers above | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs:2100-2114` | Reckless-bravery override flips `Action::Flee → Action::Fight` at `boldness > 0.9 && health > threshold`; no other commit-to-flee gate | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs:243` | `Action::Idle \| Action::Flee \| Action::Hide => None` — anxiety-interrupt class, no parent disposition | `[verified-defect]` (incomplete migration; Flee should be a Disposition) |
| Plan template | `src/ai/planner/actions.rs` | None for Flee — no goal, no constituent action chain in the GOAP catalog | `[verified-defect]` |
| Completion proxy | `src/components/commitment.rs` | None for Flee — no commitment proxy because no Disposition exists | `[verified-defect]` |
| **Flee target picker** | **`src/systems/disposition.rs:280-291`** | Naive vector projection: `target = pos + (pos - threat) / \|pos - threat\| × flee_distance`. Substrate-blind. Doesn't read `RouteCostField`. Doesn't verify the projected tile is safer | `[verified-defect]` |
| Anxiety-interrupt path | `src/systems/disposition.rs:213-294` `check_anxiety_interrupts` | Last-remaining arm `ThreatDetected` invokes the projection above; retired arms (Starvation/Exhaustion/CriticalHealth/CriticalSafety) prove this path is structurally being phased out | `[verified-defect]` (legacy path; should be retired) |
| Resolver | `src/steps/disposition/...` | No `resolve_flee` — flee runs through the interrupt path, not GOAP step resolvers | `[verified-defect]` (consequence of no Disposition) |

## Fix candidates

**Parameter-level options:**
- R1 (resolver patch) — only swap the vector projection at `disposition.rs:280-291` for a `RouteCostField` lookup; leave Flee as headless action + interrupt-path. Trivial diff but doesn't address the 86.5% interrupt rate (modifier still preempts every plan).
- R2 (modifier predicate) — gate `AcuteHealthAdrenalineFlee::preempts_in_flight` on a per-cat cooldown (~30 ticks). Reduces thrash rate but doesn't make Flee a coherent activity — still no goal, no completion.

**Structural options:**

- **R3 (split — primary fix)** — Carve `DispositionKind::Fleeing` and retire the
  last anxiety-interrupt arm. The full shape:
  - `src/components/disposition.rs:188-244`: add `Self::Fleeing` variant; map
    `Action::Flee → Some(Self::Fleeing)`; `constituent_actions(Fleeing) =
    &[Action::Flee]`. Keep `Action::Idle | Action::Hide => None` (Hide stays
    marker-gated dormant per ticket 104; Idle stays no-op).
  - `src/ai/planner/goals.rs`: add `goal_for_disposition(Fleeing)` =
    `RouteCostFieldRecovered(N)` — cat has been on a low-route-cost tile for
    N ticks (hysteresis baked into the goal predicate).
  - `src/ai/planner/actions.rs`: plan template
    `[PickFleeTarget, MoveTo(flee_target), HoldUntilSafe]`.
  - `src/components/commitment.rs`: completion proxy `safety_recovered_for_N_ticks`
    (mirrors Hunting's `Deposit` and Foraging's `RetrieveRawFood` shape).
  - `src/steps/disposition/flee_target.rs` (new): `resolve_pick_flee_target`
    reads the cat's `RouteCostField` and returns the lowest-cost passable
    tile within `flee_distance` chebyshev of current position. Replaces the
    projection at `disposition.rs:280-291`.
  - `src/steps/disposition/hold_until_safe.rs` (new): `resolve_hold_until_safe`
    waits N ticks while monitoring `route_cost_at_pos < threshold` and
    `threat_proximity_derivative <= 0`.
  - `src/ai/modifier.rs:1239-1280` `AcuteHealthAdrenalineFlee::preempts_in_flight`:
    change to "preempt UNLESS the in-flight plan IS Fleeing AND its commitment
    proxy has not yet completed". This is the natural hysteresis seat — it
    composes the modifier with the new disposition's commitment.
  - `src/systems/disposition.rs:213-294`: retire `check_anxiety_interrupts`
    entirely. The `ThreatDetected` arm is no longer needed — the substrate's
    `ThreatProximityAdrenalineFlee` modifier already lifts Flee at L2; the new
    `DispositionKind::Fleeing` carries it through the GOAP planner. This
    deletion completes the 106/107/108/119 migration.

- R4 (extend) — Keep Flee as a headless action but extend the interrupt path
  with substrate awareness + commitment ticks. Strictly worse than R3 because
  it preserves the anxiety-interrupt-class oddity and forks the commitment
  story between dispositions and headless actions.

- R5 (rebind) — Map `Action::Flee → DispositionKind::Resting` (or similar
  existing umbrella). Wrong shape — Flee's goal (escape threat) and tempo
  (per-tick reactive) are unlike anything in the existing umbrellas. Listed
  for completeness; rejected on inspection.

- R6 (retire) — Delete `Action::Flee` entirely; let cats just take damage and
  die. Obviously not the fix; listed because the layer-walk template asks.

## Recommended direction

**R3 (split)** as a single ticket. The parameter-level options (R1, R2) are
strictly subsumed: R3's flee-target step resolver IS R1's substrate-aware
picker, and R3's modifier guard IS R2's hysteresis but composed with
disposition-level commitment instead of an arbitrary tick cooldown. The
disposition + modifier guard need to land together because the modifier guard
references "is the in-flight plan Fleeing" — which doesn't make sense without
the Disposition.

The (split) shape also closes the substrate-over-override discipline thread:
the anxiety-interrupt class was the override layer the substrate refactor has
been retiring. R3 is the last-arm-out completion of that migration.

## Out of scope

- **`DispositionKind::Hiding` / `DispositionKind::Idling`** — Hide is marker-gated
  dormant per ticket 104; Idle is the no-op fallback. Both stay headless until
  evidence justifies otherwise. Open as follow-ons if the soak surfaces them.
- **Tuning `*_route_cost_weight` constants non-zero** — was the original 228
  follow-on intent. Now subsumed: this ticket exercises the same
  `RouteCostField` substrate from a different read site (Flee target). After
  this lands, the dormant DSE Field axes (Patrol/Forage/Hunt/Wander/Explore)
  remain a separate tuning question — open as a follow-on after this lands
  if the soak still shows decision-time-suppression gaps in those DSEs.
- **Inventory-full plan-failure cluster (18,374 `PickUpItemFromGround` failures)**
  — separate ticket 231 covers this; orthogonal to flee path-thrash.
- **Mood-vs-survival-stress decoupling** (mood rose 0.48→0.98 while colony
  collapsed) — separate audit; open if it shows up as a continuity-canary
  failure.

## Verification

- Hard gate: `deaths_by_cause.Starvation == 0` on the canonical seed-42 soak.
- Continuity canary: post-fix, `interrupts_by_reason.modifier_preemption(acute_health_adrenaline_flee)`
  should drop ≥10× (target: < 4,000 over 100k ticks; current: 39,536).
- Per-cat smoke check: replay the post-Wren-ambush window via
  `just soak-trace 42 Calcifer`; assert plan-create cadence stays > 15 ticks/plan
  through the 1270k–1290k window (current: 1.21 ticks/plan).
- Microexperiment: extend `src/scenarios/route_cost_decision.rs` (or open a
  sibling scenario) — bold + timid cat next to a fox at known position; assert
  bold cat's flee target is within the low-cost half of `RouteCostField`,
  assert plan commits ≥30 ticks before next preemption.

## Log

- 2026-05-08: opened from `/diagnose-collapse logs/tuned-42` post-228 soak.
  Plan-thrash root cause (39,536 AcuteHealthAdrenalineFlee preempts) traced to
  the naive vector-projection flee target picker at
  `src/systems/disposition.rs:280-291`, the last surviving arm of the
  pre-substrate `check_anxiety_interrupts` system.

- 2026-05-08: linkages audit via `just similar-linkages --ticket 230`.
  **Confirmed supersede:** ticket 203 (`CriticalHealth interrupt drives
  hunt-to-starvation plan churn — concrete reproducer for ticket 119`) names
  the same root cause from a different angle. 119's structural retirement of
  the CriticalHealth interrupt arm was insufficient because the modifier-side
  preempt path (AcuteHealthAdrenalineFlee) reproduces the thrash without the
  retired arm. 230's flee-disposition + commitment-aware modifier guard is
  the actual fix; 203's verification gate folds into 230's verification
  section.

  **Hypothesis (user 2026-05-08):** "an entire class of tickets [will]
  disappear when this next flee step finally ensures cats can survive
  combat." Candidates flagged for re-audit after 230 lands and the soak
  shows starvation closes:
  - `40` (ready) — Disposition shift after 036 collapsed Courtship/Grooming
    continuity. If continuity collapse was downstream of plan-thrash, 230 +
    231 close it.
  - `41` (ready) — Founding wagon-dismantling haul starvation balance.
    Patches around starvation-while-hauling; 230's flee-commitment lets cats
    actually feed themselves between haul legs.
  - `32` (in-progress) — Starvation rebalance to IRL cat biology. The
    "starvation as attractor" pattern documented in 32's `## Why` is plausibly
    the plan-thrash spiral 230 closes; this ticket may collapse to a
    parameter-tuning follow-on rather than a structural rewrite.
  - `221` (blocked) — caretake gates on ambush-recency at kitten tile.
    Patches around kittens being orphaned by ambushed parents; if parents
    actually flee successfully, the patch may not be needed.
  - `2` (ready) — Hunt-approach pipeline failures. May be downstream of
    plan-thrash interrupting hunt approach mid-flight.
  - `19` (blocked) — Happy paths usage-worn trails. Pathfinding polish;
    composes with 230 rather than supersedes (different layer).
  - `93` (in-progress) — Substrate-over-override epic. 230 is a milestone in
    that epic (last anxiety-interrupt arm retired); doesn't supersede the
    epic but advances it.

  **Composable, NOT superseded:**
  - `140` (blocked) — Phase 3 steering pursuit/flee polish. Continuous-position
    layer (Vec2<f32>); 230 is tile-level. Compose: 230 picks the target tile,
    140's `flee()` steering accelerates smoothly toward it.
  - `141` (ready) — combat_winnability scalar. Inputs into the scoring layer
    that decides Flee vs Fight; 230 implements the Flee disposition that
    consumes that decision.
  - `136` (ready) — WoundedAlly marker for escape_viability. Same input
    layer as 141; composes.
  - `138` (ready) — MovementBudget Phase 1. Substrate input to 140's steering.

- 2026-05-10: Cluster C C3 spinout cluster (258, 261, 263, 268) opens
  substrate consumers downstream of this ticket. 261 (ActionAffordances
  substrate) generalizes per-action success scalars including
  `Affordance(Flee, perceiver, target)` — composes naturally with this
  ticket's substrate-aware flee picker (target selection physics) by
  feeding the picker's success-scalar output into the substrate. 263
  (256-cluster DSE consumers) wires Belief + Affordance into Flee DSE
  scoring; if 230 lands first, 263's Flee axes layer on top of the
  carved DispositionKind::Fleeing. 268 (Hide DSE consumer wiring) is
  the orthogonal threat-response (Hide vs Flee) that consumes the same
  substrate. Coordinate land order during impl; no hard supersession in
  either direction.
- 2026-05-10: 2026-05-10: substrate-aware Fleeing now end-to-end. Implementation shipped in wip 7c93e70c (2026-05-08, parallel session); 251 retired AcuteHealthAdrenalineFlee (substrate-over-modifier, stronger than the original R3 modifier-guard plan); 252 lifted L3 softmax filter and audited why FleeTargetPicked=0; 254 R5 closed the picker witness contract (effective_cost = cost - chebyshev_to_threat). Hard gate Starvation==0 holds on current main; Feature::FleeTargetPicked fires end-to-end in flee_commitment scenario. Continuity canary 'interrupts_by_reason.modifier_preemption(acute_health_adrenaline_flee)' is moot post-251 (modifier no longer exists). Substrate-side Flee scoring gap (Mocha boldness=0.9 + health=0.26 + safety=0.003 yielding Flee=-0.025) opened as ticket 271 — balance follow-on, not a 230 regression (cedar pre-254 footer is bit-for-bit identical).
