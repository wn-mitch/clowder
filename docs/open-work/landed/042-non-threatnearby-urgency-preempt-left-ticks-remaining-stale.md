---
id: 042
title: Non-ThreatNearby urgency preempt left ticks_remaining stale
status: done
cluster: null
landed-at: b86df27
landed-on: 2026-04-27
---

# Non-ThreatNearby urgency preempt left ticks_remaining stale

**Landed:** 2026-04-27 | **Commits:** fix in `b86df27` (wip bundle), landing entry `533b36b`

**Why:** Ticket 041 verification on `logs/tuned-42-038-iter6/` showed 3 starvation deaths despite the founding-haul flow firing as designed. Drilling in: Mallow / Nettle / Mocha each locked at the kitchen [29, 15] with `current_action = Cook` for 13,000+ ticks, hunger draining 0.62 → 0.00, `last_scores` frozen byte-for-byte, **zero plan events** between the `PlanInterrupted` (CriticalSafety preempt of a Crafting plan) and the death. Same shape as the 038 Flee-lock but a different urgency kind and a different stale-state field.

**Root cause:** `resolve_goap_plans`'s urgency-preempt block at `src/systems/goap.rs:~2160` only reset `current.ticks_remaining` to `0` inside the `if urgent.kind == UrgencyKind::ThreatNearby { ... }` sub-block (alongside the `Action::Flee` setup). For `CriticalSafety` / `CriticalHunger` / `Exhaustion`, the preempt marked the plan exhausted and pushed `cat_entity` to `plans_to_remove` (the 038 fix dropped the `GoapPlan`) but **never reset `current.ticks_remaining`**, which still held `u64::MAX` from plan creation. Next tick, `evaluate_and_plan`'s `if current.ticks_remaining != 0 { continue; }` filter at line 974 silently skipped the cat forever — `last_scores` never re-written, no new plan made, `current.action` stuck on whatever the preempted step's `to_action()` returned. The "Cook" symptom is incidental: `GoapActionKind::to_action()` at `src/components/goap_plan.rs:273` maps all three Cook-plan steps (`RetrieveRawFood | Cook | DepositCookedFood`) to `Action::Cook`, so any Crafting plan preempted at step 0 displayed `action=Cook`.

**Fix:** One line in `src/systems/goap.rs` — add `current.ticks_remaining = 0;` unconditionally in the urgency-preempt block, outside the `ThreatNearby` sub-block. Mirrors the matching reset already present in the disposition-complete (line 2037), replan-failed (line 2076), and commitment-gate-drop (line 1943) branches.

**Verification:** `CLOWDER_FOUNDING_HAUL=1 cargo run --release -- --headless --seed 42 --duration 900` writes `logs/tuned-42-041-cook-fix/`. Survival canaries: **Starvation 0** (was 3 in iter6), ShadowFoxAmbush 5 (≤ 10 ✓), footer written ✓. Continuity tallies improve broadly (`courtship 0 → 2785`, `mythic-texture 24 → 42`, `grooming 527 → 894`); `play 3109 → 2266` is within band. Three never-fired-expected positives surfaced (`MatingOccurred`, `GroomedOther`, `MentoredCat`) — separate concern.

**Hypothesis / prediction / observation / concordance:** N/A — bug fix, not a balance change. The "drift" here is restoring intended behavior (cats re-evaluate after a non-threat preempt instead of dying frozen), which the survival-canary delta confirms.

---
