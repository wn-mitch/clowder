# Sweep comparison: `sweep-baseline-5b` → `sweep-fog-activation-1`

- base runs: 15
- post runs: 15

## ⚠ Header drift
- at least one run built from a dirty tree (headers may mislead)

## Sensory env-multiplier changes
- `weather.Fog.sight: 1.0 → 0.4000000059604645`

## Canaries (hard gates)
- ✗ `deaths_by_cause.Starvation` — max=7 (threshold == 0)
- ✗ `deaths_by_cause.ShadowFoxAmbush` — max=8 (threshold ≤ 5)

## Top movers (sorted by |Δ|)

| Metric | Base mean ± sd | Post mean ± sd | Δ%  | MWU p | Status | Notes |
|---|---|---|---|---|---|---|
| `deaths_by_cause.FoxConfrontation` | 0.00 ± 0.00 | 0.13 ± 0.35 | — | 0.164 | NO-PRED | new metric (base mean 0) |
| `deaths_by_cause.OldAge` | 0.00 ± 0.00 | 0.07 ± 0.26 | — | 0.351 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.ApplyRemedy: patient no longer alive` | 0.00 ± 0.00 | 0.07 ± 0.26 | — | 0.351 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.CleanseCorruption: misfire: fizzle` | 0.00 ± 0.00 | 0.33 ± 0.62 | — | 0.038 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.TravelTo(ConstructionSite): no reachable zone target` | 0.00 ± 0.00 | 0.33 ± 0.49 | — | 0.017 | NO-PRED | new metric (base mean 0) |
| `plan_failures_by_reason.HarvestCarcass: no carcass nearby` | 1.27 ± 3.39 | 60.27 ± 136.81 | +4657.9% | 0.131 | NO-PRED | Δ=+4657.9% (no prediction to check) |
| `plan_failures_by_reason.SetWard: no thornbriar for ward` | 353.93 ± 564.58 | 2311.73 ± 3326.44 | +553.2% | 0.051 | NO-PRED | Δ=+553.2% (no prediction to check) |
| `interrupts_by_reason.urgency ThreatNearby (level 2) preempted level 4 plan` | 3.67 ± 5.72 | 17.40 ± 29.70 | +374.5% | 0.011 | NO-PRED | Δ=+374.5% (no prediction to check) |
| `plan_failures_by_reason.TravelTo(HerbPatch): no path and stuck` | 65.33 ± 122.78 | 246.73 ± 538.50 | +277.7% | 0.068 | NO-PRED | Δ=+277.7% (no prediction to check) |
| `interrupts_by_reason.urgency ThreatNearby (level 2) preempted level 3 plan` | 1.07 ± 1.44 | 2.80 ± 3.08 | +162.5% | 0.174 | NO-PRED | Δ=+162.5% (no prediction to check) |
| `plan_failures_by_reason.EngageThreat: morale_break` | 69.73 ± 120.32 | 153.40 ± 203.00 | +120.0% | 0.516 | NO-PRED | Δ=+120.0% (no prediction to check) |
| `deaths_by_cause.Starvation` | 1.20 ± 1.74 | 2.47 ± 2.72 | +105.6% | 0.297 | NO-PRED | Δ=+105.6% (no prediction to check) |
| `deaths_by_cause.Injury` | 0.20 ± 0.56 | 0.00 ± 0.00 | -100.0% | 0.164 | NO-PRED | Δ=-100.0% (no prediction to check) |
| `plan_failures_by_reason.Construct: no target for Construct` | 0.40 ± 0.51 | 0.00 ± 0.00 | -100.0% | 0.008 | NO-PRED | Δ=-100.0% (no prediction to check) |
| `plan_failures_by_reason.GatherHerb: no herb target` | 0.13 ± 0.52 | 0.00 ± 0.00 | -100.0% | 0.351 | NO-PRED | Δ=-100.0% (no prediction to check) |
| `plan_failures_by_reason.PrepareRemedy: missing herb for remedy` | 0.33 ± 0.62 | 0.00 ± 0.00 | -100.0% | 0.038 | NO-PRED | Δ=-100.0% (no prediction to check) |
| `plan_failures_by_reason.CleanseCorruption: global step timeout` | 8.67 ± 31.65 | 0.27 ± 0.80 | -96.9% | 0.342 | NO-PRED | Δ=-96.9% (no prediction to check) |
| `plan_failures_by_reason.TravelTo(SocialTarget): no reachable zone target` | 33.13 ± 67.29 | 3.40 ± 4.15 | -89.7% | 0.699 | NO-PRED | Δ=-89.7% (no prediction to check) |
| `plan_failures_by_reason.TendCrops: no target for Tend` | 5.20 ± 11.28 | 0.80 ± 1.86 | -84.6% | 0.955 | NO-PRED | Δ=-84.6% (no prediction to check) |
| `plan_failures_by_reason.EngagePrey: lost prey during approach` | 3675.13 ± 4990.39 | 830.47 ± 458.62 | -77.4% | 0.028 | FAIL | predicted up, got down (-77.4%) |

## Per-seed paired deltas (predicted metrics only)

### `deaths_by_cause.ShadowFoxAmbush`
| seed | base mean | post mean | Δ%  |
|---|---|---|---|
| 2025 | 0.33 | 3.33 | +900.0% |
| 314 | 7.00 | 7.00 | +0.0% |
| 42 | 3.33 | 4.67 | +40.0% |
| 7 | 5.00 | 5.67 | +13.3% |
| 99 | 5.67 | 3.00 | -47.1% |
- Wilcoxon signed-rank p (base vs post, across seeds): 0.625

### `plan_failures_by_reason.EngagePrey: lost prey during approach`
| seed | base mean | post mean | Δ%  |
|---|---|---|---|
| 2025 | 13181.00 | 929.67 | -92.9% |
| 314 | 1132.00 | 779.33 | -31.2% |
| 42 | 1433.67 | 784.67 | -45.3% |
| 7 | 2374.67 | 1502.67 | -36.7% |
| 99 | 254.33 | 156.00 | -38.7% |
- Wilcoxon signed-rank p (base vs post, across seeds): 0.062

## Summary
- canaries: FAILED
- concordance: 0 PASS, 1 FAIL (within top 20)
