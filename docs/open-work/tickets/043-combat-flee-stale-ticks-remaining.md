---
id: 043
title: Combat-side Action::Flee left ticks_remaining stale, locking cats out of evaluate_and_plan during sustained threats
status: in-progress
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [combat.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

A 1-hour collapse-probe soak (`logs/collapse-probe-42/`, seed 42, 17 in-game years, full extinction) surfaced a Calcifer-shaped death: starvation at tick 1,250,552 with food stockpile at ~97% capacity. Drilling in: from tick 1,243,802 (last `PlanCreated`) through the Death event 6,750 ticks later, Calcifer logged exactly **one** `PlanCreated`, **zero** `PlanInterrupted` events, and **67 `CatSnapshot`s with `current_action="Flee"` byte-for-byte unchanged**. `last_scores` stayed frozen at the values from his last plan (`Eat=0.053`, ranked 12th of 14) while hunger drained from 0.62 → 0.00. Identical shape to ticket 042, different urgency path — and the 042 fix did not cover it.

## Root cause

Three code paths set `Action::Flee`:

| File | After Flee, `ticks_remaining =` |
|---|---|
| `src/systems/disposition.rs:229` (ThreatDetected interrupt) | `0` ✓ |
| `src/systems/goap.rs:~2207` (ThreatNearby urgency preempt) | `0` ✓ |
| `src/systems/combat.rs:577` (combat-system flee from wildlife) | **`flee_action_duration.ticks(...)` = 15 ticks** ✗ |

The combat path is the odd one out. While `flee_action_duration` is non-zero, `evaluate_and_plan`'s gate at `src/systems/goap.rs:975` (`if current.ticks_remaining != 0 { continue; }`) skips the cat — no replan, no `last_scores` update, hunger drains uninterrupted. Because the combat system re-evaluates wildlife threats every tick, persistent threat presence (ambient shadow-fox, tier-3 corruption late in a long run) **refreshes `ticks_remaining` faster than it can decay to 0**, locking the cat in Flee indefinitely. Calcifer's pattern in the collapse probe matches exactly: 6,750 ticks of Flee with continuous nearby fox activity, hunger from 0.62 to 0.0, no plan ever fires.

The `current.target_position = None` comment on the line above ("will be recalculated next evaluate_actions") was already counting on next-tick re-evaluation — but the very next line blocks it.

## Fix

One-line change at `src/systems/combat.rs:577` plus deletion of the now-unused constant.

```rust
// Make fleeing cats switch to Flee action.
for cat_entity in &cats_to_flee {
    if let Ok((_, mut current, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
        current.action = Action::Flee;
        current.ticks_remaining = 0;  // re-evaluate next tick — sibling Flee paths in disposition.rs:229 and goap.rs ThreatNearby do the same; ticks_remaining > 0 here was locking cats out of evaluate_and_plan during sustained threats (ticket 043, mirrors 042 for a different urgency path)
        // Keep target_position — will be recalculated next evaluate_actions.
        current.target_entity = None;
    }
}
```

`flee_action_duration` (`src/resources/sim_constants.rs:292,353`) had this as its only read-site. Delete the field and its `#[serde(alias = "flee_action_ticks")]` line; it becomes a config knob with no consumer.

## Verification

- `just check` + `cargo test` (no test references `flee_action_duration`).
- Re-run the collapse probe (`cargo run --release -- --headless --seed 42 --duration 3600`, fresh dir). Expected: cats no longer log multi-thousand-tick `current_action="Flee"` runs, `last_scores` updates regularly across the run, starvation cascade either resolves or shifts cause (Eat curve is the next bug — ticket 044).
- `just verdict logs/collapse-probe-42-fix-043` — Starvation gate may still fail because of the cliff-curve issue (Bug B / ticket 044), but the *Calcifer-shaped* lock pattern (Flee>1000 ticks with hunger draining) should be gone.

Not a balance change — this is restoring intended behavior. No four-artifact methodology needed.

## Out of scope

- The Eat-disposition curve being too cliff-like (`hangry()` Logistic(8, 0.75) means Eat does not realistically compete until hunger < 0.3) — separate ticket 044, since that is a balance change.
- Any tuning of combat-system flee mechanics (mood, distance, target reset) — only the `ticks_remaining` reset is in scope here.

## Log

- 2026-04-27: Ticket opened during collapse-probe analysis. Calcifer's 6,750-tick Flee lock is the smoking gun; the fix mirrors ticket 042 for a different urgency code path.
