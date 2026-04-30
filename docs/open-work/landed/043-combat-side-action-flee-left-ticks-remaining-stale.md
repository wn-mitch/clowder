---
id: 043
title: "Combat-side Action::Flee left ticks_remaining stale"
status: done
cluster: null
landed-at: 9578abb
landed-on: 2026-04-27
---

# Combat-side Action::Flee left ticks_remaining stale

**Landed:** 2026-04-27 | **Commit:** `9578abb`

**Why:** A 1-hour collapse-probe soak (`logs/collapse-probe-42/`, seed 42, 17 in-game years, full extinction) surfaced a Calcifer-shaped death: starvation at tick 1,250,552 with food stockpile at ~97% capacity. Drilling in: from tick 1,243,802 (last `PlanCreated`) through the Death event 6,750 ticks later, Calcifer logged exactly **one** `PlanCreated`, **zero** `PlanInterrupted` events, and **67 `CatSnapshot`s with `current_action="Flee"` byte-for-byte unchanged**. `last_scores` stayed frozen at the values from his last plan while hunger drained from 0.62 → 0.00. Identical shape to ticket 042, different urgency path — and the 042 fix did not cover it.

**Root cause:** Three code paths set `Action::Flee`. Two (`disposition.rs:229` ThreatDetected interrupt and `goap.rs` ThreatNearby urgency preempt) correctly set `ticks_remaining = 0`. The third — the combat-system flee at `src/systems/combat.rs:577` (`wildlife_attacks_cats` → `cats_to_flee`) — set `ticks_remaining = c.flee_action_duration.ticks(...)` (15 ticks). With `evaluate_and_plan`'s gate at `goap.rs:975` (`if current.ticks_remaining != 0 { continue; }`), and the combat system re-evaluating wildlife threats every tick, persistent threat presence **refreshed `ticks_remaining` faster than it could decay to 0**, locking the cat in Flee indefinitely. Calcifer's 6,750-tick lock with continuous nearby fox activity matches exactly.

**Fix:** One line at `src/systems/combat.rs:577`: `ticks_remaining = 0` (matches sibling Flee paths). Removed `flee_action_duration` constant from `src/resources/sim_constants.rs` — its only read site was this bug.

**Verification:** `cargo test --release --lib` passes (1344 tests). `just check` (fmt + clippy + step-resolver + time-unit linters) passes. Re-run of the collapse probe (`logs/collapse-probe-42-fix-043-044/`): CriticalHealth interrupts 4,723 → 1,429 (-70%), no more multi-thousand-tick locked-in-Flee patterns. Starvation deaths 4 → 2 (further drop comes from ticket 044's curve recalibration, landed in the same session).

**Hypothesis / prediction / observation / concordance:** N/A — bug fix, not a balance change. Restoring intended behavior (cats re-evaluate after combat-side Flee is set, instead of dying frozen).

---
