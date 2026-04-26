---
id: 039
title: FoodCooked still silent after 036 — Cook chain execution failure downstream of CraftingHint::Cook
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 036 landed the missing `CraftingHint::Cook` branch in the live GOAP disposition router (`src/systems/goap.rs:1346-1363`). The structural fix is correct — `last_scores`, eligibility, and routing are all working — and four previously-silent reproduction positives (`ItemRetrieved`, `KittenBorn`, `GestationAdvanced`, `KittenFed`) now fire on the seed-42 deep-soak as a side effect.

But `Feature::FoodCooked` is **still in `never_fired_expected_positives`** on the post-036 seed-42 soak (`logs/tuned-42/` at the 036-landing commit). The bug is now downstream of the disposition router: even when `CraftingHint::Cook` is produced, the resulting Cook plan never reaches the `resolve_cook` step's witnessed-Advance path that would record the feature.

## Suspects

1. **`actions_for_disposition(Crafting, Some(Cook), …)` A*-cost path doesn't pick the cook chain.** The action set in `src/ai/planner/actions.rs:351` (RetrieveRawFood → Cook → DepositCookedFood) competes with whatever else is in the Crafting action pool; if a cheaper alternative exists, A* picks it.
2. **`RetrieveRawFood` step fails or no-ops.** Cats can't path to a `Stores` building, can't take a raw-food item from `StoredItems`, or the planner's `Carrying` state predicate doesn't match the runtime inventory shape after the retrieve step.
3. **`resolve_cook` precondition (`ticks >= d.cook_ticks`)** isn't reached because the plan re-enters from a different state and resets the timer.
4. **Plan failure swallowed silently.** The `plan_failures_by_reason` footer on the post-036 soak shows zero Cook-related failures — so either the plan never starts, or it's terminating via a non-failure path (Advance with `unwitnessed`, the silent-advance pattern §Phase 4c.3 / 4c.4 was supposed to prevent).

## Investigation steps

1. Run `just soak-trace 42 <cook-capable-cat>` against the post-036 build (e.g. Nettle, who scored Cook at 0.5–0.7 in the pre-fix snapshots and still has the same personality post-fix). Inspect the L3 layer: when DispositionKind::Crafting wins and `crafting_hint == Cook`, what action sequence does A* produce?
2. If the action sequence includes `RetrieveRawFood → Cook → DepositCookedFood`, drill into the L1/L2 of the steps that follow — does the cat actually reach the Stores building, take an item, walk to the kitchen?
3. If A* picks a different cheaper action, the fix lives in the action-set definition or zone-distance calculation in `src/ai/planner/actions.rs:351-380`.
4. If the plan starts but the resolver never witnesses, check `resolve_cook` for an unwitnessed-Advance escape.

## Concordance prediction

Once located: a single targeted fix (planner cost adjustment OR retrieve-step repair OR cook-step witness fix) ⇒ `Feature::FoodCooked` rises from 0 to ≥1 on the seed-42 deep-soak ⇒ never-fired-expected canary clears.

## Non-goals

- Tuning Cook RtEO weights or `cook_hunger_gate` (covered as out-of-scope by 036; same here).
- The Courtship / Grooming continuity regression caused by 036's disposition shift — that's ticket 040.

## Pointer

Pre-036 baseline soak at `logs/tuned-42-a879f43-pre-cook-fix/` (FoodCooked = 0; Cook never even scored as `current_action`). Post-036 soak at `logs/tuned-42/` (FoodCooked = 0 still; reproduction features now fire). Diff the two to localize the surviving failure mode.
