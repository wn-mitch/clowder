# Sweep comparison: `sweep-forced-fog-baseline` → `sweep-forced-fog-activation`

- base runs: 5
- post runs: 5

## ⚠ Header drift
- at least one run built from a dirty tree (headers may mislead)

## Sensory env-multiplier changes
- `weather.Fog.sight: 1.0 → 0.4000000059604645`

## Canaries (hard gates)
- ✗ `deaths_by_cause.Starvation` — max=8 (threshold == 0)
- ✓ `deaths_by_cause.ShadowFoxAmbush` — max=5 (threshold ≤ 5)

## Top movers (sorted by |Δ|)

| Metric | Base mean ± sd | Post mean ± sd | Δ%  | MWU p | Status | Notes |
|---|---|---|---|---|---|---|
| `deaths_by_cause.FoxConfrontation` | 0.00 ± 0.00 | 0.20 ± 0.45 | — | 0.424 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.PrepareRemedy: missing herb for remedy` | 0.00 ± 0.00 | 0.20 ± 0.45 | — | 0.424 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.EngageThreat: morale_break` | 34.00 ± 59.57 | 170.60 ± 239.00 | +401.8% | 0.293 | NO-PRED | Δ=+401.8% (no prediction to check) |
| `plan_failures_by_reason.SearchPrey: no scent found` | 0.40 ± 0.89 | 1.80 ± 4.02 | +350.0% | 1.000 | NO-PRED | Δ=+350.0% (no prediction to check) |
| `interrupts_by_reason.urgency CriticalSafety (level 2) preempted level 3 plan` | 24.20 ± 32.99 | 106.60 ± 211.31 | +340.5% | 0.916 | NO-PRED | Δ=+340.5% (no prediction to check) |
| `plan_failures_by_reason.TendCrops: no target for Tend` | 1.20 ± 2.68 | 4.40 ± 9.29 | +266.7% | 0.607 | NO-PRED | Δ=+266.7% (no prediction to check) |
| `interrupts_by_reason.urgency CriticalSafety (level 2) preempted level 4 plan` | 170.20 ± 361.07 | 455.00 ± 806.86 | +167.3% | 0.753 | NO-PRED | Δ=+167.3% (no prediction to check) |
| `interrupts_by_reason.urgency CriticalSafety (level 2) preempted level 5 plan` | 134.20 ± 217.41 | 271.40 ± 544.79 | +102.2% | 1.000 | NO-PRED | Δ=+102.2% (no prediction to check) |
| `deaths_by_cause.Injury` | 0.20 ± 0.45 | 0.00 ± 0.00 | -100.0% | 0.424 | NO-PRED | Δ=-100.0% (no prediction to check) |
| `plan_failures_by_reason.EngagePrey: stuck while chasing` | 0.40 ± 0.55 | 0.80 ± 0.84 | +100.0% | 0.488 | NO-PRED | Δ=+100.0% (no prediction to check) |
| `plan_failures_by_reason.HarvestCarcass: no carcass nearby` | 37.80 ± 83.97 | 0.20 ± 0.45 | -99.5% | 0.519 | NO-PRED | Δ=-99.5% (no prediction to check) |
| `plan_failures_by_reason.EngagePrey: no prey target for engage` | 3.60 ± 2.97 | 6.80 ± 8.53 | +88.9% | 0.915 | NO-PRED | Δ=+88.9% (no prediction to check) |
| `plan_failures_by_reason.TravelTo(SocialTarget): no reachable zone target` | 243.00 ± 538.90 | 56.20 ± 116.87 | -76.9% | 0.830 | NO-PRED | Δ=-76.9% (no prediction to check) |
| `shadow_fox_spawn_total` | 6.00 ± 2.00 | 9.60 ± 8.68 | +60.0% | 1.000 | NO-PRED | Δ=+60.0% (no prediction to check) |
| `plan_failures_by_reason.SetWard: no thornbriar for ward` | 1196.60 ± 1580.92 | 1876.80 ± 2300.01 | +56.8% | 0.346 | NO-PRED | Δ=+56.8% (no prediction to check) |

## Per-seed paired deltas (predicted metrics only)

### `deaths_by_cause.ShadowFoxAmbush`
| seed | base mean | post mean | Δ%  |
|---|---|---|---|
| 2025 | 3.00 | 3.00 | +0.0% |
| 314 | 6.00 | 4.00 | -33.3% |
| 42 | 4.00 | 4.00 | +0.0% |
| 7 | 4.00 | 5.00 | +25.0% |
| 99 | 5.00 | 1.00 | -80.0% |
- Wilcoxon signed-rank p (base vs post, across seeds): 0.500

### `plan_failures_by_reason.EngagePrey: lost prey during approach`
| seed | base mean | post mean | Δ%  |
|---|---|---|---|
| 2025 | 1710.00 | 1710.00 | +0.0% |
| 314 | 649.00 | 1353.00 | +108.5% |
| 42 | 352.00 | 349.00 | -0.9% |
| 7 | 1707.00 | 2221.00 | +30.1% |
| 99 | 346.00 | 312.00 | -9.8% |
- Wilcoxon signed-rank p (base vs post, across seeds): 0.625

## Summary
- canaries: FAILED
- concordance: 0 PASS, 0 FAIL (within top 15)
