---
id: 108
title: ThreatProximityAdrenaline modifier — substrate axis for CriticalSafety interrupt retirement
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

`InterruptReason::CriticalSafety` (`src/systems/disposition.rs:347`) — per-tick override on `needs.safety < d.critical_safety_threshold`. Substrate replacement: lurch on **rising** threat-density derivative (the cat noticed danger getting worse this tick), not on an absolute scalar — adrenaline is about change-detection, not steady-state.

Sibling to 047's AcuteHealthAdrenaline (same lurch shape, different scalar source). Like AcuteHealthAdrenaline, eligible for two-valence Flee/Fight split (and possibly Freeze via tickets 104/105 by reuse) once the substrate is in.

## Scope

**This ticket ships only the Flee valence.** Fight valence is ticket 108b (open during this work if scope allows).

- New `ThreatProximityAdrenalineFlee` modifier in `src/ai/modifier.rs` reading a new `threat_proximity_derivative` scalar.
- New scalar `threat_proximity_derivative` published via `ctx_scalars`: `max(0, threat_proximity_now - threat_proximity_prev_tick)`. Requires a `PrevThreatProximity` Component or per-cat history slot — adds per-tick state.
- Same smoothstep lurch shape as 047. Lift Flee + Sleep on rising threat.
- Gated by `escape_viability >= threshold` (ticket 103 prerequisite for this v1; if 103 isn't ready, ship with always-true predicate per 047's pattern).
- Phase 3 hypothesize predicting `interrupts_by_reason.CriticalSafety` decreases.
- Phase 4 retire `InterruptReason::CriticalSafety` branch.

## Verification

- Same five-phase playbook as 047. Particular attention: the `threat_proximity_derivative` scalar is the load-bearing change here; its accuracy gates everything downstream.

## Out of scope

- Fight valence (open as 108b once Flee lands).
- Steady-state threat-proximity (the cat in chronic danger is a different problem — could be a new "ThreatPressure" modifier, separate ticket if needed).

## Log

- 2026-05-01: Opened as third substrate-axis follow-on from ticket 047.
- 2026-05-02: **Phase 1 landed** at cd96eced — modifier registered (pipeline 15 → 16), 4 ScoringConstants fields, 7 unit tests. The `threat_proximity_derivative` scalar is published as a 0.0 stub from `ctx_scalars`; actual derivative computation (max(0, safety_deficit_now - prev)) requires a `PrevSafetyDeficit` per-cat Component + per-tick update system that lands alongside the lift activation in the same Phase-3-or-Phase-4 commit. Double-inert (lift 0.0 + scalar stub). Phases 2-5 + perception coupling remain.
