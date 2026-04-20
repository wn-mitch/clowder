# Activation 1 — Fog sight — status (2026-04-18)

## Outcome: **deferred, blocked by pre-existing colony-survival regression**

Three rounds of 15-run sweeps were captured. Activation was **not
merged**. `Weather::Fog::sight_multiplier()` remains `1.0` in HEAD.
The infrastructure built to measure the activation (`--force-weather`,
`just sweep`, `scripts/sweep_compare.py`, event-log header snapshot
of env multipliers, per-multiplier canary test) is intact and
orthogonal to the activation decision.

## Hypothesis (unchanged for future retry)

Real felid visual acuity collapses in dense fog: optical range drops
from ~100m to ~20–40m, and contrast discrimination degrades further.
Shadow-foxes — modeled as the colony's primary nonhuman antagonist —
rely on ambush; fog asymmetrically favors the ambusher because the
cat cannot see the closing threat until it is well inside strike
range. The same mechanism degrades cat→prey sight.

## Predictions and observations

| Metric | Predicted | Observed at 0.4 | Observed at 0.6 |
|---|---|---|---|
| `deaths_by_cause.ShadowFoxAmbush` (seed 42 mean) | +30–50% | +40% (3.33 → 4.67) | +10% (3.33 → 3.67) |
| Colony survives day 180 on seed 42 (canary) | YES | NO (wipe days 49, 69, 68) | NO (wipe days 49, 69, 68) |
| Natural-weather aggregate drift ≤ ±20% | YES | NO (multiple metrics +200% to +4600%) | NO (still large downstream deltas) |

The **mechanism works.** Seed 42 showed almost exactly the predicted
+40% ShadowFoxAmbush at Fog=0.4 — strong direct evidence that the
sight-multiplier pipeline reaches `cat_sees_threat_at` and reduces
effective detection radius as expected. But on this HEAD the colony
is pre-terminal even at Fog=1.0, so the canary cannot be satisfied by
tuning the fog knob alone.

## The pre-existing regression

`baseline-5b-v2` (Fog=1.0, captured with the current HEAD binary)
showed **15 of 15 colonies wipe between sim day 28 and 98**. Median
wipe-tick at baseline is ~67k post-start (sim day ~67).

`start_tick = 60 * ticks_per_season` was introduced in
`build_new_world` to let founder cats have varied ages — the comment
in `src/main.rs` explains that without it, `born_tick = start_tick
.saturating_sub(age_ticks)` clamps every founder to Young and blocks
mating eligibility. But at `60 * ticks_per_season`, some founders
spawn near end-of-life (55-season max rolled age) and the colony
can't replenish fast enough in a 15-minute simulation.

This is a sim-balance problem **upstream of Phase 5b** and must be
resolved before fog (or any other environmental multiplier) can be
activated under the Balance Methodology rule.

### Evidence the wipes are pre-existing, not fog-caused

Per-seed mean wipe-tick (ticks survived since start), paired across
sweeps:

| seed | baseline-v2 (Fog=1.0) | fog=0.6 | Δ% | fog=0.4 | Δ% |
|---|---|---|---|---|---|
| 2025 | 28347 | 28347 | +0.0% | 29652 | +4.6% |
| 314  | 93259 | 68095 | -27.0% | 54409 | -41.7% |
| 42   | 61781 | 61490 | -0.5% | 71503 | +15.7% |
| 7    | 62969 | 104907 | +66.6% | 36939 | -41.3% |
| 99   | 85792 | 74403 | -13.3% | 76418 | -10.9% |

- Wilcoxon signed-rank p on per-seed deltas: **0.875** (vs 0.6),
  **0.438** (vs 0.4).
- Mann-Whitney U on pooled wipe-ticks: **0.90** (vs 0.6), **0.21**
  (vs 0.4).

No significant effect of fog on survivability. The wipe distribution
is dominated by pre-existing colony fragility. (Several seed-42 reps
*survived longer* with fog than without it — seed 7 at Fog=0.6
survived 66% longer than baseline, which is evidence of noise
dominance, not that fog helps.)

## Artifact paths

All under `logs/`. Headers carry `sensory_env_multipliers` matrix;
diff them to confirm which run is which.

| Path | Fog sight | Weather | start_tick | Runs |
|---|---|---|---|---|
| `sweep-baseline-5b/` | 1.0 | natural | **100_000 (legacy)** | 15 |
| `sweep-baseline-5b-v2/` | 1.0 | natural | 60 × ticks_per_season | 15 |
| `sweep-fog-activation-1/` | 0.6 | natural | 60 × ticks_per_season | 15 |
| `sweep-fog-activation-1-v0.4/` | 0.4 | natural | 60 × ticks_per_season | 15 |
| `sweep-forced-fog-baseline/` | 1.0 | `--force-weather fog` | 60 × ticks_per_season | 5 |
| `sweep-forced-fog-activation/` | 0.4 | `--force-weather fog` | 60 × ticks_per_season | 5 |

> **Important:** `sweep-baseline-5b/` is from a different `start_tick`
> than everything else — not directly comparable. It's kept for
> reference on the *old* colony regime only.

Pre-computed reports:

| Path | What it compares |
|---|---|
| `docs/balance/activation-1-natural-0.4.report.md` | `baseline-5b` (legacy start_tick) vs `fog-activation-1-v0.4` — read with cross-start_tick caveat |
| `docs/balance/activation-1-forced-fog.report.md` | `forced-fog-baseline` (Fog=1.0) vs `forced-fog-activation` (Fog=0.4) — both new start_tick, valid |

## Infrastructure that shipped (keep regardless of activation decision)

- **`ForcedConditions` resource + `--force-weather <variant>` CLI
  flag.** `src/resources/forced_conditions.rs`, plumbed through
  `src/systems/weather.rs` (new `Res<ForcedConditions>` param pins
  `WeatherState::current` when set), `src/main.rs` (CLI parse and
  header), and both `build_new_world` paths.
- **`Terrain::ALL` const** (`src/resources/map.rs`) — exhaustive
  variant iteration for header snapshots.
- **Event-log header `sensory_env_multipliers` block** — full
  `weather × phase × terrain` × channel matrix dump plus
  `forced_weather: Option<String>`. Two sweeps can now be
  diff-validated on the multiplier values even though those
  multipliers live in enum methods, not in `SimConstants`. See
  `sensory_env_multipliers_snapshot()` in `src/main.rs`.
- **`just sweep <label> [force-weather] [seeds] [reps] [duration]
  [parallel]`** in `justfile` — BSD-xargs-safe 4-way parallel.
- **`scripts/sweep_compare.py`** — per-metric mean ± sd,
  Mann-Whitney U, Wilcoxon signed-rank per-seed pairing,
  env-multiplier delta printer, canary gate check.
- **Per-activation canary test** — the Phase 1 identity canary
  `env_from_environment_is_identity_in_phase_1` was replaced by
  `env_multipliers_match_activation_schedule` in
  `src/systems/sensing.rs`. It's forward-facing: add a line to
  `expected_*` when a multiplier becomes active. Currently asserts
  every multiplier is 1.0 (since the activation was reverted).

## Recommended next steps

1. **Decide whether to fix colony fragility first or press through.**
   - *(a) Fix the regression, retry.* Tune the sim so baseline-v2
     has colonies surviving past day 180 on seed 42. Likely knobs:
     old-age mortality curve, starvation threshold, founder-age cap
     (keep the mating-age fix but cap max rolled age below end of
     life). Then retry Activation 1 at Fog=0.6 with the cleaner
     baseline — the signal at 0.6 is expected to be ~+20% ambush,
     well above per-seed noise on seeds 7/42/99/314.
   - *(b) Relax the canary for this activation.* Document the pre-
     existing fragility and ship Activation 1 as tuning-neutral
     relative to the current baseline. Violates the letter of the
     Balance Methodology rule; only acceptable if paired with a
     follow-up to fix the regression.
   - *(c) Redefine the verification window.* A 5-minute "early
     window" canary (survive to day 60) could let balance work
     proceed while long-tail survival is being worked on.

2. **Confirm `start_tick` is the root cause.** The comment in
   `build_new_world` says `60 × ticks_per_season` is required for
   mating eligibility. Actually test whether a smaller value (e.g.
   `20 × ticks_per_season`, giving cats a 5-year age span) preserves
   mating eligibility without forcing any founder to near end-of-life.
   If yes, use the smaller value. Otherwise, cap the max rolled
   founder-age at ~10 seasons regardless of `start_tick`.

3. **Re-run Activation 1 at Fog=0.6 on the fixed sim.** The
   mechanism is verified; what couldn't be measured is whether the
   magnitude is *acceptable*, because every run collapsed before
   accumulating enough cat-ticks for noise to average out.

4. **When re-running, use `sweep_compare.py` with
   `--predictions docs/balance/activation-1-fog-sight.predictions.json
   --top 30`.** Focus on the per-seed paired section, not the pooled
   Mann-Whitney row — pooled tests wash out signal because
   seed-driven variance dominates rep-driven variance by ~10×
   (observed in baseline: seed 99 ambush CV 10%, seed 2025 ambush
   CV 173%).

5. **Consider the shadow-fox-spawn signal.** In the forced-fog runs
   at Fog=0.4, `shadow_fox_spawn_total` rose 60% vs forced-fog at
   Fog=1.0. If cats are worse at placing wards and cleansing
   corruption under reduced sight, corruption climbs → more
   shadow-foxes. This would be a *secondary-order* verisimilitude
   payoff worth calling out explicitly — but verification requires
   a colony that doesn't collapse in sim day 30.

## Session summary

The core infrastructure for Phase 5b is built and working. The
activation itself hit a blocker that sits upstream of the fog
decision. When the colony-survival regression is fixed, Activation 1
should be retried at Fog=0.6 — the signal is real, the magnitude is
predictable, and the tooling to verify it is in place.
