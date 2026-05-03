---
id: 147
title: Per-axis distress-modifier value tuning (multi-seed hypothesize)
status: ready
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, distress-modifiers.md]
related-balance: [146-distress-substrate-inert.md, 106-hunger-urgency.md, 107-exhaustion-pressure.md, 110-thermal-distress.md]
landed-at: null
landed-on: null
---

## Why

Ticket 146 closed with the per-axis distress modifiers (106 / 107 / 110)
shipping at `lift = 0.0` defaults — placeholders, not tuned values.
Single-seed-42 manual sweeps in 146 §Phase 2 found a U-curve:
half-magnitudes are WORSE than either inert or full magnitudes, and
full magnitudes (0.20+0.20 Sleep) cause colony extinction via the
107+110 Sleep double-stack on cold tired nights.

The substrate decomposition is right per the substrate-over-override
discipline. The values are unknown. This ticket tunes them.

## Scope

For each per-axis lift constant, run `just hypothesize` against the
inert baseline with multi-seed variance:

- `scoring.hunger_urgency_eat_lift` — predicts food-routing changes
- `scoring.hunger_urgency_hunt_lift`, `_forage_lift` — secondary
- `scoring.exhaustion_pressure_sleep_lift` — predicts rest-routing
- `scoring.exhaustion_pressure_groom_lift` — settling-ritual secondary
- `scoring.thermal_distress_sleep_lift` — predicts shelter-seeking

The 146 saturating-composition cap (`scoring.max_additive_lift_per_dse`)
ships at 0.0 (disabled). When this ticket surfaces non-zero per-axis
lifts, set the cap to 0.60 in the same change to bound the multi-axis
Sleep double-stack at design-time, not behaviorally.

**Methodology** (per CLAUDE.md): single-knob hypothesis specs, 3 seeds
× 3 reps × 900s = 18 runs/sweep. Predict direction + magnitude band
per knob; concordance check via `just hypothesize`'s envelope. Open
follow-on for any per-axis behavior that doesn't concord.

## Out of scope

- Retiring 088. Closed under 111 + 146.
- Tuning 047 / 102 / 105 / 108. Separate tickets.
- Courtship-chain fragility. Tracked under follow-on ticket 148.

## Log

- 2026-05-02: Opened from 146 close-out. Inherits 146's saturating
  cap as design-time bound for multi-axis pile-up.
