---
id: 077
title: anxiety-cadence root-cause investigation (no-op close)
status: done
cluster: null
landed-at: 32e67f69
landed-on: 2026-04-29
---

# anxiety-cadence root-cause investigation (no-op close)

**Landed:** 2026-04-29 | **Parent:** 32e67f69 (post-072 refactor) | **Code change:** none

**Why:** Audit gap #7 — the 70% drop in `anxiety_interrupt_total` (24,874 → 7,469) between the seed-42 clean run and the 027b-active failed run was unexplained. Three hypotheses on the table: (a) plan-replan churn racing the per-tick anxiety check, (b) `urgencies.needs.clear()` racing critical-need detection, (c) the drop is a *symptom* of the stuck-loop, not a separate bug.

**Verdict — Hypothesis (c) supported. No code change.**

**Smoking-gun evidence:** post-072 refactor footer (`logs/tuned-42-072-refactor`) reproduces `anxiety_interrupt_total = 24,874` byte-for-byte against the clean baseline (`logs/tuned-42-cef9137-clean`). The 7,469 figure is exclusive to `tuned-42-027b-active-failed`. The drop tracks 1:1 with disposition-mix corruption: in the clean run **Nettle alone** fires 22,304 of 24,874 interrupts (~90% of total) under Foraging (13,990) / Hunting (6,505) / Crafting (1,539) plans; in the failed run those recovery dispositions collapse to **zero plans created** (Foraging 0 / Hunting 0 / Crafting 83) and her plan mix flips to Resting (5,336) + Socializing (3,876) + Coordinating (2,814). `check_anxiety_interrupts` (`goap.rs:488`) **exempts `DispositionKind::Resting`** by design, so half of Nettle's low-health time spends in plans where anxiety doesn't fire; the rest is the 027b stuck-loop she never escapes.

**Hypotheses (a) and (b) rejected by code-walk:**

- (a) `goap.rs:435` `check_anxiety_interrupts` is per-tick, runs BEFORE `evaluate_and_plan`, and the CriticalHealth check at `goap.rs:488–489` is sticky-while-holding (no edge trigger, no per-cat cooldown). Replan churn does not gate it.
- (b) `urgencies.needs.clear()` at `goap.rs:2405` follows step-boundary urgency evaluation in `resolve_goap_plans`. The anxiety CriticalHealth path reads `health.current / health.max` directly from `Health`, not from `PendingUrgencies`. The clear is irrelevant to the anxiety-fire site.

**Resolution path:** the disposition-mix corruption is exactly what 073 (`RecentTargetFailures`) + 074 (`require_alive` + `validate_target`) + 076 (`LastResortPromotion`) + 078 (`target_pairing_intention` Consideration) target. Once those land and Nettle's mate-selection failure stops cascading into the Socializing/Coordinating loop, her plan mix returns to Foraging/Hunting-dominant and `anxiety_interrupt_total` returns to band. Ticket 082's reactivation soak is the natural verification.

**Children unblocked:** none (077 was independent throughout). Sub-epic 071 advances toward 082.

---
