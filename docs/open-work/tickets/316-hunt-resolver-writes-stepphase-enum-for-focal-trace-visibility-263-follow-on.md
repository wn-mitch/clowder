---
id: 316
title: Hunt resolver writes StepPhase enum for focal-trace visibility (263 follow-on)
status: ready
cluster: ai-substrate
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`StepPhase` enum in `src/components/goap_plan.rs` has `Stalking / Chasing / Pouncing` variants but nothing writes to the `phase` field on `StepExecutionState` — they're aspirational. Ticket 263's resolver-side affordance-biased band thresholds inside `resolve_engage_prey` would surface much more cleanly in focal-cat traces (`trace-<focal>.jsonl`) if the phase transitions were recorded on the plan's step state. Without `StepPhase` write-back, soak-trace consumers have to reverse-engineer "which sub-phase did the cat enter on tick T?" from `HuntOutcome::LostDuringStalk` / `LostDuringChase` outcomes at the end of the attempt, not from the live state.

## Scope

- Write `StepPhase::Approaching / Stalking / Chasing / Pouncing` to `state.phase` at the appropriate band transitions inside `resolve_engage_prey` (`src/systems/goap.rs:7257+`).
- Surface `state.phase` in the focal-cat trace emitter so each tick's record shows the active phase.
- Optionally surface in `ctx_scalars` as `"hunt_phase_ordinal"` for L2 trace inspection.

## Out of scope

- Splitting Hunt sub-actions into separate GOAP actions (would be a much bigger refactor — separate ticket if ever).
- Changing the band thresholds themselves (that's in 263 + 315).

## Current state

Blocked-by 263 (the resolver-side affordance bias landed there; this adds the trace-visibility wiring on top).

## Approach

1. In `resolve_engage_prey`, identify the three transition points (Approach → Stalk at `dist <= stalk_start`, Stalk/Chase decision branches, Pounce at `dist <= pounce_range`).
2. Write `state.phase = StepPhase::Stalking / Chasing / Pouncing` at each.
3. Update the focal-trace emitter to include `phase` in the per-tick record.

## Verification

- A `just soak-trace 42 <focal>` run shows phase transitions in `trace-<focal>.jsonl`.
- Unit test on `resolve_engage_prey`: at `dist = pounce_range`, `state.phase == Pouncing`; at `dist > stalk_start`, `state.phase == Approaching`; etc.

## Log

- 2026-05-13: opened as 263 follow-on after the aspirational `StepPhase` enum was identified as load-bearing for trace visibility.
