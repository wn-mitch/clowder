---
id: 119
title: Retire CriticalHealth interrupt — final substrate-over-override step for ticket 047
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: 1b54d04a
landed-on: 2026-05-06
---

## Why

Originally Phase 4 of ticket 047 (CriticalHealth interrupt treadmill). Deferred during 047's Phase 2 verification because the substrate (`AcuteHealthAdrenalineFlee` modifier) raises Sleep's score but plan-completion momentum prevents the cat from acting on it 98.6% of injured-window ticks. The legacy CriticalHealth interrupt's force-Flee path was doing 64% of the actual life-saving in the 047 verification soak — removing it without fixing the momentum gap would lose that contribution.

Ticket 114 fixes the momentum gap by enabling acute-class modifiers to preempt in-flight plans. Once 114 lands and the substrate's behavioral expression matches its scoring-layer dominance, this ticket retires the override.

## Scope

- Remove the CriticalHealth check at `src/systems/disposition.rs:301-302`.
- Remove the parallel CriticalHealth check at `src/systems/goap.rs:493-498` (`check_anxiety_interrupts`).
- Keep the catch-all `_ =>` branch at `disposition.rs:271-274` — Starvation/Exhaustion/CriticalSafety still flow through it (each retired by tickets 106/107/108).
- Remove `InterruptReason::CriticalHealth` enum variant once both consumers are gone.
- Remove `Feature::AnxietyInterrupt` if no other consumer remains, or rename if it survives for the other interrupt branches.
- Update tests in `disposition.rs` (4123-4124, 4187-4188 reference CriticalHealth).
- Update the interrupt catalog in `docs/systems/ai-substrate-refactor.md:4064` to reflect the removal.

## Verification

- Hypothesize spec predicting `interrupts_by_reason.CriticalHealth = 0` (true zero, not just lower) and survival canaries hold.
- Focal-trace soak: Mallow scenario at the original collapse-probe coordinates resolves cleanly via substrate alone — no force-Flee injection from the interrupt path.
- 091-style regression check: whatever scenario in 091 demonstrated "interrupts removed before substrate" failure should NOT reproduce.

## Out of scope

- The other interrupt branches (Starvation/Exhaustion/CriticalSafety) — those each have their own retirement ticket (106/107/108).
- The `interrupt_invariant.md` doc (ticket 113) — separate doctrine ticket.

## Log

- 2026-05-01: Opened as the explicit retirement of the legacy interrupt, blocked on ticket 114 (momentum gap fix). Originally Phase 4 of ticket 047 but deferred when verification revealed the substrate's behavioral expression was gated by plan-completion momentum.
- 2026-05-06: Retired the legacy CriticalHealth interrupt and activated 047 lifts in one commit. Removed the InterruptReason::CriticalHealth variant + the health-deficit predicate in check_interrupt; stripped check_anxiety_interrupts down to its remaining accumulate_urgencies role (Health/event_log/focal_capture/plan_writer/activation/commands params dropped along with CriticalHealth body); deleted critical_health_interrupts_guarding/_resting unit tests. Kept Feature::AnxietyInterrupt (still emitted by ThreatDetected/CriticalSafety branches; ticket 099 retires the variant). Promoted default_acute_health_adrenaline_flee_lift 0.0 to 0.60 and _sleep_lift 0.0 to 0.50, the spec-proposed magnitudes. Verification: just verdict on the seed-42 soak shows interrupts_by_reason.CriticalHealth = 0 (structural — variant gone), modifier_preemption(acute_health_adrenaline_flee) firing 76587 times, anxiety_interrupt_total = 0, deaths down 62% (3 vs prior 8), ShadowFoxAmbush -80% (1 vs 5), continuity holds (courtship 4778, grooming 1102, mentoring 353). Pre-existing Starvation=2 (ticket 187) and burial=0 (ticket 157) unchanged. Doctrine: ai-substrate-refactor.md interrupt catalog reduced to 2 rows (ThreatDetected, CriticalSafety); §7.5b Substrate-driven preemption updated; distress-modifiers.md "Why this replaces interrupts" gets the 119 closure note. Unblocks 099 (Modifier feature emission) and 203 (CriticalHealth Hunt-to-Starvation reproducer). Closes the substrate-over-override arc that 047 opened.
