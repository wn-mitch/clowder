# Lowering breeding_hunger_floor 0.6 → 0.4 widens the AND-gate eligibility window enough to recover courtship cadence after the post-substrate reproduction collapse (2026-05-03)

Drafted by `just hypothesize` (ticket 031). Edit before committing — pre-filled
fields are starting points.

## Hypothesis

Lowering breeding_hunger_floor 0.6 → 0.4 widens the AND-gate eligibility window enough to recover courtship cadence after the post-substrate reproduction collapse

**Constants patch:**

```json
{
  "scoring": {
    "breeding_hunger_floor": 0.4
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

- Baseline: `logs/sweep-baseline-lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e`
- Treatment: `logs/sweep-lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e-treatment`

| Field | Value |
|---|---|
| Observed direction | unchanged |
| Observed Δ | 5.2% |
| p-value (Welch's t) | 0.944 |
| Cohen's d | 0.03 |

## Concordance

**Verdict: wrong-direction**

- Direction match: ✗ (increase vs unchanged)
- Magnitude in band: see |Δ|=5.2% vs predicted ±25–300%

## Survival canaries

Run `just verdict logs/sweep-lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e-treatment/<seed>-1` against any
treatment run to check survival/continuity didn't regress.

## Decision

_To fill in: ship / iterate / reject. If iterating, append the next iteration to
this file (don't open a new doc — see CLAUDE.md §Long-horizon coordination)._
