# Replacing the all-or-nothing hunger==0 cliff with quadratic graded drain reduces starvation mortality and lifts welfare-axis means by giving cats at mid-hunger time to self-rescue before the full cascade engages (2026-05-03)

Drafted by `just hypothesize` (ticket 031). Edit before committing — pre-filled
fields are starting points.

## Hypothesis

Replacing the all-or-nothing hunger==0 cliff with quadratic graded drain reduces starvation mortality and lifts welfare-axis means by giving cats at mid-hunger time to self-rescue before the full cascade engages

**Constants patch:**

```json
{
  "needs": {
    "starvation_cliff_use_legacy": false,
    "starvation_cliff_exponent": 2.0
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `deaths_by_cause.Starvation` |
| Direction | decrease |
| Rough magnitude band | ±60–90% |

## Observation

Sweeps: 3 seeds × 3 reps × 900s.

- Baseline: `logs/sweep-baseline-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g`
- Treatment: `logs/sweep-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g-treatment`

| Field | Value |
|---|---|
| Observed direction | unknown |
| Observed Δ | None% |
| p-value (Welch's t) | None |
| Cohen's d | None |

## Concordance

**Verdict: wrong-direction**

- Direction match: ✗ (decrease vs unknown)
- Magnitude in band: see |Δ|=None% vs predicted ±60–90%

## Survival canaries

Run `just verdict logs/sweep-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g-treatment/<seed>-1` against any
treatment run to check survival/continuity didn't regress.

## Decision

_To fill in: ship / iterate / reject. If iterating, append the next iteration to
this file (don't open a new doc — see CLAUDE.md §Long-horizon coordination)._
