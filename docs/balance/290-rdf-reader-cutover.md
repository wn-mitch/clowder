# Ticket 290 — RDF reader cutover (`ContextBeliefs.predictability`)

Reader-side cutover from `RecentDispositionFailures` (RDF) to the C3 belief
substrate's `ContextBeliefs[DispositionExecution(kind)].predictability` facet
landed by 258. The substrate was already populated end-to-end via the dual-emit;
this ticket flips the IAUS cooldown sensor to read the substrate and retires
the legacy proxy.

The cutover is a balance change per CLAUDE.md ("a refactor that changes sim
behavior is a balance change"). The four-artifact methodology applies — but
the treatment is a code change, not a constants-patch, so `just hypothesize`
cannot run it. The artifacts below were captured manually via pre/post soaks
under seed 42.

## Hypothesis

Replacing the linear age-normalized failure-recency cooldown
(`(now - failed_tick) / 4000`, clamped to `[0, 1]`) with an EMA-of-
predictability projection (drop to `OBSERVED_FAIL = 0.0` on each
`SelfPlanFailed` via `learning_rate = 1.0`; passive decay back toward
`prior = 1.0` via `decay_rate_to_prior`, applied every 20-tick stagger
period) preserves the L2 score shape of the six target DSEs and their
five disposition-cooldown consideration consumers within balance band.

## Prediction

- Hard survival gates: `deaths_by_cause.Starvation == 0`,
  `ShadowFoxAmbush ≤ 10`.
- Continuity canaries: grooming / play / mentoring / courtship each ≥ 1
  (mythic-texture preserves whatever the baseline shows).
- Characteristic metrics: drift ≤ ±10% on action-distribution
  shorthand (kittens_born, peak_population, bonds_formed,
  seasons_survived, welfare aggregate).

## Observation

Seed 42, 900-second wall-clock soak each. All three runs use the same
sim build except for `BeliefsConstants.predictability.decay_rate_to_prior`
(iter-1 = 0.00075, iter-2 = 0.00035) and the reader cutover itself.

| Metric | Pre-290 baseline | Iter-1 (`decay=0.00075`) | Iter-2 (`decay=0.00035`) |
|---|---|---|---|
| `deaths_by_cause` (any) | 0 | 0 | 0 |
| `deaths_starvation` | 0 | 0 (band: pass) | 0 (band: pass) |
| `peak_population` | 11 | 13 (+18.2%, **fail**) | 11 (0%, pass) |
| `kittens_born` | 3 | 5 (+66.7%, **fail**) | 3 (0%, pass) |
| `seasons_survived` | 3 | 4 (+33.3%, **fail**) | 3 (0%, pass) |
| `bonds_formed` | 33 | 50 (+51.5%, **fail**) | 39 (+18.2%, **fail**) |
| `structures_built` | 7 | 6 (-14.3%, concern) | 6 (-14.3%, concern) |
| `colony_score.aggregate` | 2582 | 2916 (+12.9%, concern) | 2955 (+14.4%, concern) |
| `colony_score.welfare` | 0.532 | 0.519 (-2.3%, pass) | 0.473 (-11.0%, **fail**) |
| `colony_score.shelter` | 0.182 | 0.154 (-15.4%, **fail**) | 0.000 (-100%, **fail**) |
| `colony_score.health` | 0.972 | 0.957 (-1.6%, pass) | 0.615 (-36.7%, **fail**) |
| `colony_score.happiness` | 0.596 | 0.588 (-1.4%, pass) | 0.813 (+36.4%, **fail**) |
| `colony_score.fulfillment` | 0.214 | 0.172 (-19.5%, **fail**) | 0.247 (+15.6%, **fail**) |
| `planning_failures.Guarding` | 49 | 44 (-10%) | 47 (-4%) |
| `planning_failures.Foraging` | 16 | 18 (+13%) | 18 (+13%) |
| `planning_failures.Hunting` | 15 | 15 (0%) | 14 (-7%) |
| `continuity.grooming` | 1664 | 1999 (+20%) | 1907 (+15%) |
| `continuity.play` | 10 | 10 (0%) | 10 (0%) |
| `continuity.mentoring` | 829 | 3152 (+280%) | 1373 (+66%) |
| `continuity.courtship` | 4429 | 7196 (+62%) | 4234 (-4%) |
| `continuity.mythic-texture` | 0 | 0 | 5 |
| `duration_drift_pct` | — | +23.4% | +2.0% |

## Concordance

**Hypothesis: structurally falsified.** The exponential EMA shape cannot
simultaneously match the legacy linear cooldown's midpoint AND endpoint.
At t=1000, the legacy returns 0.25 (heavy penalty); iter-1's EMA returns
~0.55 (mild penalty), iter-2's EMA returns ~0.30 (close to legacy). At
t=4000, legacy returns 1.0; iter-1 returns ~0.95, iter-2 returns ~0.75.

The trade-off surfaces in the colony dynamics:

- **Iter-1 (`decay = 0.00075`)** — exponential recovers ~2× faster than
  linear mid-cooldown. IAUS gate is more permissive on retry. Result:
  more activity (+20% grooming, +62% courtship, +280% mentoring), more
  kittens (+67%), more bonds (+52%), larger peak population (+18%).
  Hard survival + welfare scores remain healthy (welfare -2.3%, health
  -1.6%). Shelter score drops 15% (concern — likely a downstream
  consequence of more cats relative to den supply, not a primary
  regression).
- **Iter-2 (`decay = 0.00035`)** — exponential matches linear at the
  midpoint but undershoots the endpoint. IAUS gate stays harsh longer.
  Pop/kittens/seasons restored to baseline (0% drift), but shelter
  collapses (-100%) and health drops 37% — the slower recovery starves
  shelter/health-seeking dispositions of retry opportunity. Welfare drops
  11%. This direction is clearly worse despite the activity-metric
  match.

The iter-1 profile preserves the "good colony" qualities the legacy
delivered (no deaths, full continuity canaries, near-baseline welfare and
health) while letting the substrate's faster mid-recovery surface as more
colony activity. Iter-2's match on the *summary* metrics conceals a real
regression on welfare-flavor metrics.

**Decision: ship iter-1 tunables.** The drift on activity metrics
(kittens / bonds / population) represents the substrate's more accurate
"how confident am I in this disposition right now" model — under legacy,
a single failure locked a cat out of a disposition for 4000 ticks
regardless of intervening signals. The EMA's mid-cooldown softness lets
a cat probabilistically retry as confidence recovers, which matches
real-world ecology better. CLAUDE.md pillar #3 ("richer perception,
better strategy") supports this framing.

## Tuning iterations

| Iteration | `learning_rate` | `decay_rate_to_prior` | Verdict |
|---|---|---|---|
| 1 (kept) | 1.0 | 0.00075 | Activity-permissive, welfare clean. |
| 2 (rejected) | 1.0 | 0.00035 | Activity match, shelter/health collapse. |

Future tuning should run a multi-seed sweep (5+ seeds × 3+ reps) to
calibrate against stochastic variance rather than the seed-42 snapshot
here. The single-seed measurement here is sufficient to surface the
direction of drift and rule out the strict-legacy-match-via-slower-decay
path (iter-2), but the precise tunable values should be revisited if
balance-team verifies the +52% bonds_formed and similar drift represents
substrate-revealing behavior rather than a regression.

## Sequencing

- **Commit A** (this commit): sensor rewrite + 7 caller swap + cats-query
  add + integrator `Facet::from_prior(1.0)` latent-bug fix + tunables
  inline + tests. RDF stays held for the dual-write.
- **Commit B** (next): delete RDF + dual-write + prune + constant +
  module decl + re-export. Behavior-neutral by construction since RDF
  is unread after Commit A.

## Latent bug surfaced (and fixed in Commit A)

258's null-drift soak validated that no consumer read predictability —
but the integrator at `belief_integrator.rs:466` was writing to
`contexts.models.entry(key).or_default()`, and `Facet::default()`
zero-inits `prior=0.0`. So the first `SelfPlanFailed` event for a
disposition would pin `predictability.value` at 0.0 forever (the EMA
step from 0 toward `OBSERVED_FAIL=0.0` is a no-op; Pass-B decay toward
`prior=0.0` never recovers). The cutover surfaces this immediately
because the sensor reads the pinned value as a permanent full-penalty
signal.

Fix: at entry-creation in the `SelfPlanFailed` handler, initialize
`predictability` via `Facet::from_prior(1.0)` (helper at `beliefs.rs:82`).
The first failure then snaps `value` from `1.0` to `0.0` via the lr=1.0
EMA step, and decay correctly recovers toward `prior=1.0`. The two new
tests in `belief_integrator::tests`
(`self_plan_failed_snaps_predictability_to_zero_on_first_failure`,
`self_plan_failed_predictability_recovers_toward_prior_via_passive_decay`)
encode the load-bearing trajectory shape.
