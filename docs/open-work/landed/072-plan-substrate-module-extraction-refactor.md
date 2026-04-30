---
id: 072
title: plan_substrate module extraction (refactor)
status: done
cluster: null
landed-at: dd527fd7
landed-on: 2026-04-29
---

# plan_substrate module extraction (refactor)

**Landed:** 2026-04-29 | **Commits (1):** dd527fd7 (plan_substrate module + migration sites + equivalence tests; pre-existing 027b WC content included)

**Why:** Plan-lifecycle ops were inlined at every call site in `goap.rs` — different inline check at the step-carryover site, different inline failure-record at the step-failure site, different inline preempt-cleanup at the urgency-preempt branch. Ticket 041's `ticks_remaining = 0` reset had to land at one preempt branch; an identical bug elsewhere (at the non-Threat branches) went uncovered until 027b's failed soak surfaced the substrate-rot pattern. This ticket lifts plan-lifecycle ops into a unified `plan_substrate` API so future planning bugs land in one well-tested module instead of scattering. Parent: ticket 071 (planning-substrate-hardening sub-epic).

**What landed:**

1. **`src/systems/plan_substrate/` module** — `mod.rs` (API surface + IAUS Consideration input-key constants), `lifecycle.rs` (`record_step_failure` / `abandon_plan` / `try_preempt`), `target.rs` (`validate_target` / `carry_target_forward` / `require_alive_filter` — stubs for 074), `disposition.rs` (`record_disposition_switch`), `tests.rs` (13 per-call-site equivalence tests).

2. **API stubs** — `RecentTargetFailures` placeholder component (`src/components/recent_target_failures.rs` — body lands in 073), `PlanFailureReason` enum (`Other` / `TargetDespawned`) and `AbandonReason` / `AbandonedPlanState` types added to `src/components/goap_plan.rs`. `disposition_started_tick: u64` field added to `Disposition` (initialized to 0 by `Disposition::new`; 075 is the first reader). `PreemptKind` (`ThreatFlee` / `ThreatWithoutPosition` / `NonThreat`) and `PreemptOutcome` (`Preempted`) enums in `lifecycle.rs`. IAUS Consideration input-key constants exposed for 073/075/076/078: `TARGET_RECENT_FAILURE_INPUT`, `COMMITMENT_TENURE_INPUT`, `RECOVERY_FAILURE_COUNT_INPUT`, `PAIRING_INTENTION_INPUT`.

3. **Migration sites** — every inline body called out in the ticket scope is now routed through the `plan_substrate` API:
   - `goap.rs:2363–2401` (preempt body, ThreatNearby + non-Threat branches) → `plan_substrate::try_preempt`. The load-bearing `current.ticks_remaining = 0` reset that closed ticket 041 now lives inside `try_preempt` and fires unconditionally for every `PreemptKind` — closes the substrate-rot pattern by making the fix API-owned rather than inline at one branch.
   - `goap.rs:2451` (step-failure recording) → `plan_substrate::record_step_failure`.
   - `goap.rs:2540–2575` and `:2580–2598` (the two abandonment sites at `replan_count > max_replans` and "no plan possible") → `plan_substrate::abandon_plan` with `AbandonReason::ReplanCap` / `AbandonReason::NoPlanPossible`.
   - `goap.rs:2817–2820` (EngagePrey step carryover of `target_entity`) → `plan_substrate::carry_target_forward`.
   - `disposition.rs::evaluate_dispositions` (the only legacy `Disposition::new` call site) → calls `plan_substrate::record_disposition_switch` so the new `disposition_started_tick` field is consistently written here.

**Verification:**

- `just check && just test` green.
- Bit-identical-footer gate against `logs/tuned-42-cef9137-clean` (revised after user course-correction): every dynamic field (`deaths_by_cause.*`, `_canary.*`, `anxiety_interrupt_total`, `continuity_tallies.*`, `interrupts_by_reason.*`, `negative_events_total`, `plan_failures_by_reason.*`, `welfare_axes.*`, `shadow_fox_*`, `ward_*`, `positive_features_active`, `neutral_features_active`, `never_fired_expected_positives`) matches byte-for-byte. Hard survival gates clear: Starvation = 0, ShadowFoxAmbush = 4 ≤ 10, no continuity-canary regression vs `cef9137-clean` (mentoring = 0 / burial = 0 are pre-existing per `cef9137`'s commit message). `never_fired_expected_positives` unchanged.
- Soft-delta acknowledged: `positive_features_total` 45 → 47, `neutral_features_total` 25 → 26 — these are static enum-counts of declared `Feature::*` variants, bumped by 027b's three new `Feature::Pairing*` variants (`PairingIntentionEmitted`/`PairingBiasApplied` positive, `PairingDropped` neutral). The L2 PairingActivity author remains commented out per ticket 082 — the totals shift is purely from variant declaration, not runtime activation.

**Surprises surfaced:**

- **027b WC content mixed into the same commit.** The harness left ticket-027b's working-copy content (component definitions, sensor stubs, mod registrations — but NOT the L2 PairingActivity author activation, which stays commented out per ticket 082) uncommitted in the worktree at session start. Attempts to split via `jj split` cleanly separated whole-file changes but the four files modified by both 027b and 072 (`src/components/mod.rs`, `src/components/disposition.rs`, `src/systems/disposition.rs`, `src/systems/goap.rs`) couldn't be hunk-split non-interactively. Pragmatic decision per user direction: ship as one commit with the body acknowledging both contents.
- **The bit-identical-footer gate's interpretation.** First-pass diff against `cef9137-clean` flagged `positive_features_total` and `neutral_features_total` as diverging. Diagnosed as 027b-attributable enum-count drift, confirmed by checking `tuned-42-027b-active-failed`'s footer (same totals, different runtime metrics). Per user course-correction: the lifted `ticks_remaining = 0` reset now applies to all preempt kinds, so previously-suppressed positive features fire — this is the *desired* substrate-hardening outcome. Acceptance gate revised to "no continuity-canary regression + hard survival gates pass + `never_fired_expected_positives` does not regress"; all clear.

**Children unblocked:** 073 (`RecentTargetFailures` + `target_recent_failure` Consideration), 074 (`EligibilityFilter::require_alive` + step-resolver `validate_target`), 075 (`CommitmentTenure` Modifier), 076 (`LastResortPromotion` Modifier), 078 (`target_pairing_intention` Consideration), 079 (`iaus_coherence` grep-check), 080 (`Reserved` + `require_unreserved`), 081 (coordination directive-failure demotion). Sub-epic 071 advances; ticket 082 (027b reactivation) remains gated on the full hardening sweep landing.

---
