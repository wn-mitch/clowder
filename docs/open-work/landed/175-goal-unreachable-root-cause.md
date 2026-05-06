---
id: 175
title: GoalUnreachable plan-failure root-cause investigation (172 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: da92888b
landed-on: 2026-05-05
---

## Why

Ticket 172 (`055d54ee`) extended `EventKind::PlanningFailed.reason`
to a typed `PlanningFailureReason` enum and added a per-`(disposition,
reason)` footer aggregator. The post-172 seed-42 soak (`logs/tuned-42/`)
shows **100% of the residual plan-failure surface is `GoalUnreachable`**:

- `Cooking:GoalUnreachable` 2076
- `Herbalism:GoalUnreachable` 1663
- `Hunting:GoalUnreachable` 243
- `Foraging:GoalUnreachable` 181

Zero `NoApplicableActions`, zero `NodeBudgetExhausted`. The clean
histogram **rejects** sibling 173's premise (substrate-eligibility
gating wouldn't help) and rejects a planner-budget bump
(`max_nodes` isn't the bottleneck). What's left: A* finds applicable
actions, fully explores reachable states, and *cannot* satisfy the
goal predicate.

The defect-shape is one of:

1. **L1-marker-vs-`PlannerState` desync.** The eligibility marker
   (`HasFunctionalKitchen`, `HasRawFoodInStores`, etc.) says "go",
   but the `PlannerState` constructed in `goap.rs` doesn't carry
   the world-fact the action chain depends on (e.g.,
   `HasStoredFood`-style predicate is false even when the marker is
   true). The L3 softmax elects the Disposition; the planner's start
   state can't find a path because the substrate the marker claims
   is missing from the search space. This is the substrate-vs-
   search-state inversion `ai-substrate-refactor.md` §4.7 warns
   against.
2. **Action-effect / goal-predicate mismatch.** The action chain's
   effects don't update the predicate the goal checks. E.g., the
   Cooking goal requires `TripsAtLeast(N+1)` but only the terminal
   `DepositCookedFood` increments trips, AND its precondition isn't
   reachable from the cat's starting `Carrying` state.

Hunting + Foraging at 243 + 181 are pre-existing volumes; the
shared `GoalUnreachable` mode suggests a generalized pattern (not
just a 155 follow-on). Ticket 091 lit the lamp; 172 typed it; 175
explains it.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/dses/cook.rs:108-112`, `src/ai/dses/herbcraft_*.rs` | `CanCook ∧ HasFunctionalKitchen ∧ HasRawFoodInStores` (Cook); `forbid Incapacitated` only (Herbcraft Gather/Prepare) | `[verified-correct]` (per ticket 172 phase 1) |
| L2 DSE scores | `src/ai/scoring.rs::evaluate_single_with_trace` | DSEs score and emit `Intention::Goal { state: ... }`; goals carry the predicate the planner targets | `[suspect]` — does the goal predicate fully cover the action chain's terminal effect? |
| L3 softmax | `src/ai/scoring.rs::select_disposition_via_intention_softmax_with_trace` | picks an `Action` from the pool; `from_action` collapses to a `DispositionKind` | `[verified-correct]` |
| `PlannerState` construction | `src/systems/goap.rs:~1840` (call site 1) | builds `PlannerState` from cat needs / inventory / world; threads `markers: &MarkerSnapshot` via `PlanContext` | `[suspect]` — does PlannerState carry the same world-facts the L1 marker reflects? `HasStoredFood` is a marker (093), but is it queried by every relevant goal predicate? |
| Plan template | `src/ai/planner/actions.rs::cooking_actions` / `herbalism_actions` / `hunting_actions` / `foraging_actions` | per-Action chain with effects | `[suspect]` — do the effects update the predicate the goal checks? |
| Goal predicate | `src/ai/planner/goals.rs::goal_for_disposition` | `TripsAtLeast(N+1)` style | `[verified-correct]` shape; `[suspect]` whether the chain reaches it from arbitrary start states |
| Resolver | `src/steps/disposition/cook.rs`, herbcraft steps | runtime side; not invoked when planning fails | `[verified-correct]` — these aren't reached on `GoalUnreachable` |

## Fix candidates

**Parameter-level options:**

- R1 — bump `max_depth` from 12 (`goap.rs:1849,2531,2833`). Rejected
  if the histogram shows zero `NodeBudgetExhausted` — it does;
  budget isn't the bottleneck.

**Structural options (CLAUDE.md bugfix discipline):**

- R2 (**extend**) — promote the loose L1 markers to *substrate*
  predicates the planner reads. Today `HasFunctionalKitchen` /
  `HasRawFoodInStores` gate Cook's L1 eligibility but the planner
  doesn't re-check them in its goal-reachability search; the chain
  succeeds in the abstract but fails at the marker→PlannerState
  bridge. Make the bridge bit-exact: every L1 marker that gates a
  Disposition's election MUST appear as a `StatePredicate` on the
  goal AND be set on the `PlannerState` during construction. The
  current `HasMarker(...)` predicate is the right shape; the audit
  is whether it's threaded through every goal that needs it.
- R3 (**rebind**) — re-author the goal predicates so they reference
  what the action chain *actually produces*. E.g., Cooking's goal
  becomes "increment trips OR end with `Carrying = CookedFood
  successfully deposited`" — let the chain succeed via either
  channel rather than gating on a single terminal increment that
  might not reach.
- R4 (**split**) — separate "the cat is eligible to elect this
  Disposition" from "the cat can complete this Disposition right
  now." Today both are conflated in the L1 marker. Splitting would
  let L1 reflect *capability* (Adult ∧ ¬Injured ∧ kitchen built)
  and a separate substrate fact reflect *current achievability*
  (HasRawFood RIGHT NOW reachable). The L3 softmax could then back
  off if achievability is false even when capability is true.
  Closest precedent: `WardStrengthLow` is achievability-shaped
  while `CanWard` is capability-shaped; the split exists for ward
  but not for cooking.

## Investigation hooks

Phase 1 (cheap):

- `just q events logs/tuned-42 --type=PlanFailed` filtered by
  `disposition=Cooking` — read the `cat`, `disposition`, `reason`,
  and the snapshot fields (`hunger`, `energy`, `temperature`,
  `food_available`, `has_stored_food`) on each event. Look for
  patterns: do the failures correlate with `food_available=false`
  but `has_stored_food=true`, suggesting a marker-vs-predicate
  gap?
- Same for Herbalism — check whether the failure events fire when
  the cat's start state doesn't allow `HerbcraftGather` to reach
  `IncrementTrips`.

Phase 2 (focal trace):

- `just soak-trace 42 Bramble` (top Cooking-failure cat per ticket
  172 phase 1) — see L1/L2/L3 + per-Action `make_plan` outcomes.
- `just soak-trace 42 Heron` (top Herbalism-failure cat) — same.

Phase 3 (code audit):

- `goal_for_disposition` (`src/ai/planner/goals.rs`) for the four
  affected dispositions — what predicates does each goal carry?
- `PlannerState` construction at `goap.rs:~1840` — what fields
  does it copy from cat / world, and does each goal-predicate
  field have a matching `PlannerState` field?

## Out of scope

- Substrate-eligibility marker authoring at the personality /
  affinity layer (the ticket-173 surface). 173 is parked; pulling
  it back is a separate question.
- Balance iteration on cooked-food nutrition or ward strength —
  substrate-must-stabilize-first per CLAUDE.md substrate-refactor
  guidance.
- Wildlife planner — `core::make_plan` still returns `Option`; the
  cat planner is the one in scope here.

## Verification

- After the fix lands, the post-fix soak's
  `planning_failures_by_reason` shows Cooking + Herbalism each
  drop their `GoalUnreachable` count below 1,000, OR a documented
  hypothesis on this ticket explaining why a residual count above
  1,000 is correct (e.g., "Cooking failures track ticks where the
  Stores literally have no raw food — this is correct ecological
  behavior; the metric is an observation, not a defect").
- Hunting + Foraging `GoalUnreachable` counts unchanged within
  ±10% (regression check on the pre-existing dispositions).
- Survival canaries hold (Starvation == 0, ShadowFoxAmbush ≤ 10);
  `never_fired_expected_positives == []`; constants drift = none.
- Continuity canaries unchanged or improved.

## Resolution

The investigation surfaced two coupled defects rather than the
single PlannerState-vs-marker desync the ticket's audit table
guessed at. Both lived at the planner-veto / inventory-projection
boundary, and the fix took three coordinated changes (commit
`da92888b`):

1. **GoalUnreachable veto removal.** Six action chains
   (Building.GatherMaterials, Herbalism.GatherHerb base + sub-
   actions, Cooking.RetrieveRawFood) carried a planner-side
   `CarryingIs(Carrying::Nothing)` veto. A cat carrying *any*
   leftover item couldn't plan a Cook chain even when the kitchen
   was functional and Stores held raw food. The vetoes are now
   removed; the runtime gates inventory capacity via the typed
   transfer primitive instead.

2. **Items-are-real refactor at the deposit boundary.** The pre-
   fix `resolve_deposit_at_stores` removed ALL food from inventory
   up-front, then bailed on the first capacity miss — every item
   past the miss was silently destroyed (3028 per soak on seed-42).
   New `transfer_item_stores_to_inventory` primitive enforces the
   ordering invariant: Inventory.add succeeds *before* Stores.remove
   + entity.despawn. Three retrieve resolvers migrate to the typed
   primitive. `scripts/check_item_transfers.sh` lints any code that
   pairs `stored.remove` with `entity.despawn` outside the primitive
   (allowlisted: `eat_at_stores`, `prey.rs` raids — both genuine
   consumption events, not transfers).

3. **L2 carry-affinity scaffolding** (default-off at 1.0). New
   `Carrying::from_inventory()` centralizes the multi-slot →
   coarse-`Carrying` projection used by both planner-side
   `build_planner_state` and L2 scoring's `apply_carry_affinity`.
   Default `carry_affinity_bonus = 1.0` disables the bias; balance-
   tuning is deferred until 176's substrate stabilizes.

The structural fix exposed a downstream Maslow-ladder defect (cats
have no AI-side response to held overflow; hunt/forage L2 doesn't
saturate on colony food security; no signal for "build more
Stores"). That defect is the subject of follow-on ticket 176, which
this ticket unblocks.

## Log

- 2026-05-05: opened by ticket 172's closeout. The diagnostic
  refactor (172) typed the failure-cause taxonomy; the histogram
  showed every failure is `GoalUnreachable`; this ticket owns the
  root-cause. 173 was parked because the histogram rejected its
  capability-marker premise.
- 2026-05-05: landed at `da92888b`. Investigation surfaced
  planner-veto + items-are-real coupling; fix lands all three
  coordinated changes. Opens follow-on 176 for the Maslow-
  ladder defects the silent-loss path was hiding.
