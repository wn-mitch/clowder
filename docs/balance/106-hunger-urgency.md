# HungerUrgency modifier — Phase 2 verdict: trigger condition rarely met; ship inert (2026-05-02)

Authored after the ticket-106 Phase 2 focal-trace soak. No Phase 3
hypothesize sweep was run — see the Decision section for why.

## Hypothesis

Adding a `HungerUrgency` pressure-modifier lift on Eat / Hunt / Forage
above the 0.6 urgency threshold redirects hungry cats to food-acquisition
before the `Starvation` interrupt fires, reducing
`interrupts_by_reason.Starvation` in the same regime.

**Constants patch (proposed; never promoted from defaults):**

```json
{
  "scoring": {
    "hunger_urgency_threshold": 0.6,
    "hunger_urgency_eat_lift": 0.40,
    "hunger_urgency_hunt_lift": 0.20,
    "hunger_urgency_forage_lift": 0.20
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `interrupts_by_reason.Starvation` |
| Direction | decrease |
| Rough magnitude band | ±40–90% |

## Observation

**Phase 3 hypothesize sweep was skipped.** The single-seed canonical
baseline (`logs/tuned-42`, commit 0783194) carries
`interrupts_by_reason.Starvation == 0`. A `direction: decrease` prediction
against a noise floor of zero is unmeasurable.

Phase 2 substituted a focal-trace stress test instead:
`logs/tuned-trace-42-106-phase-2/`, 900s seed-42 release headless run with
focal cat Simba, the proposed lifts active, AND doubled hunger decay
(`needs.hunger_decay = 0.2`/day vs default 0.1/day). The doubled decay
forces cats into a slow-starvation regime to verify the modifier's
trigger and L2 lift behavior under load.

CatSnapshot hunger trajectory survey across all 8 cats:

| Cat | n snapshots | min hunger | p10 hunger | in-window (≤0.4) | legacy threshold (≤0.15) |
|---|---:|---:|---:|---:|---:|
| Birch | 839 | 0.438 | 0.509 | 0 | 0 |
| Calcifer | 122 | 0.474 | 0.514 | 0 | 0 |
| Ivy | 120 | 0.489 | 0.521 | 0 | 0 |
| Lark | 1065 | 0.413 | 0.514 | 0 | 0 |
| Mallow | 271 | 0.442 | 0.502 | 0 | 0 |
| Mocha | 1743 | 0.439 | 0.508 | 0 | 0 |
| Nettle | 1756 | 0.383 | 0.489 | **3 (0.2%)** | 0 |
| Simba | 1756 | 0.466 | 0.508 | 0 | 0 |

Footer carries `interrupts_by_reason.Starvation == 0` even under doubled
decay. Mechanical wiring of the modifier is verified: override JSON
applied to events.jsonl header (echoed `hunger_urgency_eat_lift: 0.4`
etc.); Simba's L2 trace shows `hunger_urgency` correctly silent at
below-threshold ticks while peer modifiers (`independence_solo`,
`stockpile_satiation`) fire normally.

## Concordance

**Verdict: not-applicable.** Trigger condition rarely met in the canonical
regime (0.2% of one cat's ticks even under stress). The substrate's lift
magnitude IS sufficient to flip the L2 contest if triggered (Simba's
average hunt = 0.61, forage = 0.64 — full-ramp lift pushes to 0.81 / 0.84,
above wander = 0.74), but the trigger is structurally rare.

## Why the regime doesn't trigger

The `Starvation` interrupt branch was designed for a regime where cats
sit in Crafting / Guarding while starving. That regime doesn't materialize
in the post-091 colony for two structural reasons:

1. **70% of action-time is Hunting / Foraging** (per the colony-wide
   action distribution — those dispositions ARE the food solution).
2. **The disposition-replan interrupt arm and the GOAP urgency arm both
   exempt Hunting / Foraging / Resting** (the wrapper at the retired
   `disposition.rs:312-325` and the live arm at `goap.rs:619`). Cats only
   leave the exempt set when their hunger is comfortable enough that
   neither path needs to fire.

The legacy `Starvation` interrupt was therefore vestigial in the post-091
regime. The substrate replacement (`HungerUrgency`) is similarly inert
under canonical conditions but provides redundant safety in stressed
regimes (food scarcity, prey collapse, dietary disruption) — pure
substrate, no override.

## Decision

- **Ship the modifier inert** (defaults 0.0/0.0/0.0). Phase 1 wiring
  landed at c83de3cd alongside 107 + 110.
- **Skip Phase 3 hypothesize sweep.** The metric is at the noise floor
  in both baseline AND treatment.
- **Phase 4 retired the `Starvation` interrupt arm** as zero-risk
  structural cleanup — `disposition.rs:312-325` (wrapper + arm) and the
  `InterruptReason::Starvation` enum variant deleted at 03f9f541. The
  900s seed-42 verification soak post-retirement confirms 0% drift on
  every footer metric vs the pre-retirement baseline at
  `logs/tuned-42-pre-106-107-phase-4/`.
- **Live food-routing** remains the GOAP urgency arm at
  `goap.rs:615-626` — out of scope for this ticket; would need its own
  Phase-3 finding showing it's redundant under the modifier.

## Worked-example takeaway for distress-modifiers.md (ticket 113)

This ticket is the first worked example of the **"trigger rarely met →
substrate is redundant safety, not active driver"** outcome. Distinct
from 047's "lift wins L2 not L3" plan-completion-momentum gap (107 is
the example for that on the energy axis). The doctrine table in
`docs/systems/distress-modifiers.md` should carry both as branches of
the substrate-over-override decision tree.
