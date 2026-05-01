---
id: 107
title: ExhaustionPressure modifier — substrate axis for Exhaustion interrupt retirement
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

`InterruptReason::Exhaustion` (`src/systems/disposition.rs:315`) — same per-tick override pattern as 047's CriticalHealth. Substrate-over-override wants a kind-specific modifier reading `energy_deficit` (already in `ctx_scalars`) that lifts Sleep + GroomSelf so a tired cat selects rest unprompted.

Pressure modifier (graded ramp). Sibling to ticket 106 (HungerUrgency).

## Scope

- New `ExhaustionPressure` modifier reading `energy_deficit`.
- Lifts Sleep (largest), GroomSelf (smaller — exhausted cats sometimes groom-then-sleep as a settling ritual).
- Constants: `exhaustion_pressure_threshold`, `exhaustion_pressure_sleep_lift`, `exhaustion_pressure_groom_lift`. Default 0.0; enable via hypothesize patch.
- Phase 3 hypothesize predicting `interrupts_by_reason.Exhaustion` decreases.
- Phase 4 retire `InterruptReason::Exhaustion` branch (`disposition.rs:314-316`).

## Verification

- Same playbook as 047 / 106.

## Out of scope

- HungerUrgency (106), ThreatProximityAdrenaline (108), ThermalDistress (110).

## Log

- 2026-05-01: Opened as substrate-axis follow-on from ticket 047, applying the playbook to the energy axis.
