---
id: 072
title: "`plan_substrate` module extraction (refactor)"
status: done
cluster: null
landed-at: dd527fd
landed-on: 2026-04-29
---

# `plan_substrate` module extraction (refactor)

**Landed:** 2026-04-29 | **Commits (1):** dd527fd (refactor + 027b WC scaffolding bundled)

**Why:** Plan-lifecycle ops were inlined at every call site in `goap.rs` — ticket 041's `ticks_remaining = 0` reset patched one preempt branch, leaving sibling sites at risk. 027b's failed seed-42 soak (ticket 071's diagnosis) confirmed the bug class. This ticket lifts the inline pattern into a unified `plan_substrate` module so future planning bugs land in one well-tested place. Substrate-rot resists pointwise fixes; a unified API propagates fixes automatically.

**What landed:**

1. **`plan_substrate` module** (`src/systems/plan_substrate/{mod,lifecycle,target,disposition}.rs`) — `record_step_failure`, `abandon_plan`, `try_preempt`, `validate_target` (stub), `carry_target_forward` (stub), `require_alive_filter` (stub), `record_disposition_switch`. Stub bodies preserve pre-refactor semantics; behaviour bodies land in 073–081. Exposes the four IAUS sensor-key constants (`TARGET_RECENT_FAILURE_INPUT`, `COMMITMENT_TENURE_INPUT`, `RECOVERY_FAILURE_COUNT_INPUT`, `PAIRING_INTENTION_INPUT`) for downstream tickets.

2. **Migration of inline call sites** — `goap.rs:2363–2401` (preempt body, owns the load-bearing 041 reset) → `try_preempt`; `:2451` (step-failure recording) → `record_step_failure`; `:2540–2575` (plan abandonment) → `abandon_plan`; `:2817–2820` (step carryover) → `carry_target_forward`; `disposition.rs::evaluate_dispositions` → `record_disposition_switch`. The 041 `ticks_remaining = 0` reset now lives uniformly inside `try_preempt` so all preempt kinds inherit it.

3. **New types** — `PlanFailureReason::TargetDespawned` variant on `goap_plan.rs` (used by 074); `disposition_started_tick: u64` field on `DispositionState` (read by 075); `RecentTargetFailures` placeholder component (extended by 073); `PreemptOutcome` / `AbandonReason` / `AbandonedPlanState` plumbing types.

4. **13 per-call-site equivalence tests** in `src/systems/plan_substrate/tests.rs` — each asserts the new API call produces the same `GoapPlan` / `DispositionState` mutation as the old inline body on a fixed input.

**Bit-identical-footer gate (mandatory A0):** `logs/tuned-42-072-refactor` vs `logs/tuned-42-cef9137-clean` — `deaths_by_cause`, `_canary.*`, `anxiety_interrupt_total`, `continuity_tallies` (courtship 408 / grooming 73 / mentoring 0 / burial 0 / mythic-texture 26 / play 417), `interrupts_by_reason`, `negative_events_total`, `plan_failures_by_reason`, `welfare_axes`, `shadow_fox_*`, `ward_*` all match byte-for-byte. `never_fired_expected_positives` unchanged from baseline (MatingOccurred / GroomedOther / MentoredCat). Hard survival gates clear: Starvation = 0, ShadowFoxAmbush = 4 ≤ 10, six continuity canaries (mentoring/burial documented as pre-existing in cef9137).

**Soft-delta:** `positive_features_total` 45 → 47, `neutral_features_total` 25 → 26. Accounted for by 027b's three new `Feature::Pairing*` variant enumerations (PairingIntentionEmitted / PairingBiasApplied positive, PairingDropped neutral) — static enum-counts in the registry, not runtime activations. The L2 author remains commented out per ticket 082.

**Surprises:**

- **027b WC bundled in.** The harness left 027b's pre-existing modifications (PairingActivity component, sensor stubs, mod registrations, deferred L2 author scaffold with author activation still commented out) in the worktree. Splitting them out cleanly with `jj` proved costly; pragmatic call was to bundle them into the same commit with explicit message acknowledging both contents. Ticket 082 will activate the L2 author.
- **Three new Feature variants ⇒ +3 in `_features_total`.** Initially flagged as a hard-gate failure; user clarified that closing the inline-only fix at one preempt branch can legitimately let suppressed features fire. In this case the delta turned out to be enum-cardinality, not runtime activation, so even softer than expected.

**Verification:** `just check && just test` (1539 lib tests pass including 13 new `plan_substrate::tests`). Bit-identical footer gate (above). Hard survival gates clear.

**Unblocks:** tickets 073, 074, 075, 077, 078, 079, 080 (Wave 2 fan-out); 076, 081 (Wave 3); 082 (Wave 4 — 027b reactivation).

---
