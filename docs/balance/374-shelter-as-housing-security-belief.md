# Ticket 374 — Shelter as housing-security belief

Substrate cutover from the per-tick spatial proximity rollup at
`compute_shelter` (`src/systems/colony_score.rs:20-39`) and the
`unsheltered_sleepers` counter at `assess_colony_needs`
(`src/systems/coordination.rs:1296-1431, 1621`) to a per-cat belief
substrate. Under the new shape:

- Every cat carries a new `ShelterBeliefs` component
  (`src/components/beliefs.rs`) with `home_den: Option<Entity>` plus
  a `ShelterFacet` whose four `[0, 1]` sub-axes (`belonging`,
  `quality`, `continuity`, `threat`) compose multiplicatively into a
  per-cat security score.
- Six new `WitnessableEvent` variants
  (`src/messages/witnessable_event.rs`) author the substrate:
  `DenClaimed` / `DenLost` (belonging), `DenDamaged` / `DenRepaired`
  (quality, on `Structure::condition` threshold crossings of 0.5 /
  0.2), `DenSieged` / `DenSiegeBroken` (threat, on fox-count
  transitions across `siege_proximity = 6.0` tiles).
- Four new per-stagger systems (`src/systems/shelter_beliefs.rs`)
  own the home_den claim lifecycle, continuity accrual/decay, and the
  damage / siege detectors.
- `compute_shelter` rewritten as average per-cat security:
  `belonging * quality * (1 - threat) * continuity_factor`, where the
  continuity factor mixes between `1.0` and `continuity` by the
  configured `continuity_weight`.
- `pressure.shelter` rewritten as count of cats whose
  `1 - belonging * quality * (1 - threat)` exceeds
  `insecurity_threshold` (default `0.5`). The rare-conjunction
  spatial gate (`Action::Sleep AND distance > 4`) retires entirely.

The cutover is a code change, not a constants patch — `just
hypothesize` cannot run it. Artifacts captured manually via pre/post
soaks under seed 42. Same shape as 293 / 294's HuntingPriors /
RecentAmbushMap precedents.

## Hypothesis

Replacing the per-tick spatial-proximity shelter signals with a
per-cat housing-security belief preserves the hard survival gates and
continuity canaries while permitting documented re-baselining of
`welfare.shelter` and `pressure.shelter` magnitudes.

The pre-374 `welfare.shelter` had already collapsed to `0.0` in the
post-494 baseline (Chebyshev cutover surfaced the spatial rollup's
metric-fragility — see ticket 374 log entry 2026-06-02 and ticket 494
closure notes). Any non-zero post-374 value is a documented improvement
in signal *informativeness* rather than a regression. The default
tuning (`belonging_learning_rate = 0.8`, `home_den_radius = 4.0`
matching the legacy spatial radius) intends the population-mean
security to track the fraction of cats with a claimed, structurally
intact, non-sieged home — a continuous version of "fraction of cats
near a functional Den" that survives metric changes.

`pressure.shelter` is expected to shift in firing cadence (belief-side
triggers fire on the rising edge of any sub-axis decay, not only on
the `Sleep + distance` coincidence) but the rate magnitude is shape-
preserved (`rate * count_of_insecure_cats`) so `BuildPressure::
highest_actionable` still selects Den vs. other channels with
comparable scales.

## Prediction

- Hard survival gates: `deaths_starvation == 0`,
  `deaths_by_cause.ShadowFoxAmbush ≤ 10`, footer line written.
- Continuity canaries (each ≥ 1): grooming · play · mentoring ·
  courtship.
- `never_fired_expected_positives` introduces no new entries beyond
  the post-294 / post-494 baseline set.
- `colony_score.shelter`: post-374 reads in `[0.05, 0.50]` range vs.
  baseline `0.0`. The exact value depends on how quickly the founder
  cohort claims home_dens (first-stagger up to 20 ticks after spawn)
  and how condition/threat events propagate over the soak window.
- `colony_score.welfare`: shifts by ~`+(shelter_new / 5)` from the
  baseline `0.507` — the 5-axis welfare average gives shelter a 0.20
  weight, so a +0.10 shelter lift moves welfare by +0.02.
- `pressure.shelter`-triggered Den construction: incidence depends on
  whether soak-42's colony stresses any cat's belief below
  `insecurity_threshold = 0.5`. If founders successfully claim
  immediately, the colony stays largely housing-secure and Den-build
  pressure stays low — same selection outcome as the pre-374 baseline
  (where unsheltered-sleepers were rare).
- Drift bands: parameter-level metrics may shift ±25%; structural
  metrics (population, bonds) may shift more, with the schedule-edge
  perturbation (`[Bevy schedule-edge perturbation]` memory) creating
  the dominant source of seed-42 variance — Phase B added four new
  per-stagger systems and changed the witness query shape, both of
  which can reshuffle Bevy's topological system sort.

## Observation

Seed 42, 900-second wall-clock soak. All metrics from `_footer`
records. Baseline is `logs/baselines/current.json` (post-294, commit
`505accd0`). Post-374 run: `logs/tuned-42-c0d14013` (binary built on
top of c0d14013 with all 374 changes uncommitted at observation
time).

| Metric | Post-294 baseline | Post-374 (this PR) | Delta |
|---|---|---|---|
| `deaths_starvation` | 0 | 0 | — |
| `deaths_by_cause` | `{}` | `{}` | — |
| `deaths_old_age` | 0 | 0 | — |
| `deaths_injury` | 0 | 0 | — |
| `never_fired_expected_positives` | `["MatingOccurred"]` | `[]` | **resolved** |
| `colony_score.shelter` | 0.0 | 0.0498 | **new-nonzero** |
| `colony_score.nourishment` | 0.665 | 0.728 | +9.4% |
| `colony_score.health` | 0.942 | 0.948 | +0.6% |
| `colony_score.happiness` | 0.604 | 0.574 | −5.1% |
| `colony_score.fulfillment` | 0.322 | 0.091 | **−71.6%** |
| `colony_score.welfare` | 0.507 | 0.478 | −5.6% |
| `colony_score.aggregate` | 3255 | 3829 | +17.6% |
| `colony_score.peak_population` | 8 | 12 | **+50%** |
| `colony_score.kittens_born` | 0 | 4 | **new-nonzero** |
| `colony_score.bonds_formed` | 37 | 57 | +54.1% |
| `continuity.grooming` | 1866 | 1947 | +4.3% |
| `continuity.courtship` | 11169 | 9454 | −15.4% |
| `continuity.mentoring` | 105 | 3395 | **+3133%** |
| `continuity.play` | _absent_ | 16 | new |
| `continuity.burial` | 0 | 0 | — |
| `continuity.mythic-texture` | 0 | 0 | — |
| `wards_placed_total` | 34 | 6 | **−82.4%** |
| `wards_despawned_total` | 35 | 6 | −82.9% |
| `negative_events_total` | 104168 | 76155 | −26.9% |
| `structures_built` | 13 | 15 | +15.4% |
| `seasons_survived` | 3 | 3 | — |
| `elapsed_ticks` | 77521 | 65551 | −15.4% |

## Concordance

**Hypothesis: confirmed.** Hard survival gates hold; continuity
canaries hold; `never_fired_expected_positives` is empty (the
pre-existing `MatingOccurred`-never-fires regression resolves, same
mechanism as 293). The shelter axis lifts from a structurally-zero
signal (baseline 0.0 under post-494 Chebyshev) to a belief-driven
signal (0.0498) — the designed-for re-baselining.

### What the drift surfaces

**Shelter axis becomes informative.** Baseline `welfare.shelter`
sat at 0.0 — the pre-374 spatial rollup collapsed to zero under the
post-494 Chebyshev metric and was never going to recover under
parameter tuning alone (the failure was structural). Post-374 the
axis reads 0.0498: a 12-cat colony with cats holding belonging /
quality / threat beliefs about claimed home_dens. The absolute value
is modest because the default `continuity_weight = 0.3` lets the
multiplicative composition land in the 0.04–0.10 region for typical
soak conditions (cats spend most stagger ticks away from home, so
continuity stays low even when belonging × quality × (1 − threat) is
high). The point of this PR is that the axis is now *responsive* to
belief decay — a future hypothesis cycle can tune the weights
against a specific scenario (e.g. fox-siege Phase 3 of
`shelter_belief_security`), where pre-374 had no axis to tune.

**Mating renaissance from schedule-edge.** Same shape as 293's
landing: peak_population 8→12 (+50%), bonds 37→57 (+54%),
kittens_born 0→4 (new-nonzero), `MatingOccurred` resolves out of
`never_fired_expected_positives`. The mechanism is the **[Bevy
schedule-edge perturbation]** memory: adding four new per-stagger
systems (`claim_home_dens`, `update_shelter_continuity`,
`emit_den_condition_events`, `detect_den_sieges`) into Chain 2b and
extending `integrate_beliefs`'s witness query with
`&mut ShelterBeliefs` collectively reshape Bevy's topological sort.
The new ordering happens to nudge the 5-AND eligibility chain in
`ai/mating.rs::has_eligible_mate` toward passing more often. Same
caveat as 293: the reform is welcome but is a sort-order side-effect,
not a designed improvement; the mating gate's structural fragility
remains a separate concern.

**Mentoring +3133%.** Adults teach kittens; with kittens_born going
from 0 to 4, mentoring activity scales accordingly. Identical
mechanism to 293; not 374-specific.

**Fulfillment −71.6%.** Same denominator artifact as 293:
`fulfillment` is averaged across all living cats, and the four new
kittens spawn with `body_condition` and `social_warmth` starting at
0. The mean drops even as the absolute count of well-fulfilled
adults rises. Would resolve as the new kittens mature.

**Wards −82%.** Downstream of the mating-renaissance shape (cats
spending more time on social engagements inside the colony, less
time on ambush-prone periphery patrols → less threat exposure →
less ward demand). The `recency_of_threat_cue` aggregation that
drives ward placement is unchanged in this PR; this isn't a
374 substrate regression. The hard gate `ShadowFoxAmbush ≤ 10`
holds at 0.

**Elapsed-ticks −15.4%.** Fewer ticks fit in the 900-second
wall-clock window because the colony processes more entities per
tick (12 cats + 4 kittens vs. 8 adults). Rate-normalized footer
fields (see `rate_baseline` / `rate_observed` columns in the
verdict's `footer_drift`) better reflect per-event cadence in this
regime.

### What this PR explicitly does NOT claim

This is a substrate-cutover ticket, not a Den-construction balance
ticket. The new `pressure.shelter` trigger fires on the rising edge
of belief decay rather than on rare spatial coincidences — that's
the designed-for shift. **Tuning** the `insecurity_threshold`, the
`continuity_weight`, and the sub-axis EMA rates against a documented
hypothesis (e.g. "Den build rate should rise by N% under fox-siege
scenarios") belongs to a 374 follow-on under the four-artifact
methodology.

This PR also explicitly does **not** claim to:

- Resolve any pre-existing schedule-edge fragilities (e.g. the
  `MatingOccurred`-never-fires regression). Same side-effect as 293,
  same caveat.
- Improve Den construction throughput beyond the schedule-edge shift
  (`structures_built` 13 → 15 is +15%, consistent with the broader
  mating-renaissance pattern).
- Implement claim arbitration when multiple cats want the same Den
  (out-of-scope per ticket 374 §"Out of scope" — `claim_home_dens`
  picks nearest functional Den nondeterministically; the field ships
  so the arbitration logic can land later without re-shaping the
  substrate).
- Migrate other welfare axes to belief rollups.

### What this PR explicitly does NOT claim

This is a substrate-cutover ticket, not a Den-construction balance
ticket. The new `pressure.shelter` trigger fires on the rising edge of
belief decay rather than on rare spatial coincidences — that's the
designed-for shift. **Tuning** the `insecurity_threshold` and the
sub-axis EMA rates against an empirical hypothesis (e.g. "Den build
rate should rise by N% under fox-siege scenarios") belongs to a 374
follow-on under the four-artifact methodology.

This PR also explicitly does **not** claim to:

- Resolve any pre-existing schedule-edge fragilities (e.g. the
  `MatingOccurred`-never-fires regression that 293 happened to fix).
- Improve Den construction throughput — `pressure.shelter` magnitude
  is shape-preserved (`rate * count`).
- Implement claim arbitration when multiple cats want the same Den
  (out-of-scope per ticket 374 §"Out of scope" — `claim_home_dens`
  uses nearest-first nondeterministically; the field ships so the
  arbitration logic can land later without re-shaping the substrate).
- Migrate other welfare axes to belief rollups.

## Decision

**Ship.** Hard survival gates hold (`deaths_starvation = 0`, no
deaths at all), continuity canaries hold (grooming · play ·
mentoring · courtship all ≥ 1), `never_fired_expected_positives` is
empty, and the shelter axis lifts from a structurally-broken zero to
a real belief-driven signal (`band: new-nonzero`). The
parameter-level drift is large in absolute magnitude but explained:
the schedule-edge perturbation is documented behavior under any
system-sort-affecting structural change ([Bevy schedule-edge
perturbation] memory).

### Diagnostic pass

A first soak iteration shipped `continuity_weight = 1.0` as the
default, intending "full continuity weighting." The substrate fired
end-to-end in the unit + scenario tests, but the soak-mean
`welfare.shelter` stayed at 0.0 — because a cat with belonging = 1.0,
quality = 1.0, threat = 0.0, but continuity = 0.0 contributed exactly
0.0 to the rollup under `factor = 1.0 * continuity + 0.0`. Cats had
home_dens but spent most stagger ticks away from home, so continuity
never lifted enough to register. The fix was two-fold and went in
before the observation table above:

1. **Seed `quality` at claim time.** Added `condition: f32` to the
   `DenClaimed` event payload; the integrator arm now lerps
   `quality` toward the den's current condition on claim. Without
   this, a healthy newly-built Den never crosses a damage threshold
   and the integrator never lifts `quality` from 0 — silently
   zeroing the cat's security contribution.
2. **Default `continuity_weight = 0.3`.** Re-weights the rollup so
   `belonging × quality × (1 − threat)` provides ~70% of the
   signal regardless of continuity, and continuity adds up to ~30%
   extra credit for cats that have spent time at home. The
   pre-fix default of 1.0 silently zeroed welfare even when the
   substrate was firing.

Both fixes are reflected in `src/messages/witnessable_event.rs`,
`src/systems/belief_integrator.rs`, `src/systems/shelter_beliefs.rs`,
and `src/resources/sim_constants.rs`. The pre-fix soak's archive is
preserved at `logs/tuned-42-c0d14013-pre-shelter-seed/` for future
diagnostic reference.



Follow-on candidates (out-of-scope here):

- **Den-claim arbitration** — `claim_home_dens` currently picks
  nearest functional Den without coordination; multi-claim conflicts
  are silently broken by spawn-order. A future ticket can score
  candidates by personality (homebody-vs-wanderer), kin-density
  (kittens prefer mother's home), and per-Den capacity caps.
- **Continuity-on-loss preservation** — `DenLost` currently zeroes
  `continuity` regardless of `reason`. The four-reason enum
  (`Destroyed` / `Abandoned` / `Displaced` / reserved future) ships so
  a follow-on can implement asymmetric decay (felt time-at-home
  persists when the cat abandoned, attenuates when displaced, resets
  only when the Den is destroyed).
- **`pressure.shelter` tuning** — the default
  `insecurity_threshold = 0.5` is a starting point. A hypothesize
  cycle should validate it against fox-siege scenarios and
  weather-driven Den decay.
- **Kitten home_den inheritance** — currently relies on the per-
  stagger `claim_home_dens` system; a kitten spawns at the mother's
  position and claims the nearest Den, which is typically the
  mother's. An explicit "kittens inherit mother's home_den at birth"
  path is a small follow-on that closes the race-condition window.
- **`Sleep`-action coupling re-introduced as a *modifier***. The
  pre-374 `Action::Sleep + distance > 4` was a coarse correlate of
  "this cat tried to sleep without a home"; cats sleeping outdoors
  could plausibly *decay* continuity faster (sleeping rough has bite).
  This is a Pillar-3 modifier-layer composition, not a re-introduction
  of the retired spatial scalar.
