---
id: 105
title: AcuteHealthAdrenaline Freeze branch — overmatched-predator response
status: blocked
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: [103, 104]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: null
landed-on: null
---

## Why

Third predator-response valence in the §6 N-valence framework. When escape is not viable AND combat is not winnable (predator approaching, overmatched, no cover within sprint range), real cats freeze flat against the ground. Distinct from Flee (escape viable) and Fight (combat winnable).

## Scope

- New `AcuteHealthAdrenalineFreeze` modifier in `src/ai/modifier.rs` reading `health_deficit` + `escape_viability` + `combat_winnability`.
- Lifts the new Hide/Freeze DSE (from ticket 104) — proposed `acute_health_adrenaline_freeze_lift` ≈ 0.70 (largest of the three valences; freeze is the last-resort response).
- Gate: `escape_viability < threshold AND combat_winnability < threshold`.
- Register after `AcuteHealthAdrenalineFight` (ticket 102) in `default_modifier_pipeline`.

## Verification

- Synthetic test: wounded cat next to wall-corner with adult fox approaching → Hide DSE wins.
- Hypothesize spec predicting reduction in wounded-cat-died-in-combat outcomes; some shift to "wounded-cat-survived-encounter" via Hide.

## Out of scope

- The `combat_winnability` scalar — open as ticket 106 if needed; v1 may use inverse `escape_viability` as a proxy.
- Hide/Freeze DSE itself (ticket 104).
- Escape-viability scalar (ticket 103).

## Log

- 2026-05-01: Opened as third predator-response branch from ticket 047's N-valence framework. Blocked by 103 (gate) + 104 (DSE).
