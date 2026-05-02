---
id: 107
title: ExhaustionPressure modifier — substrate axis for Exhaustion interrupt retirement
status: in-progress
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
- **Wrapper cleanup (per landed-112's supersession Log):** if 106 has already
  landed at the time 107's Phase 4 ships, also delete the wrapping
  `if !matches!(disposition.kind, Resting | Hunting | Foraging)` block at
  `disposition.rs:305-317` — both arms inside it are gone, the wrapper is
  dead code. If 106 hasn't landed yet, leave the wrapper for 106's Phase 4
  to remove.

## Verification

- Same playbook as 047 / 106.

## Out of scope

- HungerUrgency (106), ThreatProximityAdrenaline (108), ThermalDistress (110).

## Log

- 2026-05-01: Opened as substrate-axis follow-on from ticket 047, applying the playbook to the energy axis.
- 2026-05-02: **Phase 1 landed** at c83de3cd alongside 106/110 — modifier registered (pipeline +1), 3 ScoringConstants fields with 0.0 lift defaults (ships inert), 7 unit tests pass. Phases 2-5 remain.
