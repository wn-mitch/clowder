---
id: 143
title: IntraspeciesConflictResponseFight — territorial combat valence (same-species)
status: blocked
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: [109]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase B sub-ticket of 109's four-valence intraspecies conflict response
framework. The Fight valence — territorial combat against a
same-species rival. Distinct from 102's `AcuteHealthAdrenalineFight`
which gates on `escape_viability` under physical injury — this one
fires on social-status pressure and reads a different scalar.

## Scope

- New `IntraspeciesConflictResponseFight` Modifier in
  `src/ai/modifier.rs` reading `social_status_distress`.
- Lifts Fight when the cat's status differential supports contesting
  (not pure subordinate retreat). Personality coupling: high-temper /
  high-boldness cats more likely to elect Fight over Flight.
- Constants `intraspecies_conflict_fight_threshold`,
  `intraspecies_conflict_fight_lift`. Defaults 0.0 (ship inert).
- Suppression on Flee when Fight gate trips (mutual exclusion with
  Phase A Flight valence — same cat shouldn't simultaneously fight
  AND retreat).

## Verification

- Same five-phase playbook. Particular attention to mate-competition
  scenarios (rival-male territorial contest) which are the canonical
  Fight-valence trigger.

## Out of scope

- 109 Phase A Flight (lands first).
- Phase B Freeze (142) and Fawn (144).
- Combat damage scaling for cat-vs-cat fights — that's a separate
  combat-system ticket if cat-cat damage isn't already wired.

## Log

- 2026-05-02: Opened as 109 Phase B Fight sub-ticket alongside 109
  Phase A landing.
