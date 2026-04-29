---
id: 071
title: Planning-substrate hardening — gird against the stuck-cat bug class (sub-epic)
status: in-progress
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md]
landed-at: null
landed-on: null
---

## Why

027b's failed soak surfaced a pre-existing planning-substrate fragility — the same bug class as tickets 038 (Founding-haul Flee-lock) and 041 (Mallow Cook-lock): cats permanently stuck in failing plan loops because nothing in the substrate persists target-failure memory across plan abandonment, no IAUS primitive penalizes recently-failed targets, and dead targets can be picked because there's no `EligibilityFilter::require_alive`.

Each prior incident closed one inline call site. This sub-epic closes the bug class.

The triggering soak: activating L2 `PairingActivity` (027b) flipped seed-42 from `Starvation = 0` to `Starvation = 3` — not because the L2 author misbehaved (`PairingIntentionEmitted = 0`, `PairingActivity` never inserted, bias readers collapse to identical math when `pairing_partner = None`), but because the schedule perturbation drifted long-horizon RNG enough to flip a single mate selection at tick ~1.2M, and the substrate amplified that into a colony-wide cascade. Mocha hit 109 `HarvestCarcass` failures, Nettle 66 identical `TravelTo(SocialTarget)` failures, Lark 91 `EngageThreat` failures; `anxiety_interrupt_total` dropped 70%.

The substrate has three defenses today (`replan_count` cap, per-plan `failed_actions` set, commitment drop gates) but none survive plan abandonment — the cat re-picks the same blocker on the next plan. Any sufficiently-perturbing schedule change (a Bevy version bump, a new feature flag, a future system) would expose the same fragility. Hardening pays off across every future ticket.

## Scope

Coordinates child tickets 072–081. The hardening lifts plan-lifecycle ops into a unified `plan_substrate` module (downstream of IAUS) and lands every cross-tick "memory-of-failure / stay-where-you-are / force-fallback" defense **inside** the IAUS engine as a `Consideration`, `Modifier`, or `EligibilityFilter`. Ships an `iaus_coherence` grep-check in `just check` to enforce the discipline going forward.

## Out of scope

Open as follow-on after 071 lands:
- Audit gap #8 — marker / sensing staleness (separate ticket later)
- Bevy parallel-scheduler determinism audit (HashMap → BTreeMap in `MatingFitnessParams`)

## Current state

Sub-epic opened 2026-04-29. Children pending; 027b activation (ticket 082) blocked on this. The substrate audit identified 4 blocking + 3 important + 2 deferred fragilities plus 2 IAUS-coherence cleanups.

## Approach

Two layers operating as machined gears:

- **`plan_substrate` API (072)** owns plan-lifecycle: `record_step_failure` / `abandon_plan` / `try_preempt` / `carry_target_forward` / `validate_target` / `record_disposition_switch`. Future planning bugs land in this single well-tested module instead of scattering across `goap.rs` + 5 step resolvers + 6 target resolvers.
- **IAUS engine extensions (073–078, 080)** express each defense as an engine primitive: Consideration on target DSEs (073, 078), EligibilityFilter (074, 080), Modifier in §3.5.1 pipeline (075, 076).

Children:

- **072** — `plan_substrate` module extraction (refactor; bit-identical footer gate)
- **073** — `RecentTargetFailures` + `target_recent_failure` Consideration on all 6 target DSEs
- **074** — `EligibilityFilter::require_alive` + step-resolver `validate_target`
- **075** — `CommitmentTenure` Modifier
- **076** — `LastResortPromotion` Modifier + no-target step resolvers
- **077** — anxiety-interrupt cadence root-cause investigation
- **078** — backport 027b's `bond_score` pin to a `target_pairing_intention` Consideration
- **079** — `iaus_coherence` grep-check in `just check`
- **080** — `Reserved` component + `EligibilityFilter::require_unreserved` (audit gap #9)
- **081** — coordination directive-failure demotion via `RecentTargetFailures` aggregate (audit gap #10)

Order to land: 072 → (073, 074, 079 in parallel) → (075, 076, 078, 080, 081 in parallel) → 077 (independent throughout) → 082 (027b reactivation).

Tickets 080 and 081 are important hardening but not blocking for 027b reactivation — the seed-42 stuck-loop pattern doesn't involve resource contention or coordination drift specifically. They strengthen the substrate against future failure modes; 082 can ship before they land if a tight 027b unblock is preferred. Recommended: land all of 075–081 before 082 so the substrate is fully girded.

**Blocks:** 027b activation (ticket 082 reactivates).

## Verification

- All children `status: done`.
- `just soak 42 && just verdict logs/tuned-42-substrate-hardened` passes hard gates (`Starvation = 0`, `ShadowFoxAmbush ≤ 10`, all six continuity canaries ≥ 1).
- Multi-seed sweep clears across 12 runs (4 seeds × 3 reps).
- `just check` includes the new `check_iaus_coherence.sh` gate.

## Log

- 2026-04-29: Opened. Diagnosis of 027b's failed soak corrected (the "Bevy 0.18 topological-sort reshuffle" framing in 027b's Log is mechanically wrong — chain 2a uses `.chain()`, source order is enforced). The actual mechanism is long-horizon RNG drift + planning-substrate stuck-loop fragility (same bug class as tickets 038/041). Plan stored at `~/.claude/plans/working-in-users-will-mitchell-clowder-keen-planet.md`.

## Audit findings (full reference)

| # | Class | File:line | Severity | Mitigation |
|---|---|---|---|---|
| 1 | Cross-plan target memory | `goap_plan.rs:44–45` | **Blocking** | 073 — `RecentTargetFailures` |
| 2 | Target-resolver failure-awareness | `socialize_target.rs:109–137` etc. | **Blocking** | 073 — `target_recent_failure` Consideration |
| 3 | Plan-validity revalidation | `goap.rs:2817–2820` | **Blocking** | 074 — `EligibilityFilter::require_alive` |
| 4 | Stale-entity guard | `goap.rs:2817–2820` + step resolvers | **Blocking** | 074 — `validate_target` at step entry |
| 5 | Disposition oscillation | `sim_constants.rs:672–674` | Important | 075 — `CommitmentTenure` Modifier |
| 6 | Spiral-of-failure | `goap.rs:485–559` | Important | 076 — `LastResortPromotion` Modifier |
| 7 | Anxiety cadence drift | `disposition.rs:231` + `goap.rs:2414` | Important | 077 — investigation |
| — | 027b `bond_score` pin | `socialize_target.rs:193` | **IAUS-coherence** | 078 — backport to Consideration |
| — | No process gate | (no script today) | **IAUS-coherence** | 079 — `check_iaus_coherence.sh` |
| 8 | Marker staleness | `goap.rs:866`, `1072–1161` | Important | Defer (out-of-scope) |
| 9 | Resource reservation | (no `Reserved` today) | Important | 080 — `Reserved` + `require_unreserved` |
| 10 | Coordination drift | `coordination.rs:788–964` | Minor | 081 — directive-failure demotion |

Reference fixed defenses (do not reopen): ticket 041 fix at `goap.rs:2363–2383` (unconditional `ticks_remaining = 0` reset on all preempt kinds — Mallow Cook lock and Founding-haul Flee lock pattern is closed; the reset moves into `plan_substrate::try_preempt` in 072 to make it API-owned).
