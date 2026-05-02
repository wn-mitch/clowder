---
id: 105
title: AcuteHealthAdrenaline Freeze branch — overmatched-predator response
status: done
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: 9b024a60
landed-on: 2026-05-02
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
- 2026-05-02: 103 (escape_viability) landed (2216a44); 104 landed (2a68f595). Landed Phase 1 substrate inert via the double-inert contract: lift defaults 0.0 + Hide gated by HideEligible (never authored). Uses 1.0 - escape_viability as combat_winnability proxy v1; dedicated scalar deferred. Pipeline 17 → 18.
