---
id: 108
title: ThreatProximityAdrenaline modifier — substrate axis for CriticalSafety interrupt retirement
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 5561bb04
landed-on: 2026-05-07
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
- 2026-05-07: **Phases 2 + 3 + 4 landed** (single bundle, mirroring 119's atomic activate-and-retire shape). **Soak verdict (`logs/tuned-42`):** macro outcomes bit-identical to 119-verify (`logs/tuned-42-119-verify`) — same 2 kitten starvations (Wrenkit-85 @ tick 1309110, Wispkit-78 @ tick 1312127), same `bonds_formed: 34`, same `kittens_born: 3 / surviving: 0`, same `deaths_injury: 1`, same `peak_population: 9`. Drift relative to 119-verify is ≤5% on event-emission counters (`negative_events_total` 111991 → 106281 reflects occasional 108-preempt substitutions for 119-preempt paths on transient safety-deficit drops) and ≤3% on continuity tallies. The seed-42 wildlife dynamics produce few-enough rising-derivative events that the substrate is structurally live but rarely fires on this seed; the test of behavioral expression awaits seeds with sharper threat dynamics. **Inherited regressions (NOT introduced by 108):** 119-verify already showed `Starvation == 2` (hard-gate fail per CLAUDE.md) and `continuity_tallies.burial == 0` (continuity canary fail). Verdict reports both — they fail against the 2026-05-02 pre-119 baseline regardless of 108's no-op contribution. Open follow-up ticket for the 119-introduced kitten-starvation cluster (the new bonding/mating cascade producing kittens whose adults can't keep them fed under increased fox spawn rate). **Phase 5 (post-land balance):** deferred — 108's contribution is bit-identical to 119 on this seed, so no per-axis Phase-5 hypothesize cycle is informative. The four-axis substrate (047 + 102 + 105 + 108) covers fight/flight/freeze/threat-adrenaline; 047 + 119's verification stands in for 108's per-seed verification given the bit-identical outcome. **Phase 2 (perception coupling):** new `PrevSafetyDeficit(pub f32)` Component (`src/components/prev_safety_deficit.rs`) inserted at cat spawn alongside `RecentTargetFailures`. New `update_prev_safety_deficit` system in `plan_substrate/sensors.rs`, registered `.after(evaluate_and_plan).after(resolve_goap_plans)` so the writeback runs *after* the scoring pass — chain 2a placement was wrong (it runs at tick start; would zero out the derivative). New `threat_proximity_derivative: f32` field on `ScoringContext`, populated from `max(0, (1 - needs.safety) - prev_safety_deficit)` at both production builders (`evaluate_and_plan` in goap.rs:1613 and `evaluate_dispositions` in disposition.rs:921); first-tick / lazy-insert cats see prev = now → derivative = 0. The 118 preempt path's fetch closure (`check_modifier_preemption` in goap.rs) reads the same shape via `Option<&PrevSafetyDeficit>` on its query so substrate and preempt see identical values. **Phase 3 (lift activation):** `default_threat_proximity_adrenaline_flee_lift` 0.0 → 0.60 (mirrors 047's Flee lift); `default_threat_proximity_adrenaline_sleep_lift` 0.0 → 0.50 (mirrors 047's Sleep lift). Existing `threat_proximity_adrenaline_default_inert` test renamed to `*_default_active_lifts` and updated to assert 0.50-base + 0.60/0.50 lifts under saturated derivative + viable escape. **Phase 4 (retirement):** `InterruptReason::CriticalSafety` variant + the `if needs.safety < d.critical_safety_threshold { return Some(InterruptReason::CriticalSafety) }` body removed from `disposition.rs::check_interrupt`; the `_ =>` catch-all in the match arm collapses since `ThreatDetected` is now the sole variant. The `UrgencyKind::CriticalSafety` (separate enum, `goap.rs:895`) stays — that's GOAP urgency taxonomy for plan routing, not the disposition-strip interrupt. `update_capability_markers`-style integration test patterns: 1920 lib tests pass; `just check` clean (substrate-stub lint, step-resolver lint, time-unit lint, IAUS coherence). `distress-modifiers.md` updated: 108 row → "Landed; lifts active under 108 Phase 3, `InterruptReason::CriticalSafety` retired in Phase 4"; preempts table → "scalar live as `max(0, safety_deficit_now - PrevSafetyDeficit)`". Soak verdict pending. **Phase 5 (post-land balance check):** if drift > ±10% on canary metrics, four-artifact hypothesize cycle per CLAUDE.md balance discipline; otherwise 108 closes.
