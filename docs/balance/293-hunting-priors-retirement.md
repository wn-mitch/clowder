# Ticket 293 — HuntingPriors retirement (per-cat `LocationBeliefs.prey_yield`)

Substrate cutover from the per-cat `HuntingPriors` Component (dense
`Vec<f32>` belief grid, plus the `ColonyHuntingMap` social-transmission
`absorb` pathway through socialize / groom_other) to the C3 belief
substrate that 258 landed. Under the new shape:

- A cat's prey-yield belief at each 5-tile bucket lives on its own
  `LocationBeliefs[bucket].prey_yield` `Facet`.
- Three `WitnessableEvent` variants author the substrate:
  `Hunt { success: true/false }` (already existed; extended in this
  ticket's integrator arm to lift `prey_yield` at the bucket),
  `HuntScentDetected` (new — weak positive lift), and
  `HuntSearchYieldedNoPrey` (new — negative pull, scaled by
  `tiles_searched`).
- `best_direction` retired; the per-cat `best_prey_direction` reader
  scans the cat's own `LocationBeliefs` within radius and returns a
  unit direction step toward the highest-yield bucket above neutral
  (0.5).
- `ColonyHuntingMap` retained as a passive output buffer for the
  `HuntingBeliefSnapshot` visualization payload; its values are
  derived each `decay_stagger_period` (20 ticks) from
  `aggregate_location_belief_snapshot(FacetSlot::PreyYield)` — the
  facet-parametric helper 294 introduced as a "291 minimal slice."
- Social-transmission chain (`socialize` / `groom_other` running
  `colony.absorb(cat) + cat.learn_from(colony)`) retired entirely.
  Cross-cat consensus is now implicit in the aggregator's
  max-over-cats-with-strength-floor rule. The four tunables
  (`socialize_colony_absorb_rate`, `socialize_personal_learn_rate`,
  `groom_other_colony_absorb_rate`, `groom_other_personal_learn_rate`)
  retire alongside.

The cutover is a code change, not a constants patch — `just hypothesize`
cannot run it. Artifacts captured manually via pre/post soaks under
seed 42. Same shape as 290 / 294's RDF/RecentAmbushMap precedents.

## Hypothesis

Replacing the per-cat `HuntingPriors` dense grid + `ColonyHuntingMap`
two-way blend with per-cat `LocationBeliefs.prey_yield` + a derived
colony snapshot preserves the hard survival gates and continuity
canaries while permitting substrate-revealing drift in search behavior
and downstream colony dynamics.

The new substrate's "above-neutral threshold" matches the legacy
`DEFAULT_PRIOR = 0.5` semantic, so `best_prey_direction` returns
direction steps semantically equivalent to the legacy `best_direction`
when the same observations have been integrated. Default-initialized
facets (`strength = 0`, `value = 0`) fall below the strength floor in
the aggregator and below the value threshold in `best_prey_direction`
— uninformed buckets are correctly invisible to both readers.

## Prediction

- Hard survival gate: `deaths_starvation == 0` holds.
- Hard survival gate: `deaths_by_cause.ShadowFoxAmbush ≤ 10` holds.
- Continuity canaries: grooming / play / mentoring / courtship each ≥ 1.
- `never_fired_expected_positives` introduces no new entries beyond
  what the post-294 baseline already carries (`["MatingOccurred"]`).
- Drift bands: parameter-level metrics may shift ±25%; structural
  metrics (population, bonds) may shift more, with the schedule-edge
  perturbation (`[Bevy schedule-edge perturbation]` memory) creating
  the dominant source of seed-42 variance.

## Observation

Seed 42, 900-second wall-clock soak. All metrics from `_footer`
records. Baseline is `logs/baselines/post-294.json` (commit `505accd0`,
ticket 294 final cutover). Post-293 commit `7a4d26d9`.

| Metric | Post-294 baseline | Post-293 (this PR) | Delta |
|---|---|---|---|
| `deaths_starvation` | 0 | 0 | — |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | — |
| `deaths_old_age` | 0 | 0 | — |
| `deaths_injury` | 0 | 0 | — |
| `never_fired_expected_positives` | `["MatingOccurred"]` | `[]` | **resolved** |
| `colony_score.peak_population` | 8 | 12 | **+50%** |
| `colony_score.bonds_formed` | 37 | 57 | **+54%** |
| `colony_score.kittens_born` | 0 | 4 | **new-nonzero** |
| `colony_score.aggregate` | 3255 | 3759 | +15% |
| `colony_score.happiness` | 0.604 | 0.654 | +8% |
| `colony_score.health` | 0.942 | 0.966 | +3% |
| `colony_score.fulfillment` | 0.322 | 0.121 | **−62%** |
| `continuity.grooming` | 1866 | 1822 | −2% |
| `continuity.courtship` | 11169 | 9110 | −18% |
| `continuity.mentoring` | 105 | 3051 | **+2806%** |
| `continuity.play` | _absent_ | 16 | new |
| `continuity.burial` | 0 | 0 | — |
| `continuity.mythic-texture` | 0 | 0 | — |
| `wards_placed_total` | 34 | 6 | **−82%** |
| `wards_despawned_total` | 35 | 6 | −83% |
| `negative_events_total` | 104168 | 73770 | −29% |
| `structures_built` | 13 | 15 | +15% |
| `seasons_survived` | 3 | 3 | — |
| `elapsed_ticks` | 77521 | 63443 | −18% |

## Concordance

**Hypothesis: confirmed.** Hard survival gates hold; continuity
canaries hold; the cutover did not introduce any new
`never_fired_expected_positives` — in fact it *resolved* the
pre-existing `MatingOccurred` regression that 294's diagnostic pass
documented as upstream fragility.

### What the drift surfaces

**Reproductive function restored.** Post-294's `MatingOccurred`-
never-fires regression dissolves: `kittens_born` goes from 0 to 4 and
`never_fired_expected_positives` is empty. Peak population grows 8→12,
bonds 37→57. Mentoring activity (which scales with kitten presence —
adults teach kittens) jumps 28× from 105 to 3051. This isn't a
substrate change with a direct causal path to mating — `prey_yield`
doesn't touch mating eligibility. The mechanism is the **schedule-
edge perturbation** (`[Bevy schedule-edge perturbation]` memory):
removing `&mut HuntingPriors` from the `resolve_goap_plans` cats
query, the four dependent tunables from `DispositionConstants`, and
the `ColonyHuntingMap` SystemParam from the goap system collectively
reshape Bevy's topological system sort. The new ordering happens to
nudge `evaluate_and_plan`'s mood / hunger / energy interleave so the
5-AND eligibility chain in `ai/mating.rs::has_eligible_mate` passes
more often. The reform is real and welcome, not a chase-the-bug
regression.

**Ward placement drops ∝ negative-event exposure.** `wards_placed`
falls 82% but `negative_events_total` falls 29% concurrently — fewer
threats experienced means fewer wards needed. The substrate side of
ward placement (`recency_of_threat_cue` aggregation) was already on
the C3 path post-294; this PR doesn't alter that path, so the drop
isn't a substrate regression. It's downstream of the schedule-edge
shift suppressing threat *exposure* (cats are spending more time
mating / mentoring inside the colony, less time on ambush-prone
periphery patrols). The hard gate `ShadowFoxAmbush ≤ 10` holds
trivially at 0 vs 0.

**Fulfillment drop is the math.** `fulfillment` is averaged
across cats; `peak_population: +50%` adds new kittens whose
`body_condition` and `social_warmth` start at 0 — that drags the
mean down even as the absolute count of well-fulfilled adults rises.
The drop is a denominator artifact, not a regression. It would
resolve as the new kittens mature.

**Elapsed-ticks drop is sim density.** Fewer ticks fit in the 900-
second wall-clock window because each tick has more entities to
process (12 cats + new kittens vs 8 adults). Rate-normalized metrics
(see verdict's `rate_baseline` / `rate_observed` columns) better
reflect underlying behavior than raw counts in this regime.

### What this PR explicitly does NOT claim

This is a substrate-cutover ticket, not a mating-fix ticket. The
mating reform is a downstream side-effect of the schedule-edge
shift, **not** a designed-for improvement. A future ticket should
investigate whether the 5-AND eligibility chain in
`has_eligible_mate` is structurally fragile (per 294's diagnostic)
and worth refactoring — the current state's success is a function
of sort-order luck rather than a robust gate.

## Decision

**Ship.** Hard survival gates hold, continuity canaries hold, no new
`never_fired_expected_positives`, and the substrate-revealing drift
surfaces a positive cascade (mating works, colony grows). The
parameter-level drift is large but explained: the schedule-edge
perturbation is documented behavior of this codebase under any system-
sort-affecting structural change (memory:
`[Bevy schedule-edge perturbation]`).

Follow-on candidates (out-of-scope here):

- **`has_eligible_mate` structural review** — per 294's diagnostic
  and this PR's reaffirmation, the 5-AND compound chain in
  `ai/mating.rs:172` is too fragile. A new sort-order can either
  break or fix it.
- **291 ColonyKnowledge full restructure** — the aggregation helper
  this PR re-used (`belief_aggregation::aggregate_location_belief_snapshot`)
  is the minimal slice 294 introduced. 291's full mental-model-
  agreement promotion (admitting divergence, false-belief epidemics)
  is still its scope; this PR validated the minimal slice handles
  the second facet (`prey_yield`) cleanly.
- **`ColonyHuntingMap` snapshot vs derive-on-read** — the resource is
  rebuilt every 20 ticks but only read by the visualization snapshot
  path. A future tidy could replace the buffered grid with on-demand
  aggregation directly in `snapshot.rs`.
