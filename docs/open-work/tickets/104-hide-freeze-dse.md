---
id: 104
title: Hide/Freeze DSE — predator-avoidance third valence ("remain still and hope")
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cats currently have Fight and Flee but no "remain still and hope" action. Real cat ethology shows freeze as a distinct predator-response — body flat against the ground, attempt to avoid detection. Required infrastructure for two downstream tickets: `AcuteHealthAdrenaline` Freeze branch (ticket 105) and `IntraspeciesConflictResponse` freeze valence (ticket 109).

## Scope

- New `Action::Hide` variant (or `Freeze` — pick name during implementation).
- New `HideFreezeDse` in `src/ai/dses/` — minimal scoring shape: gates on threat-presence + low-cover-required (cat selects a nearby low-visibility tile to hold). Action effect is "stay still + reduce sensing-visibility-to-threats for N ticks".
- New step `resolve_hide` under `src/steps/` with the standard 5-heading rustdoc preamble.
- `Feature::HideFreezeFired` classified as positive in `Feature::expected_to_fire_per_soak()` (returns false initially — rare event, exempt from per-seed canary until colony hits a scenario producing freeze regularly).
- Sensing integration: while in Hide/Freeze, cat's sensory profile reduces own visibility to threats (`cat_sees_threat_at` inverse — `threat_sees_cat_at` should account for freeze state).

## Verification

- Unit tests: DSE scores zero when no threat; non-zero when threat in sight + low-cover-tile within 2-tile range.
- Step contract test: `resolve_hide` mutates only the `CurrentAction` + per-tick freeze-counter (no movement, no resource consumption).
- Integration test: focal cat with predator approaching selects Hide when threat is too close to flee, no fight allies nearby.

## Out of scope

- Wiring it to AcuteHealthAdrenaline.Freeze (ticket 105).
- Wiring it to IntraspeciesConflictResponse.Freeze (ticket 109).
- Any "predator-loses-sight-of-frozen-cat" mechanic — that's a separate sensing-system change.

## Log

- 2026-05-01: Opened as required DSE infrastructure for two follow-ons from ticket 047's N-valence framework decision.
