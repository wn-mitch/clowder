---
id: 234
title: Damage-recency perception scalar — couple AcuteHealthAdrenalineFlee to felt danger
status: ready
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception, welfare-fidelity]
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`AcuteHealthAdrenalineFlee` (`src/ai/modifier.rs:1143-1278`) currently
triggers on `health_deficit` — a steady-state wound-level scalar. A cat
sitting at HP=0.5 for hours has the same modifier ramp as a cat that
just took an ambush hit one tick ago. This is the wrong shape for "the
danger a cat currently feels."

Real animals don't lurch on chronic wounds — they lurch on *fresh*
injury. An adrenaline response is a damage-event response, not a
damage-level response. The post-230 soak (`logs/tuned-42`) measured
**28,360 acute_health_adrenaline_flee preempts in 100k ticks** — that's
the modifier firing roughly every 4 ticks for any wounded cat,
regardless of whether anything is actually happening to them. It's
chronic, not acute.

The substrate-correct fix is to author a `damage_recency` perception
scalar — decay-driven, like the existing `threat_proximity_derivative`
(rising-only derivative of `safety_deficit`). On every damage event,
spike the scalar to 1.0; decay it linearly over ~200 ticks back to 0.0.
The `AcuteHealthAdrenalineFlee` modifier then triggers on
`damage_recency × health_deficit` (or replaces `health_deficit` outright
in its ramp formula). A wounded cat that took a hit 5 ticks ago: high
ramp, sharp lurch, cat flees. A wounded cat that's been at half-HP for
200 ticks doing chores: ramp at zero, modifier silent. **Tied to the
danger a cat currently feels.**

This is the original framing the user surfaced post-230: "AcuteHealth-
AdrenalineFlee should be tied sharply to the danger a cat currently
feels." Substrate enhancement, not modifier surgery — the modifier
formula gets one new input from a new scalar; the perception layer
takes responsibility for the "is this fresh damage or chronic state?"
question.

## Scope

- Author a new perception scalar `damage_recency` (or
  `recent_damage_pulse` — bikeshed):
  - Per-cat, range `[0.0, 1.0]`.
  - Spikes to 1.0 on every damage event (ambush, combat hit, corruption
    pulse, starvation tick — pick the set that captures "felt danger").
  - Decays linearly (or via short-half-life smoothstep) over ~200 ticks.
  - Authored by a system in `src/systems/interoception.rs` or sibling,
    reading damage events and writing the per-cat scalar.
  - Surfaces in `ScoringContext` and `ctx_scalars` per the existing
    perception-publishing pattern.
- Couple `AcuteHealthAdrenalineFlee` to the new scalar:
  - Replace or extend the `health_deficit` ramp input with
    `damage_recency × health_deficit` (or
    `max(damage_recency, baseline)` — the formula needs sensitivity
    sweep before locking).
  - Goal: chronic wounded cats see ramp ≈ 0; freshly-injured cats see
    ramp at saturation.
- Optionally couple sibling adrenaline-class modifiers
  (`AcuteHealthAdrenalineFight` if it has the same chronic-fire shape;
  audit before extending).
- New `SimConstants` knob: `damage_recency_decay_ticks` (default ~200).

## Out of scope

- **Damage event taxonomy** — what counts as "damage"? Ambush hit yes;
  starvation tick yes (felt danger from your own body); corruption
  pulse yes; injury severity weighting unclear. Land with a minimal set
  (combat + ambush + critical-need pulses) and extend per balance
  evidence.
- **`ThreatProximityAdrenalineFlee` coupling** — that modifier already
  reads a derivative scalar (`threat_proximity_derivative`); it's
  already shape-correct on the felt-danger axis. Out of scope unless
  audit reveals the same chronic-fire pattern.
- **Personality coupling** — anxious cats might decay slower (sustained
  fear); bold cats faster (shake it off). Composable as a follow-on
  once the baseline curve stabilizes.
- **Combat-side damage events** — Fight DSE outcomes feed this scalar
  too if the cat takes counter-damage. Includes; verify the integration
  point is the same as ambush.

## Current state

- `AcuteHealthAdrenalineFlee::apply` reads `health_deficit` directly
  via the `fetch` closure (see `src/ai/modifier.rs:1255-1260`).
  `preempts_in_flight` reads the same scalar
  (`src/ai/modifier.rs:1267-1277`). Ramp is a smoothstep over
  `[threshold, threshold + transition_width]`.
- The post-230 soak measured 28,360 preempts/100k ticks
  (down from 39,536 pre-230 — 230 closed half the gap structurally
  via the disposition-tier early-skip). The remaining 28,360 preempts
  are the chronic-fire shape this ticket addresses.
- 230's commit-aware preempt guard (the `Resting | Eating | Fleeing`
  early-skip in `try_preempt_with_modifier_lurch`) ONLY protects when
  the cat is in those dispositions. The chronic-fire on non-tier-1
  plans is exactly what 234's damage-recency coupling addresses.
- `threat_proximity_derivative` is the precedent: rising-only
  derivative of `safety_deficit`, computed from `PrevSafetyDeficit`
  component, surfaces in `ctx_scalars`. Same shape needed for
  `damage_recency` but with damage *events* as the trigger rather
  than continuous derivative.

## Approach

1. Add a `LastDamageEvent { tick: u64, severity: f32 }` component (or
   reuse an existing damage-tracking component if one exists).
   Authored by combat / ambush / damage-event handlers.
2. Add a `damage_recency` field on `ScoringContext` (mirrors
   `threat_proximity_derivative`). Computed at scoring time as
   `(decay_ticks - (now - last_damage_tick)) / decay_ticks`,
   clamped `[0, 1]`.
3. Surface in `ctx_scalars` with key `"damage_recency"`.
4. Update `AcuteHealthAdrenalineFlee::apply` and `preempts_in_flight`
   to read `damage_recency` (or composite of recency × deficit).
5. Sensitivity sweep: `just hypothesize` with prediction "preempt
   count drops > 5× without losing the wound-driven Flee response."
6. Calibrate `damage_recency_decay_ticks` (~200) and the modifier's
   ramp threshold against the dying-arc replay.

## Verification

- **Dying-arc replay:** preload a wounded cat (HP=0.5) with NO recent
  damage event — assert `AcuteHealthAdrenalineFlee::preempts_in_flight`
  returns `false`. Compare vs preload with damage_recency = 1.0
  (fresh hit) — assert returns `true`.
- **Soak preempt count:** seed-42 `just soak-trace 42 Calcifer`. Target:
  `interrupts_by_reason.modifier_preemption(acute_health_adrenaline_flee)`
  drops from current 28,360 (post-230) to < 4,000 (the original
  ticket-230 ≥10× target).
- **Chronic-wound non-fire:** scenario where a cat sits at HP=0.5 for
  500 ticks doing chores with no damage events — assert preempt count
  during that window is 0.
- **Acute-injury fire:** scenario where a healthy cat takes one ambush
  hit (HP drops from 1.0 → 0.5) — assert preempt fires once and decays
  off within `damage_recency_decay_ticks`.
- **No-regression on 230:** the Fleeing-disposition commitment cycle
  (PickFleeTarget → Flee → HoldUntilSafe) must still complete its hold
  window without modifier interference. Substrate-aware Flee is the
  *response* to felt danger; this ticket sharpens *when* the modifier
  fires, not what it does once committed.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **103** (done, ai-substrate, score 0.86 (cross-cluster)) — escape_viability perception scalar — first-class predicate for adrenaline-valen…
- ✓ landed ** 87** (done, ai-substrate, score 0.85 (cross-cluster)) — Interoceptive perception substrate
- · **304** (ready, belief-perception, score 0.85) — WitnessableEvent::Attack emit — gated on cat-vs-cat aggression substrate

<!-- linkages:end -->
## Log

- 2026-05-08: opened from post-230 soak. The 230 substrate-aware
  Fleeing chain dropped modifier-preempts from 39,536 → 28,360 (~28%
  reduction) but didn't close the gap; the user's hypothesis is that
  `AcuteHealthAdrenalineFlee` triggers on chronic wound state rather
  than felt-danger event, so it never quiets down. Substrate-correct
  fix is a damage-recency perception scalar (mirrors the
  `threat_proximity_derivative` shape) that captures "I just took
  damage" rather than "I am wounded." Modifier reads the new scalar;
  cats with chronic wounds doing chores stop seeing preempts; cats
  that just took an ambush see sharp lurches.
- 2026-05-19: accuracy audit pass — AcuteHealthAdrenalineFlee verified at src/ai/modifier.rs:1143-1278, threat_proximity_derivative pattern confirmed, no file path issues.
