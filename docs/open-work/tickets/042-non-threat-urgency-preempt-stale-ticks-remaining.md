---
id: 042
title: Non-ThreatNearby urgency preempt left ticks_remaining stale, locking cats out of evaluate_and_plan forever
status: in-progress
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

Ticket 041 verification (`logs/tuned-42-038-iter6/`, seed 42, `CLOWDER_FOUNDING_HAUL=1`) reproduced 3 starvation deaths despite the founding-haul flow firing as designed. Drilling into Mallow / Nettle / Mocha showed each cat **locked at the kitchen [29, 15] with `current_action = Cook` for 13,000+ ticks**, hunger draining 0.62 → 0.00, `last_scores` frozen byte-for-byte (`Patrol = 0.81166583` winning every snapshot but never taken), and **zero plan events** between the lock-in and death:

```
1205257  PlanCreated     Crafting [RetrieveRawFood, TravelTo(Kitchen), Cook, ...]
1205261  PlanInterrupted urgency CriticalSafety preempted level 4 plan, step=RetrieveRawFood
[no plan events for 13,194 ticks]
1218455  Death           cause=Starvation pos=[29, 15]
```

Same shape as the ticket 038 Flee-lock (cats stuck in `Action::Flee` for 5000+ ticks because the `ThreatNearby` preempt left `GoapPlan` attached) — but a different trigger and a different stale-state field.

## Root cause

`resolve_goap_plans`'s urgency-preempt block at `src/systems/goap.rs:~2160` only resets `current.ticks_remaining` to `0` inside the `if urgent.kind == UrgencyKind::ThreatNearby { ... }` sub-block (line 2207, alongside the `Action::Flee` setup). For every other urgency (`CriticalSafety`, `CriticalHunger`, `Exhaustion`), the preempt:

1. Marks the plan exhausted (`plan.current_step = plan.steps.len()`).
2. Pushes `cat_entity` to `plans_to_remove` (the ticket 038 fix) so the `GoapPlan` is dropped at end of system.
3. **Never resets `current.ticks_remaining`**, which still holds `u64::MAX` from when the plan was created (line ~1588, `current.ticks_remaining = u64::MAX;`).

Next tick, `evaluate_and_plan` filters cats with `if current.ticks_remaining != 0 { continue; }` (line 974). The cat has no `GoapPlan` (correct — the preempt removed it) but its `ticks_remaining` is still `u64::MAX` (stale). She is silently skipped by the planner forever:

- `evaluate_and_plan` continues past her — `last_scores` are never re-written, so they stay frozen on the values from the last full evaluation.
- `resolve_goap_plans` skips her too (no `GoapPlan`).
- `current.action` keeps the value set on the preempted step's first tick at line 2090 (`current.action = action_kind.to_action(plan.kind);`).

The "Cook" symptom is incidental but consistent: `GoapActionKind::to_action()` at `src/components/goap_plan.rs:273` maps **all three** Cook-plan steps (`RetrieveRawFood | Cook | DepositCookedFood`) to `Action::Cook`, so any Crafting plan that gets non-ThreatNearby-preempted at step 0 (RetrieveRawFood) leaves the cat displaying `action=Cook`. The same lock would surface as `Hunt` / `Forage` / `Patrol` for those plans; Cook is just the most visible because Crafting's `target_completions` is small (1 + `patience.round()`) so cats churn through Crafting plan boundaries fastest, and the kitchen is a single chokepoint tile.

## Fix

In `src/systems/goap.rs`, at the urgency-preempt block (after `plan.current_step = plan.steps.len();`), add `current.ticks_remaining = 0;` unconditionally — outside the `if urgent.kind == UrgencyKind::ThreatNearby { ... }` sub-block. The redundant set inside the ThreatNearby branch is left as-is for cohesion (it sits with the flee-target wiring).

This mirrors the matching reset already present in:
- `disposition_complete` branch (line 2037)
- replan-failed branch (line 2076)
- commitment-gate drop branch (line 1943)

## Verification

- `CLOWDER_FOUNDING_HAUL=1 cargo run --release -- --headless --seed 42 --duration 900` — `Starvation = 0`, footer written, `ShadowFoxAmbush ≤ 10`, `never_fired_expected_positives = 0`.
- No cat has more than ~1 consecutive `CatSnapshot` showing `current_action = Cook` followed by no plan events. (One stale-action snapshot tick is acceptable — the preempt happens between system boundaries within the tick.)
- Continuity tallies hold no worse than baseline.

## Out of scope

- Generalizing to a "stale ticks_remaining detector" — none of the other plan-drop paths leak it; the bug is localized to the urgency-preempt block.
- The CriticalSafety urgency tuning itself (it fired correctly here — Mallow's safety dropped below threshold; the preempt was the right call, only the cleanup was incomplete).
