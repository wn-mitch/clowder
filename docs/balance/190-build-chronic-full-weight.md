# 190 — build_chronic_full_weight balance investigation

**Verdict:** Findings-only. Leave `build_chronic_full_weight` at `0.5`
(plausibility default). Root cause is upstream of BuildDse — see
[ticket 382](../open-work/tickets/382-influence-map-based-colony-district-placement-retire-find-building-placement-spiral-plan-expansion-zones.md).

## Hypothesis (iter-1 + iter-2)

Across 16 historical seed-42 archives the chronic-full feedback loop
fires reliably — `DepositRejected` accumulates to 5K+ per soak and the
`ColonyStoresChronicallyFull` marker passes L2 eligibility on the
majority of Build-DSE checks. But only 3 structures are built per
15-min soak and the welfare shelter axis sits at 0.20, indicating
the colony cannot keep up with deposit pressure under the
plausibility-default weight (0.5).

BuildDse's composition slot for chronic-full is 0.15; at weight 0.5
the axis contributes a maximum of `0.5 × 0.15 = 0.075` to the 0-1
weighted sum — modest relative to DILIGENCE (`0.4 × ≤1.0`) and competing
DSEs scoring on hunger / fatigue / fear pressure. Iter-1 lifted the
weight to 0.7 (predicted +20-200% on `structures_built`). Iter-2
escalated to 1.0 (max input value, contributing `1.0 × 0.15 = 0.15` —
the structural ceiling).

## Prediction

| Field | Value |
|---|---|
| Metric | `colony_score.structures_built` |
| Direction | increase |
| Rough magnitude band | ±20–200% |

## Observation

Sweeps: 1 seed (42) × 1 rep × 900s.

| Iter | weight | structures_built | colony_score.aggregate | shelter | nourishment | survival gates |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.5 | 5 | 2783.9 | 0.000 | 0.674 | pass |
| iter-1 | 0.7 | 5 | 2801.8 | 0.091 | 0.698 | pass |
| iter-2 | 1.0 | 5 | 2800.6 | 0.091 | 0.698 | pass |

| Field | iter-1 vs baseline | iter-2 vs baseline |
| --- | --- | --- |
| Observed direction | unchanged | unchanged |
| Observed Δ | 0.0% | 0.0% |
| p-value (Welch's t) | 1.0 | 1.0 |
| Cohen's d | 0.0 | 0.0 |

## Concordance

**Verdict: wrong-direction (both iterations).**

- Direction match: ✗ (predicted increase, observed unchanged)
- Magnitude in band: |Δ|=0.0% vs predicted ±20–200%

Survival hard-gates pass at all weights. Continuity canaries hold
(`courtship` ~4400, `grooming` ~1700, `mentoring` ~440, `play` 9,
`mythic-texture` 0, `burial` 0). No regressions.

## Structural reframe — why the weight doesn't move the metric

Layer-walk diagnosis from focal-cat trace (`trace-Simba.jsonl`):

- **L1 marker writer:** `ColonyStoresChronicallyFull` fires on 10,490
  of 19,044 Build-DSE eligibility checks (~55%). Working.
- **L2 BuildDse scoring:** When eligible, Build scores ~0.48 (sample
  tick 1,201,601). Weight lifts shift this by `±0.075` max. Working.
- **L3 selection:** Build is top-scoring in **0 of 9,522 trace ticks**
  for Simba (0.00%). pick_up dominates at 0.98 max. Even at weight=1.0
  Build's ceiling (~0.55) loses to pick_up by half a point.
- **Upstream filter:** Build only appears in Simba's trace 46 / 9,522
  times (0.5%). Concentrated in two 88-tick windows immediately
  following coordinator directives. After tick 1,203,506, Build never
  enters Simba's evaluation again — no ConstructionSite within range.
- **Root cause:** `find_building_placement` (`src/systems/coordination.rs:1267-1292`)
  spiral-searches a 16-tile Manhattan radius from colony center. After
  founder buildings + early-wave constructions saturate the disk,
  every later directive returns `None` and sits in the queue forever.

Baseline soak narrative confirms: 6 "decides the colony needs..."
narrations issued, but only 3 "marks out the site for..." narrations
(the latter fires when placement succeeds). Stuck directives at ticks
1,210,880 and 1,211,840 never spawn sites; the colony goes 50,500
ticks with no further building activity.

The substrate is doing exactly what it was designed to do — the chronic
loop fires, BuildDse scores when eligible, cats select what they can.
But the coordinator can never spawn the construction sites for cats
to engage with.

## Decision

- **Ship `build_chronic_full_weight = 0.5` (no change).** Tuning this
  weight cannot move `structures_built` until the placement bottleneck
  is fixed.
- **Open ticket 382** — Influence-map based colony-district placement.
  The substrate-correct fix per "richer perception, better strategy"
  pillar.
- **Re-run a focused hypothesize on `build_chronic_full_weight`
  after 382 lands** — only then will the weight's contribution to
  Build-win-rate be observable.

## Sibling artifacts from this investigation

- **Ticket 382** — Influence-map colony-district placement (the actual
  fix).
- **Ticket 373** — Den/Workshop food retrieval substrate (surfaced by
  190's UI work on FoodStores breakdown).
- **Ticket 374** — Shelter as housing-security belief (surfaced by
  parallel diagnosis of welfare.shelter = 0.20).
- **Documented but unopened** — `ColonyPriorityLift` retirement
  (pre-substrate player-driven flat lift on Build; pending user
  direction on whether to open).
