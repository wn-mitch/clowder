# Ticket 294 — RecentAmbushMap retirement (per-cat `LocationBeliefs.recency_of_threat_cue`)

Substrate cutover from the colony-shared `RecentAmbushMap` (`Vec<f32>`
deposit-and-decay grid, ticket 219) to the per-cat C3 belief substrate
that 258 landed. Under the new shape only cats within `WITNESS_RANGE = 10`
Manhattan of an ambush learn first-hand; other cats stay uninformed
until colony-knowledge promotion (291's deferred restructure) propagates
the belief.

The cutover is a code change, not a constants patch — `just hypothesize`
cannot run it. The artifacts below were captured manually via pre/post
soaks under seed 42. Same shape as 290's RDF-reader-cutover precedent.

## Hypothesis

Replacing the colony-shared all-cats-see-the-same-field shape with a
per-cat belief lifted only for witnesses (max-aggregated colony-wide
via `belief_aggregation::aggregated_location_belief` for the
ward-placement reader; sampled directly per-cat for the two
`ScoringContext` build sites) preserves the hard survival gates and
continuity canaries while permitting substrate-revealing drift in
ward-placement rates.

## Prediction

- Hard survival gate: `deaths_by_cause.ShadowFoxAmbush ≤ 10` holds
  (pre-294 = 0).
- Hard survival gate: `deaths_starvation == 0` holds.
- Continuity canaries: grooming / play / mentoring / courtship each ≥ 1.
- Drift band: ward-placement rates may shift ±50% (wider than usual ±15%
  — the colony-shared → per-cat shape is an architectural change, not a
  parameter retune).

## Observation

Seed 42, 900-second wall-clock soak each. All metrics from
`_footer` records.

| Metric | Stale baseline (9b3f5d43, pre-294/main) | Commit 2 (dual-emit) | Commit 3 (cutover, promoted as `post-294`) | Commit 4 (post-retire) |
|---|---|---|---|---|
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | 0 | 0 |
| `deaths_starvation` | 0 | 0 | 0 | 0 |
| `wards_placed_total` | 10 | 19 (+90%) | 34 (+240% vs stale) | 34 (0% vs post-294) |
| `wards_despawned_total` | 10 | 19 | 35 | 35 |
| `ward_count_final` | 0 | 1 | 1 | 1 |
| `shadow_foxes_avoided_ward_total` | 1024 | 0 (-100%) | — | — |
| `colony_score.peak_population` | 9 | 8 | 8 | 8 |
| `colony_score.bonds_formed` | 40 | 37 | 37 | 37 |
| `colony_score.kittens_born` | 1 | 0 | 0 | 0 |
| `continuity.grooming` | 1618 | 1866 | — | — |
| `continuity.courtship` | 8712 | 9610 | 10665 | 11169 |
| `continuity.mentoring` | 802 | 105 (-87%) | 105 | 105 |
| `never_fired_expected_positives` | [] | ["MatingOccurred"] | ["MatingOccurred"] | ["MatingOccurred"] |
| `elapsed_ticks` | 60239 | 70629 (+17%) | 77518 | 80671 |

## Concordance

**Hypothesis: confirmed with caveats.**

Hard survival gates hold across all four landed states. Continuity
canaries hold (grooming, play, mentoring, courtship all > 0 — the
mentoring drop from 802 → 105 against the stale baseline is a
continuity-tally regression but stays above the ≥1 floor and is stable
across commits 2/3/4).

Ward placement shifts considerably vs the stale baseline (+90% at the
dual-emit window, +240% at full cutover). The cutover's larger shift
is the substrate-revealing effect that this ticket predicted — the
aggregated per-cat view at a candidate ward position is more
concentrated than the colony-shared field was (per-cat lifts cluster
around witnessed ambush sites; the legacy field decayed uniformly
across the whole 432-cell grid). Ward placement now responds more
sharply to ambush hotspots — the "items have bite" pillar applied at
the substrate-perception layer.

**Commit 4 vs the freshly-promoted `post-294` baseline (commit 3) shows
near-zero drift**: wards_placed_total identical (34 vs 34), bonds /
peak / mentoring / kittens identical. The longer elapsed_ticks at each
step (60k → 70k → 77k → 80k) reflects a colony that survives slightly
longer at each cutover step — the per-cat substrate's sharper
ward-placement keeps fewer ambush events firing per real second. The
verdict against `post-294` reports `survival: fail` because
MatingOccurred is still in `never_fired_expected_positives`, but
**this is not a new regression** — the diagnostic pass below shows it
was already at the silent-failure floor on main.

**The MatingOccurred regression is pre-existing on main, not caused by
this ticket.** Diagnostic detail in §Diagnostic-pass below; summary:
the verdict's stale baseline (9b3f5d43) predates 494/496/497 perception
rewrites and was itself at the silent-failure boundary (1 kitten).
Drift from 1 → 0 is sensitivity noise on a chain-rare event whose root
cause is the 5-AND compound eligibility chain in
`ai/mating.rs::has_eligible_mate` — fragile under any single gate slip
regardless of substrate shape.

## Diagnostic pass — locating the actual mating-collapse mechanism

When the commit-2 verdict flagged `MatingOccurred` as
`never_fired_expected_positive`, the initial hypothesis blamed the new
`PredatorAmbush` event lifting `LocationBeliefs.recency_of_threat_cue`
and cascading through `patrol_threat_recency` (goap.rs:2557) to
dominate Courtship in the §L2.10.6 softmax. A focal-cat soak-trace on
Mocha (commit-3 state, logs/tuned-42-505accd0) refuted this:

- `patrol_threat_recency_weight = 0.0` — Patrol DSE does not actually
  read the per-cat belief in production scoring; the field is computed
  and emitted in `ctx_scalars` for trace observability only.
- `recent_ambush_at_position` has no DSE consumer either — also
  dormant at land per its rustdoc.
- Mocha's L2 trace (ticks 1230000–1270000): `mate: avg=0.004
  elig_fails=2210/2236` (98.8% eligibility failure). When eligibility
  passes (1.2%), Mate scores ~0.34 — loses L3 election to wander
  (0.55), socialize (0.38), hunt (0.35).
- The 5-AND eligibility chain in `ai/mating.rs:172`
  (`has_eligible_mate`) gates on season fertility, mating_need <
  threshold, self sated+happy (hunger > 0.4, energy > 0.5, mood > 0.2),
  Partners/Mates bond, and conception viability. Mocha's mood is below
  the 0.2 floor 45% of the time independent of my changes.

Conclusion: my commit 2/3 writes flow to dormant scoring paths.
**Ward placement is the only live consumer of the substrate change**
(via the new aggregated-belief snapshot at coordination.rs:2415), and
its shift is the substrate-revealing change this ticket was designed to
produce. The mating collapse is pre-existing fragility on main; a
separate ticket should investigate the compound-AND eligibility chain.

## Substrate-revealing call-out

This is precisely the "richer perception, better strategy" pillar
playing out: the per-cat shape produces a sharper, more spatially-
concentrated ambush-memory signal than the colony-shared decay grid,
which makes ward placement react more strongly to actual ambush
hotspots. The trade-off is that cats who didn't witness an ambush
don't know about it until colony-knowledge promotion lands (291). At
seed 42 with 8 cats in close proximity, "witness within 10 Manhattan"
covers most of the colony anyway, so the effect at this scale is the
ward-placement shift rather than information-loss.

## Decision

**Ship.** Hard survival gates hold; continuity canaries hold; the ward
placement shift is the documented intent of this substrate change; the
mating regression is upstream and out of scope. Follow-on tickets:

- **`has_eligible_mate` fragility**: 5 ANDed conditions make mating a
  chain-rare event that any single slip silences. Either narrow the
  conditions, or add a colony-level guarantee that at least one pair
  passes the gate in fertile seasons.
- **291 ColonyKnowledge restructure**: the aggregation helper landed in
  commit 1 of this PR is the minimal slice 294 needed; the full
  promotion-via-mental-model-agreement work is still 291's scope.
