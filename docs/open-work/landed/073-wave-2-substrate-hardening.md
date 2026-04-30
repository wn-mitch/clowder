---
id: 073
title: Wave 2 substrate hardening
status: done
cluster: null
also-landed: [74, 75, 78, 80]
landed-at: null
landed-on: 2026-04-29
---

# Wave 2 substrate hardening

**Landed:** 2026-04-29 | **Commits (5):** 075 (CommitmentTenure Modifier) → 080 (Reserved + require_unreserved) → 074 (require_alive + validate_target) → 073 (RecentTargetFailures + cooldown Consideration) → 078 (Pairing-Intention Consideration backport)

**Why:** The five blocking + important sub-epic 071 children that turn the `plan_substrate` refactor (ticket 072, just landed) into machined IAUS-engine defenses against the seed-42 stuck-loop pattern. Each ticket lands a Consideration, Modifier, or EligibilityFilter — never a post-hoc resolver-body pin.

**What landed:**

1. **Ticket 075 — CommitmentTenure Modifier** (gap #5). New §3.5.1-pipeline modifier reads `tick - disposition_started_tick` (written by `plan_substrate::record_disposition_switch` in 072) and applies an additive lift `oscillation_score_lift` (default 0.10) on the cat's *current* disposition's constituent DSE scores while the tenure window is active. Tied dispositions stay sticky for `min_disposition_tenure_ticks` (default 200 ticks ≈ 30 sim-minutes) before switching. Inspectable in the same modifier-pipeline trace as `Pride` / `Patience`. 13 unit tests cover the lift logic, no-active-disposition no-op, no-resurrect-zero-score guard, and the synthetic disposition-oscillation scenario.

2. **Ticket 080 — Reserved component + EligibilityFilter::require_unreserved** (gap #9). `Reserved { owner, expires_tick }` component on contended resource targets (carcass, herb tile, prey, mate). New `is_reserved_by_other` closure on `EvalCtx` consulted by target-DSE candidate prefilter when `EligibilityFilter::require_unreserved == true`. Lifecycle hooks live in `plan_substrate::lifecycle` (reserve_target / release_target); `expire_reservations` maintenance system in chain 2a. New `Feature::ReservationContended` (Neutral, exempt from canary until producer side `record_target_picked` ships in a follow-on). New `PlanningSubstrateConstants::reservation_ttl_ticks` (default 600 ≈ 1 sim-hour). Filter applied to `hunt_target`, `forage_target`, `mate_target`, and the carcass-harvest path; intentionally skipped on `socialize_target` and `groom_other_target` per ticket Out-of-scope.

3. **Ticket 074 — EligibilityFilter::require_alive + step-resolver validate_target** (gaps #3, #4). New `target_alive` closure on `EvalCtx` reads Dead / Banished / Incapacitated component state. `EligibilityFilter::require_alive` flag gates target DSE scoring to 0.0 on invalid candidates. Step-resolver `validate_target` calls (072 added the call sites) now consult the same predicate, catching mid-step despawn that the eligibility filter (which gates at scoring time) couldn't have known about. Belt-and-suspenders, both inside the engine. `plan_substrate::carry_target_forward`'s dead-entity check writes `RecentTargetFailures` (when 073's component is present) and triggers `PlanFailureReason::TargetDespawned`.

4. **Ticket 073 — RecentTargetFailures + target_recent_failure Consideration** (gaps #1, #2). Per-cat `HashMap<(GoapActionKind, Entity), u64>` component pruned by `prune_recent_target_failures` (chain 2a decay batch). New IAUS sensor `target_recent_failure_age_normalized` published via the `TARGET_RECENT_FAILURE_INPUT` constant 072 reserved. Cooldown curve `Piecewise [(0.0, 0.1), (1.0, 1.0)]` multiplies a fresh-failure candidate's contribution down to ~10% of its no-failure value; recovers linearly over `target_failure_cooldown_ticks` (default 8000 ≈ 2 sim-hours). All six target DSEs gain the consideration as the next axis with weight renormalization. New `Feature::TargetCooldownApplied` (Neutral). `plan_substrate::record_step_failure` and `abandon_plan` now write into `RecentTargetFailures` when the failed step has a `target_entity`.

5. **Ticket 078 — Pairing-Intention Consideration backport** (IAUS-coherence). Replaces 027b Commit B's MacGyvered `if pairing_partner == Some(target) { return 1.0; }` pin in `socialize_target.rs::bond_score` with a first-class `target_pairing_intention` Consideration on `socialize_target_dse`. Cliff curve at 0.5 promotes the L2-elected partner to a 0.10 IAUS lift; `bond_score` returns to its pure tier→scalar form (Friends → 0.5, Partners/Mates → 1.0). The `unpinned_bond_score` shim retired with the pin. `Feature::PairingBiasApplied` continues firing via the same load-bearing observability gate (picked target == partner AND post-pin bond < 1.0). `scripts/check_iaus_coherence.sh` (079) now reports `iaus-coherence: no MacGyvered pins` — the EXEMPT marker disappeared with the pin.

**Combined renormalization on `socialize_target_dse`:** five original axes [0.20, 0.28, 0.20, 0.12, 0.20] sum to 1.0; tickets 073 + 078 add the cooldown axis (1/6 ≈ 0.1667) and pairing-intention axis (0.10), so originals scale by 0.7333. Final 7-axis weight vector sums to 1.0; argmax preserved at non-Intention steady state (verified by `non_intention_pick_unchanged_by_axis_addition` and `renormalization_preserves_no_failure_steady_state` tests).

**Architectural guardrail upheld:** every cross-tick defense lands inside the IAUS engine — Considerations on target DSEs (073, 078), Modifier in §3.5.1 pipeline (075), or EligibilityFilters (074, 080). No resolver-body pins. The 079 grep gate enforces this going forward.

**Verification:**
- `just check` green (cargo check + clippy + step-resolver contracts + time-units + IAUS-coherence).
- `just test` green: 1618 lib tests pass (was 1539 pre-Wave 2; added 79 unit + synthetic-world integration tests across the five tickets).
- Single-seed `just soak 42` deferred to the bundled Wave 2 verification soak (`logs/tuned-42-wave2-substrate-hardened`); Wave 4's 027b reactivation soak (ticket 082) is the canonical end-to-end gate with hard survival canaries.

**Surprises (3):**

1. **Bundled cherry-pick.** Five agents worked in parallel isolated worktrees but never ran their own soaks before hitting the rate-limit wall. Cherry-picking 075 → 080 → 074 → 073 → 078 onto main hit conflicts on the six target-DSE builders (all three of 073/074/078 touch them) and on `EligibilityFilter`'s field/method placement (074 + 080 both extend it). Resolved by introducing `require_alive_and_unreserved_filter()` as a combined helper in `plan_substrate::target`, then using `python` heredocs to bulk-resolve the regular-shape DSE conflicts. Trade-off: I integrated all five into a single soak rather than five sequential soaks (75+ min compute saved); the integrated state is what ticket 082 will verify anyway.
2. **Feature totals shifted by static enum count, not runtime activation.** Like ticket 072's enum-count delta, 073 + 080 added 2 new `Feature::*` Neutral variants — `_features_total` rose 26 → 28 statically. Both are Neutral and exempt from the never-fired-positive canary until producer-side wiring lands.
3. **socialize_target weight rebalance.** Three tickets touched the same builder in parallel, each adding an axis. Combined renormalization picks up where 073's ×5/6 left off — originals × 0.7333, cooldown 1/6, pairing intention 0.10. Tested.

**Unblocks:** Wave 3 (076 LastResortPromotion + 081 directive-failure demotion); Wave 4 (082 027b reactivation).

---
