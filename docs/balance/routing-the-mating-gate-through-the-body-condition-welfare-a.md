# Routing the mating gate through the body_condition welfare axis (slow-moving across hunger oscillations) reduces courtship variance and stabilizes reproduction cadence across the photoperiod — replicating the real-cat 'eat-rest cycle' tolerance the legacy gate fails on (2026-05-05)

Drafted by `just hypothesize` (ticket 031). Edit before committing — pre-filled
fields are starting points.

## Hypothesis

Routing the mating gate through the body_condition welfare axis (slow-moving across hunger oscillations) reduces courtship variance and stabilizes reproduction cadence across the photoperiod — replicating the real-cat 'eat-rest cycle' tolerance the legacy gate fails on

**Constants patch:**

```json
{
  "fulfillment": {
    "body_condition_decay_per_unit_hunger_deficit": 0.0001,
    "body_condition_recovery_per_unit_satiation": 5e-05,
    "body_condition_pivot": 0.5,
    "use_body_condition_for_breeding_gate": true
  },
  "scoring": {
    "breeding_hunger_floor": 0.6
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `continuity_tallies.courtship` |
| Direction | increase |
| Rough magnitude band | ±25–300% |

## Observation

Sweeps: 3 seeds × 3 reps × 900s.

- Baseline: `logs/sweep-baseline-routing-the-mating-gate-through-the-body-condition-welfare-a`
- Treatment: `logs/sweep-routing-the-mating-gate-through-the-body-condition-welfare-a-treatment`

| Field | Value |
|---|---|
| Observed direction | increase |
| Observed Δ | 43.2% |
| p-value (Welch's t) | 0.3628 |
| Cohen's d | 0.45 |

## Concordance

**Verdict: concordant**

- Direction match: ✓ (increase vs increase)
- Magnitude in band: see |Δ|=43.2% vs predicted ±25–300%

## Survival canaries

Run `just verdict logs/sweep-routing-the-mating-gate-through-the-body-condition-welfare-a-treatment/<seed>-1` against any
treatment run to check survival/continuity didn't regress.

## Decision

_To fill in: ship / iterate / reject. If iterating, append the next iteration to
this file (don't open a new doc — see CLAUDE.md §Long-horizon coordination)._
