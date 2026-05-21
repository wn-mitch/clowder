# 367 Phase 1b preservation — first-light substrate activation (Iter 0)

Skeleton authored at substrate-landing time (367 Commit 7). The
Observation + Concordance sections fill in after the first-light
`just soak-trace 42 Simba` + `just verdict logs/tuned-42/` runs. Iter
1 (sister doc + sister spec) extends from substrate-activation
("does the layer fire?") to magnitude prediction ("does the colony's
winter food buffer materially lift?").

Cross-references:
- Plan: [`docs/open-work/tickets/367-phase-1-preservation-recipes-...`](../open-work/tickets/367-phase-1-preservation-recipes-dried-fish-smoked-meat-preserved-organ-drying-rack-and-smoking-rack-stations-016-phase-1b.md)
- Spec: [`367-phase-1-preservation.yaml`](./367-phase-1-preservation.yaml)
- Doctrine: [`docs/systems/crafting.md`](../systems/crafting.md) Phase 1 hypothesis (line 164)
- Substrate doc-comments: `src/components/building.rs::DryingLoad` / `SmokingLoad`, `src/systems/preservation.rs`, `src/resources/sim_constants.rs::CraftingConstants`

## Hypothesis

The 367 preservation substrate (Drying Rack + Smoking Rack + 6
`preserve.*` recipes + per-tick drying system + RawOrgan hunt drop +
organ mood bump) is end-to-end load-bearing on seed-42 within
`--duration 900`. Cats build at least one rack, hunt prey whose
carcass enters inventory, load the rack with raw food (and fuel for
smoking), the per-tick drying system advances under Clear weather
(or tend cycles advance smoking), and the recipe's output `Item`
entity spawns on the ground at the rack tile.

First-light gate: `feature_counts.FoodDried >= 1`. A single drying
completion validates the entire chain. Companion canary:
`feature_counts.MeatSmoked >= 1` for the smoke pipeline.
`OrganPreserved` is chain-rare (30% organ drop × herb availability ×
~2-day cure) and stays `expected=false` in the never-fired
enrollment.

**Constants patch:** none — defaults shipped with 367 / 4b / 5 / 6.

## Prediction

| Field | Value |
|---|---|
| Primary metric | `feature_counts.FoodDried` |
| Direction | increase |
| Rough magnitude band | `[1, ∞)` — substrate-activation gate, not a magnitude prediction |
| Companion gates | `feature_counts.MeatSmoked >= 1`; `deaths_by_cause.Starvation == 0` (existing hard gate); never-fired-canary clean |

## Observation

_To fill after first-light run. Replace this paragraph with the
output of `just verdict logs/tuned-42/` after the seed-42 soak. The
expected shape:_

- Baseline: _none — first-light run IS the baseline that Iter 1 (food-stockpile lift) compares against._
- Treatment: `logs/tuned-42/` (or named archive if running off-canonical path)
- `feature_counts.FoodDried`: ?
- `feature_counts.MeatSmoked`: ?
- `feature_counts.OrganPreserved`: ? (chain-rare; informational only)
- `feature_counts.FoodLoadedOnDryingRack`: ?
- `feature_counts.MeatLoadedOnSmokingRack`: ?
- `feature_counts.SmokingRackTended`: ?
- `deaths_by_cause.Starvation`: ?
- `deaths_by_cause.ShadowFoxAmbush`: ?
- Never-fired-canary status: ?

## Concordance

_To fill._

Outcomes by first-light shape:

- **All five positive Features (`FoodLoadedOnDryingRack`,
  `MeatLoadedOnSmokingRack`, `SmokingRackTended`, `FoodDried`,
  `MeatSmoked`) fire ≥ 1:** substrate is end-to-end load-bearing.
  Open Iter 1 ticket targeting `food_stockpile_season_3_median` lift
  vs pre-367 baseline.
- **Load Features fire but `FoodDried` / `MeatSmoked` don't:** the
  consumer chain (per-tick drying system, tend cycles) is
  silent-failing. Open a bugfix ticket; layer-walk the preservation
  system's eligibility / weather gate / progress arithmetic.
- **Load Features don't fire either:** the DSE-side eligibility or
  load resolver is silent. Layer-walk the DSE markers
  (`HasFunctionalDryingRack`, `HasRawFishInInventory`, etc.) and the
  marker-snapshot writers in `goap.rs::evaluate_and_plan`.
- **All Features fire but a survival gate trips:** the substrate
  works but its second-order effects (e.g., preservation labor
  pulled cats off hunting, leading to a hunger cascade) are
  net-negative. Open a balance ticket; first-light's role is now
  diagnostic, not validation.

## Survival canaries

Run `just verdict logs/tuned-42/` post-soak. Hard gates:

- `deaths_by_cause.Starvation == 0`
- `deaths_by_cause.ShadowFoxAmbush <= 10`
- Footer present
- `never_fired_expected_positives == 0` (catches load Feature silence
  per the 367 Commit 4 enrollment)

Continuity canaries (≥1 each): grooming, play, mentoring, courtship,
mythic-texture.

## Iter 1 scope (deferred)

Per the
[`feedback_dormant_substrate_activation_soak_first`](../../.../memory/feedback_dormant_substrate_activation_soak_first.md)
memory, first-light is a substrate-activation gate, not a magnitude
prediction. Iter 1 (sister spec + doc) lands once first-light shows
all five positive Features firing. Targets:

- `food_stockpile_season_3_median` ≥ ~2× pre-367 baseline (the
  crafting.md Phase 1 prediction)
- `deaths_by_cause.Starvation` continues at 0
- Mortality timing shifts from late-winter to non-seasonal causes
  (qualitative — read from `q deaths` output, not a single metric)
- Preservation labor doesn't materially suppress hunting / foraging
  (sweep-stats vs baseline; expect <10% drift)

Open Iter 1 as a follow-on ticket once first-light data exists.

## Log

- _Iter 0 — substrate landed at 367 Commits 1-7. Doc + spec authored
  at landing time as Iter-0 skeleton; first-light data appended in
  the next session by running `just soak-trace 42 Simba` + `just
  verdict logs/tuned-42/`._
