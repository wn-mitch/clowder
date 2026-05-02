---
id: 112
title: Retire per-disposition exemption lists (Resting/Hunting/Foraging/Guarding) — substrate replacement via commitment momentum
status: done
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: 2026-05-02
---

## Why

`src/systems/disposition.rs:305-342` carries per-disposition exemption lists for the interrupt cascade (Resting/Hunting/Foraging exempt from hunger interrupts; Guarding exempt from threat interrupts). Same per-tick-override pattern as the interrupt branches 047 retired, in special-case clothing.

The 047 ticket text explicitly names this cluster: "the per-disposition exemption lists at `disposition.rs:305-342` are the same pattern in special-case clothing." Substrate-over-override wants a Rao-Georgeff §7.2 commitment/momentum modifier — a cat already pursuing Hunt has implicit momentum that resists Starvation interrupts because the planner hasn't yet succeeded in completing Hunt.

## Scope

Umbrella ticket. Sub-tickets per disposition if scope demands (likely it will):
- `112a-resting-exemption-retire.md` — Resting cats already in den shouldn't reroute to a different need-fulfillment disposition. Substrate: commitment momentum on Resting that decays only after rest_complete.
- `112b-hunting-exemption-retire.md` — Hunting cats are already pursuing food. Substrate: HungerUrgency (ticket 106) + commitment-tenure modifier on Hunting.
- `112c-foraging-exemption-retire.md` — same pattern.
- `112d-guarding-threat-exemption-retire.md` — Guarding's threat exemption is its own thing; the cat IS the threat-response. Substrate: gate the ThreatDetected interrupt on `disposition.kind != Guarding` from substrate, not from imperative match.

## Verification

- Each sub-ticket follows the 047 playbook: substrate first, focal-trace verify, hypothesize sweep, retire exemption.

## Out of scope

- Retiring the §7.2 CommitmentTenure modifier itself — it's the substrate replacement, not the override.

## Log

- 2026-05-01: Opened as cleanup follow-on from ticket 047. Same pattern (substrate-over-override) as the interrupt branches retired in 047 + 106 + 107 + 108.
- 2026-05-02: Closed as superseded. The "Rao-Georgeff §7.2 commitment momentum" substrate replacement this umbrella called for is **already shipped** — `CommitmentTenure` modifier (`src/ai/modifier.rs:448-554`, ticket 075, landed 2026-04-30) + `CommitmentStrategy::should_drop_intention` (`src/ai/commitment.rs`, ticket 076). Patience modifier provides additional personality-scaled commitment lift on the same constituent-DSE class. §7.4 persistence-bonus (logistic on `completion_fraction`) remains spec-only, but that's a richer shape outside this umbrella's scope.

  Practical disposition of the four sub-pieces named in original Scope:
  - 112a/b/c (Resting/Hunting/Foraging exemption at `disposition.rs:305-317`) is dead-code cleanup that folds into tickets **106** (HungerUrgency, retires `InterruptReason::Starvation` arm) and **107** (ExhaustionPressure, retires `InterruptReason::Exhaustion` arm). The wrapping `if !matches!(...)` becomes inert when both arms retire; 106/107 Phase 4 picks up the wrapper deletion. Notes appended to both tickets.
  - 112d (Guarding/ThreatDetected exemption at `disposition.rs:319-342`) is genuinely distinct — ticket 108 retires `CriticalSafety` only, not `ThreatDetected`. Substrate (`Guarding → Blind` strategy + `CommitmentTenure` lift on Patrol/Fight constituent DSEs) is in place. If post-108 empirical verification shows the substrate is sufficient for Guards to outscore Flee under threat, retire the exemption then. **Open as a fresh ticket on demand** — not opening speculatively per the "close-not-track" disposition.
