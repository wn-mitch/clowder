---
id: 232
title: Body-state-coupled L3 softmax temperature for stake-aware decision sharpness
status: done
cluster: null
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-08
---

## Why

The L3 softmax temperature is currently a fixed constant (~0.15) regardless
of the cat's state. The post-230 dying-arc analysis of `logs/tuned-42`
(commit `ffb2b69b`, focal Calcifer) surfaced this as a structural decision-
quality defect at exactly the moments where decision quality matters most:

| Tick | Cat | HP | Threat | **Chose** | Top L2 (within 1% of winner) |
|---|---|---|---|---|---|
| 1251500 | Calcifer | 0.49 | fox 2 tiles away | **PickUp** (0.958) | Flee (0.948) |
| 1252100 | Cedar | 0.20 | safety=0.02 | **Wander** | PickUp 1.00 / Sleep 0.98 / Flee 0.96 |

At temperature 0.15, two DSEs scored within 0.01 of each other are
effectively a coin-flip. Calcifer's fatal decision was a 53/47 split
between PickUp and Flee — the cat lost the coin flip and walked into
fox-scent territory carrying carcass items, took another ambush 60 ticks
later, died.

A wounded cat with an active threat should NOT be making coin-flip
decisions between work and survival. A calm cat with all needs met SHOULD
be exploring (broad temperature → diverse pick distribution is part of
what makes the colony narratively rich). The current fixed-temperature
softmax treats every tick the same — same breadth of consideration whether
the cat is bleeding out or napping in the sun. **Decision *shape* should
be a substrate-driven signal, not a constant.**

This is substrate enhancement, not a gate or override: the temperature
itself becomes a function of body-state perception. Calm cat: high T
(broad exploration). Wounded / threatened / starving cat: low T (decisions
sharpen because the cost of a wrong pick is high). The cat's homeostasis
literally tunes the breadth of its own consideration. Composes with 231's
pickup-class body-state Considerations (231 makes Sleep score *higher* than
PickUp when wounded; 232 makes the gap MATTER by sharpening the softmax
draw).

## Scope

- A new function `softmax_temperature(ctx: &ScoringContext) -> f32` that
  reads body-state perception (`body_distress_composite`,
  `threat_proximity_derivative`, possibly `pain_level`) and produces a
  per-tick temperature.
- Wire the function into the L3 softmax site in `src/ai/scoring.rs`
  (replace the fixed `softmax_temperature` constant or per-cat lookup).
- Curve calibration:
  - Floor: T_min ≈ 0.05 — sharp enough that a 5% score margin
    deterministically picks the winner. Hit at high body distress AND
    high threat-proximity (the dying-arc state).
  - Ceiling: T_max ≈ 0.20–0.25 — slightly broader than today's 0.15 so
    healthy-cat decisions stay narratively diverse.
  - Composition: `T = T_max - (T_max - T_min) × max(body_distress,
    threat_proximity_derivative)` is a candidate first shape; tune via
    sensitivity sweep before locking.
- A `SimConstants::scoring::softmax_temperature_floor` /
  `softmax_temperature_ceiling` knob pair so the curve is tunable without
  touching code.

## Out of scope

- Per-cat personality coupling on temperature (anxious cats more focused,
  curious cats broader). Composable as a follow-on once the body-state
  baseline curve stabilizes.
- L2 evaluator-level temperature changes (this ticket only touches the L3
  disposition softmax).
- Fox / wildlife AI softmax — those have separate eval paths in
  `src/ai/fox_scoring.rs` etc. Open as siblings if the same shape gap
  applies there.

## Current state

- Fixed temperature `softmax_temperature: 0.15` lives in
  `SimConstants::scoring`. Read once per softmax draw in
  `src/ai/scoring.rs` (grep `softmax_temperature` for the call site).
- Body-state perception is fully published: `body_distress_composite`,
  `pain_level`, `threat_proximity_derivative`, `safety_deficit` are all
  fields on `ScoringContext` (see `src/ai/scoring.rs:209+`) with
  authoring systems already running. The substrate side of the gap is
  closed; consumers exist (Sleep, Flee read these scalars), but the
  softmax temperature is not yet a consumer.
- Ticket 231's body-state subscription on pickup-class DSEs (R3b) is
  the L2-side companion to this ticket. Both compose: 231 makes the
  L2 ranking honest under body state, 232 makes the L3 softmax draw
  decisive when the ranking matters.

## Approach

1. Add `softmax_temperature(ctx: &ScoringContext) -> f32` near the
   existing `ctx_scalars` builder in `src/ai/scoring.rs` (single-source-
   of-truth alongside the perception layer).
2. Replace the fixed-constant read at the L3 softmax site with the
   function call.
3. Add the floor / ceiling constants to `DispositionConstants` or
   `ScoringConstants` (whichever houses the existing
   `softmax_temperature` constant).
4. Smoke test: replay the dying-arc state from `logs/tuned-42` Calcifer
   tick 1251500 — assert post-fix temperature is at the floor, and the
   Flee vs PickUp 0.01 gap deterministically picks Flee.
5. Calm-cat parity: replay a healthy-state tick (Bramble at tick 1200100,
   all needs > 0.7, no threat) — assert temperature is at the ceiling,
   and routine decisions remain stochastic.

## Verification

- **Dying-arc replay (microexperiment):** preload a wounded cat (HP=0.49,
  body_distress_composite ≥ 0.5) next to a fox; assert L3 picks Flee
  deterministically when Flee scores within 5% of PickUp's score.
- **Calm-cat parity:** replay a healthy founder tick; assert L3 decisions
  stay stochastic (multiple winners across 100 reps under different RNG
  seeds).
- **Soak verdict drift:** the post-232 soak (combined with 230 + 231)
  should show modifier_preemption count drop further (currently 28,360
  with 230 alone; target < 4,000 per ticket 230's verification).
- **Sensitivity sweep:** `just hypothesize` with body-state-temperature
  hypothesis spec — predicted direction "wounded cats elect tier-1
  dispositions more reliably; healthy cats unchanged."

## Log

- 2026-05-08: opened from post-230 soak dying-arc analysis. Calcifer
  picked PickUp (0.958) over Flee (0.948) at HP=0.49, fox 2 tiles away
  — the 1% softmax margin under fixed T=0.15 is the canary. Cedar's
  parallel arc shows the same shape at a different tick. Companion
  follow-on alongside 231's body-state Considerations on pickup-class
  DSEs (R3b) — 231 makes the L2 ranking honest under body state, 232
  makes the L3 softmax draw decisive when the ranking matters.
- 2026-05-08: 2026-05-08: landed. Soak verdict 'concern' (survival-pass). Hard gates clean (deaths_starvation=0, ShadowFoxAmbush=0, never_fired=0). modifier_preemption(acute_health_adrenaline_flee) 15,604 → 6,736 (-57% vs pre-232 9b302638), the headline substrate effect. Aggregate colony score +118%, health +230%, peak_population +50%. Calibration finding: floor T=0.05 alone gives only ~55% probability on Calcifer's 1% L2 margin (vs ~52% at T=0.15) — full decisiveness on the dying arc requires 231's L2 widening + 232's L3 sharpening together. Burial canary fails as deaths=0; structures_built -25% and seasons_survived -28.6% flagged for follow-on attention. anxiety_interrupt_total → 0 confirmed retired metric, not a regression.
