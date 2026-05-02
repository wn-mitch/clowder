---
id: 144
title: IntraspeciesConflictResponseFawn — appeasement valence (belly-up, slow blink)
status: blocked
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: [109, 145]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase B sub-ticket of 109's four-valence intraspecies conflict response
framework. The Fawn valence — appeasement gesture (belly-up posture,
slow blink, scent-marking by the subordinate). Predators don't accept
appeasement (ecologically incoherent), which is why predator-response
branches do not include Fawn — it's intraspecies-only by construction.

Distinct in that it requires **new** behavior infrastructure (a Submit
gesture DSE — see ticket 145), not just a modifier on existing DSEs.

## Scope

- New `IntraspeciesConflictResponseFawn` Modifier in
  `src/ai/modifier.rs` reading `social_status_distress`.
- Lifts the new `Submit` DSE (from ticket 145) — appeasement gesture
  toward the dominant nearest cat.
- Constants `intraspecies_conflict_fawn_threshold`,
  `intraspecies_conflict_fawn_submit_lift`. Defaults 0.0 (ship inert).

## Verification

- Five-phase playbook. Phase 2 focal-trace on a low-status cat near a
  high-status cat showing Submit gesture firing instead of Flight.

## Out of scope

- Submit gesture DSE infrastructure (ticket 145 — prerequisite).
- Cross-species fawn (e.g. cat appeasing a fox) — ecologically
  incoherent; predator-response does not include Fawn.
- 109 Phase A Flight (lands first).
- Phase B Freeze (142) and Fight (143).

## Log

- 2026-05-02: Opened as 109 Phase B Fawn sub-ticket alongside 109
  Phase A landing.
