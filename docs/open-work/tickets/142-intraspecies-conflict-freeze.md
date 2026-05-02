---
id: 142
title: IntraspeciesConflictResponseFreeze — hold-position low-body-posture social valence
status: blocked
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: [104, 109]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase B sub-ticket of 109's four-valence intraspecies conflict response
framework. The Freeze valence — subordinate cat goes still and low-body
in dominant cat's space, hoping to de-escalate without retreating.
Distinct from 109 Phase A's Flight (subordinate withdraws) and from
105's predator-Freeze (no eye-contact, last-resort still). Reuses the
Hide/Freeze DSE infrastructure from ticket 104.

## Scope

- New `IntraspeciesConflictResponseFreeze` Modifier in `src/ai/modifier.rs`
  reading `social_status_distress` (the same scalar 109 Phase A reads).
- Lifts the `Hide` DSE (from ticket 104) when distress is high but the
  cat elects "hold position" rather than withdraw.
- Choice predicate between Flight and Freeze: TBD during impl —
  candidates include personality (boldness inverse), proximity to
  dependent, escape-tile availability.
- Constants `intraspecies_conflict_freeze_threshold`,
  `intraspecies_conflict_freeze_hide_lift`. Defaults 0.0 (ship inert).

## Verification

- Same five-phase playbook as 047. Phase 2 focal-trace pinning the
  Freeze magnitude on a synthetic subordinate-encounters-dominant
  scenario.

## Out of scope

- 109 Phase A (Flight) — lands first. This ticket is gated on both 104
  (Hide DSE) and 109 (the social_status_distress scalar + composition
  resolved with its v1 substrate work).
- Phase B Fight (143) and Fawn (144) — separate sub-tickets.
- The status-differential composition itself — owned by 109's
  Phase-3-commit perception-coupling work.

## Log

- 2026-05-02: Opened as 109 Phase B Freeze sub-ticket alongside 109
  Phase A landing.
