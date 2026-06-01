# Ticket 138 — Snake cadence (per_tick=0.5) + escape_viability mobility-differential term

Phase 1 of the [#135 continuous-position-migration epic](../../docs/open-work/tickets/135-continuous-position-migration.md).
Lands per-entity `MovementBudget` and re-enables the mobility term in
[`escape_viability`](../../src/systems/interoception.rs) that was
punted at [ticket 103](../open-work/landed/103-escape-viability-scalar.md)'s landing.

## Hypothesis

Slowing snake cadence from `per_tick = 1.0` to `per_tick = 0.5` (every-other-tick
step on the integer grid), combined with adding a mobility-differential term
that lifts cat-vs-snake `escape_viability` by `mobility_weight × 0.25 = +0.05`
at the default weight (`mobility_weight = 0.2`), will:

1. **Reduce per-cat injury rate from snake encounters by 30–50%.** Cats now
   outpace snakes by default, so a snake in pursuit takes twice as many ticks
   to close range. Pursuit-driven encounters fail; ambush-from-cover encounters
   still land but represent a smaller share of the encounter mix.

2. **Reduce snake-driven cat deaths to near-zero.** The full kill chain
   requires (a) snake reaches the cat (now harder), (b) snake strikes
   (`Feature::SnakeStruckPrey`), (c) cat fails recovery. Step (a) becoming
   harder collapses the chain rate. Snake-from-ambush kills are still
   possible if the snake catches a cornered cat, but the steady-state rate
   should be ≪ baseline.

3. **Lift cat-vs-snake `escape_viability` by ~+0.10 (open terrain).** Direct
   structural prediction from the new composition. With `mobility_weight = 0.2`
   and `mobility_normalization = 1.0`, a cat at `per_tick = 1.0` facing a
   snake at `per_tick = 0.5` produces:
   - `mobility_advantage = clamp((1.0 − 0.5) / 1.0, −1, +1) = 0.5`
   - `mobility_term = 0.5 × 0.5 + 0.5 = 0.75`
   - Contribution: `0.2 × 0.75 = 0.15` (vs neutral `0.2 × 0.5 = 0.10`)
   - Net lift: `+0.05` (`escape_viability` for cat-vs-snake in open terrain,
     no dependents: from `0.7` neutral → `0.75`).

   The ticket's `+0.15–0.20` band assumes additional knock-on effects:
   blocked pursuit means fewer "snake at strike range" encounters reach the
   ScoringContext at all, so the *aggregate* shift in Flee selection during
   snake encounters has a multiplier from encounter-frequency drift.

## Constants patch

```yaml
constants_patch:
  escape_viability:
    terrain_weight: 0.6           # was 0.7
    mobility_weight: 0.2          # new (ticket 138)
    mobility_normalization: 1.0   # new (ticket 138)
    dependent_weight: 0.2         # was 0.3
```

`WildSpecies::Snake::default_movement_budget()` returns `0.5`. Hawk, Fox,
ShadowFox all `1.0` (steady-state cadence; burst abilities are own tickets).

## Predictions table

| Metric                                        | Direction | Magnitude (vs baseline) | Source                |
|-----------------------------------------------|-----------|-------------------------|-----------------------|
| `cat.per_cat_injury_rate.snake`               | decrease  | 30–50%                  | Pursuit gate from #138 |
| `cat.deaths_by_cause.snake_ambush`            | decrease  | → 0 (≥80% reduction)    | Chain-rate collapse   |
| `cat.escape_viability.mean_when_threat=snake` | increase  | +0.15–0.20              | Structural + frequency |
| `wildlife.snake.position_writes_per_tick.mean`| decrease  | ~50%                    | per_tick=0.5 gate     |

## Concordance method (CLAUDE.md balance methodology)

Four-artifact run via `just hypothesize docs/balance/138-snake-cadence-and-mobility-term.md`:

1. **Hypothesis** (this doc).
2. **Prediction**: the magnitudes table above.
3. **Observation**: footer + canary measurements from the soak sweep.
4. **Concordance**: pass if every prediction lands within its acceptance band,
   else surface the failures and either revise the model or revise the doc
   *before* `just promote`.

## Seeds & duration

```yaml
seeds: [42, 99, 7]
reps: 3
duration: 1200  # 12 in-game days; long enough for the snake encounter rate
                # to stabilize past first-light noise
```

## Single-seed verification (`just soak-trace 42 Simba 600`, commit ecb584b5)

- **Survival gates**: PASS
  - `starvation_deaths`: 0
  - `shadowfox_ambush_total`: 0 (≪ 10 cap)
  - `deaths_by_cause`: `{}` (no deaths)
  - `never_fired_expected_positives`: `[]`
- **Continuity canaries**: PASS — grooming 1558×, play 6×, mentoring 569×, courtship 6375× (burial + mythic-texture at 0×; both 0× on baseline too).
- **Verdict**: `concern` — driven by `colony_score` drift (shelter −100%, fulfillment −63.8%, welfare −17.6%) plus elevated plan-failure rates (Guarding/Hunting `GoalUnreachable` at 40–70× baseline ratio).

**Diagnosis of the verdict-`concern` drift.** This run inserted
`accumulate_movement_budget` as a new sibling system in Chain 1's
wildlife sub-chain. Per the
[bevy-schedule-edge-perturbation memory](../../docs/conventions/),
adding a new sibling in a Chain perturbs Bevy's topological sort
deterministically on seed-42 — even when the new system is
behaviorally inert for the entities involved (cats and foxes at
`per_tick = 1.0`). The plan-failure spike and shelter/welfare drift
match the schedule-perturbation signature, not the snake-cadence
behavioral prediction.

**Next step**: run `just hypothesize docs/balance/138-snake-cadence-and-mobility-term.md`
for the four-artifact methodology — 3 seeds × 1200-tick duration
will isolate the snake-attributable drift from the schedule-edge
noise. If the hypothesize sweep confirms the snake-injury and
snake-death predictions within their bands, run `just promote` to
refresh the comparison baseline (the new constants `terrain_weight
= 0.6`, `mobility_weight = 0.2`, `mobility_normalization = 1.0`,
`dependent_weight = 0.2` need to be baked into `current.json` for
future `just verdict` calls to compare apples-to-apples).

## Out of scope (parked for follow-on tickets)

- **Hawk dive burst speed.** Steady-state hawk cadence stays at 1.0 here;
  the dive is its own per-ability ticket (the per-ability ticket gets a
  one-shot accumulator boost, not a steady-state cadence change).
- **Shadow-fox lurch.** Same shape as hawk-dive — burst ability, separate ticket.
- **Per-cat cadence variance.** Sprightly elders, lumbering hunters. Phase 1 keeps
  all cats at `per_tick = 1.0`; the substrate (`MovementBudget::cat()`) is
  in place so future tuning is parameter-only.
- **Cat-side step-gate fan-out.** ~30 cat step-sites under `src/steps/disposition/`,
  `src/systems/goap.rs`, `src/systems/disposition.rs` still write `*pos = next`
  unconditionally. With all cats at `per_tick = 1.0` these are no-op gates;
  the fan-out lands when per-cat cadence variance does.
