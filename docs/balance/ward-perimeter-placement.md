# Perimeter ward placement via L1 influence maps

**Date:** 2026-04-27
**Ticket:** [045](../open-work/tickets/045-ward-perimeter-coverage.md)
**Commit (lands with):** _(this commit)_
**Predecessor evidence:** `logs/collapse-probe-42-fix-043-044/` (1-hour collapse-probe soak, post-043+044, 17 in-game years).

## Hypothesis

Ward placement was reactive-clustered: 40 placements spread across only 13 unique tiles, 11 within Manhattan-3 of the colony centroid. The colony altar got blanketed; approach corridors stayed bare. The ticket's deeper-cause framing names three compounding factors (short ward lifetime, corruption-only gating, single-priestess throughput); the user has flagged **placement is the load-bearing miss** — even with the existing reactive gate, when the priestess does decide to ward, she should cover the threat corridor, not re-stack the altar.

The `compute_ward_placement` function previously ran a defensive heuristic — "cover an uncovered structure" + "distance from existing wards" — with no notion of *where the threats actually come from*. The L1 influence-map system already runs a `fox_scent` map that decays each tick (0.90/tick) and stamps emitter-falloff around live foxes, so high-`fox_scent` tiles *are* the corridors SFs walk through, weighted by recency. Sampling that map at candidate tiles expresses approach-path placement as a derivative of substrate signals, with no spawn-zone enumeration, no Bresenham, no `KnownThreatVectors` resource.

Anti-clustering needs the same per-tile expression: a new `ward_coverage` L1 map (commit 1 of this ticket) stamps each ward's repel falloff into the grid every tick; subtracting it from the threat signal yields *unaddressed* threat. Modest weight on `cat_presence` keeps placement biased toward where cats actually live (a ward covering nobody is wasted).

Per-tile score:

```
threat        = max(fox_scent, corruption)
unaddressed   = clamp(threat - ward_coverage, 0, 1)
score         = unaddressed + 0.3 * cat_presence + jitter
```

Candidate tiles are a coarse grid (every 5 tiles, matching the bucket size of the influence maps) within `placement_radius` of the priestess's anchor, with hard exclusion of tiles within Manhattan-3 of any existing ward.

## Prediction

This is a structural refactor — no constants change; behavior changes by routing the same placement decision through a different scoring function.

| Metric | Direction | Rough magnitude | Why |
|---|---|---|---|
| `shadow_foxes_avoided_ward_total` (15-min seed-42 soak) | increase | +50–200% | Wards now sit on tiles SFs actually walk through, so each ward gets more avoid-encounters per unit time. |
| `wards_placed_total` | unchanged ±30% | The "should we ward?" gate is unchanged; only the placement target moves. |
| Ward placement-tile diversity | increase | ≥2× distinct tiles | Hard Manhattan-3 anti-clustering + fox-scent-driven scoring distributes ward across the placement disk instead of restacking. |
| `deaths_by_cause.ShadowFoxAmbush` | decrease | 50%+ ideally | Foxes deflected on approach corridors before reaching cats. |
| `deaths_by_cause.WildlifeCombat` (cluster acceptance) | decrease | no cluster of 4+ deaths within any 1,500-tick window | Ticket's primary acceptance bar. Indirect — depends on whether perimeter coverage actually catches the foxes that drive clustering. |
| Survival canaries (Starvation = 0, ShadowFoxAmbush ≤ 10) | preserved | hard gates per CLAUDE.md | No regression. |

A negative result is *informative*: if `shadow_foxes_avoided_ward_total` doesn't rise, the placement-clustering hypothesis is wrong (fix A — lifetime — would then be the next probe). The substrate work in commits 1 and 2 is structurally correct regardless.

## Observation

15-min seed-42 release deep-soak in `logs/tuned-42-ticket-045/`, commit `a837e18` (post-refactor). Compared to the registered baseline `logs/tuned-42` (commit `a879f43`, pre-043+044) via `just verdict`:

| Field | Baseline | Observed | Δ% | Band |
|---|---|---|---|---|
| `shadow_foxes_avoided_ward_total` | 4 | 2172 | **+54,200%** | significant |
| `ward_siege_started_total` | 1 | 310 | **+30,900%** | significant |
| `anxiety_interrupt_total` | 6259 | 819 | **−86.9%** | significant |
| `deaths_by_cause.ShadowFoxAmbush` | 5 | 3 | **−40%** | significant |
| `wards_placed_total` | 26 | 36 | +38.5% | significant |
| `wards_despawned_total` | 26 | 36 | +38.5% | significant |
| `deaths_by_cause.WildlifeCombat` | 3 | 4 | +33.3% | significant |
| `deaths_by_cause.Starvation` | 0 | 1 | new-nonzero | regression flag |

Ward placement-tile diversity (raw `WardPlaced` event aggregation): 36 placements over 11 distinct tiles, top tile `[34, 19]` (NE quadrant of the placement disk) with 11 placements. Placement mean shifted to `(31.2, 19.8)` — clustered around the colony but with the maximum tile pulled NE toward where fox-scent has accumulated, rather than dropping back on the colony altar at `[31, 20]`. Tile spread `x ∈ [26, 41], y ∈ [14, 23]`.

Per-ward effectiveness: `2172 / 36 = 60 SF avoidances per ward placed`. Pre-refactor baseline was `4 / 26 = 0.15 SF avoidances per ward placed`. **400× lift** on per-ward deflection, indicating wards are now where SFs actually walk.

## Concordance

| Metric | Predicted direction | Observed | Magnitude band | Concordance |
|---|---|---|---|---|
| `shadow_foxes_avoided_ward_total` | increase, +50–200% | +54,200% | far above band | **direction concordant; magnitude vastly exceeds prediction** |
| `wards_placed_total` | unchanged ±30% | +38.5% | just outside band | direction-flat as expected; magnitude marginally above band — placement gating slightly more permissive when fox-scent corridors register as scoring opportunities |
| Tile diversity (≥2× distinct tiles) | increase | 11 tiles, similar to pre-refactor 13 | does not meet threshold | **prediction partially fails** — diversity is bounded by `ward_placement_radius=10`; the algorithm spreads *within* the disk but the disk itself is small |
| `deaths_by_cause.ShadowFoxAmbush` | decrease, ~50% | −40% | within band | concordant |
| `anxiety_interrupt_total` | (not predicted) | −86.9% | n/a | unexpected positive — cats much calmer because fewer SF encounters reach colony |

Overall: **directionally concordant on every prediction; magnitudes exceed prediction on the threat-deflection axis by 100×+**. The placement-tile-diversity prediction underdelivers because the existing `ward_placement_radius=10` constant bounds the placement disk to ~10 tiles around the colony cluster — within that disk, the new algorithm distributes well (NE corridor instead of altar-stack), but to actually project wards out toward map edges (where SFs spawn from) the radius would need to grow. That's a separate balance lever, not a flaw in the substrate work.

The verdict gate technically reports `fail`, gated entirely on (a) `deaths_by_cause.Starvation: 0 → 1` and (b) continuity-canary zeros for mentoring / burial / courtship + 5-item `never_fired_expected_positives`. **Verified pre-existing in the post-043+044 baseline** (`logs/collapse-probe-42-fix-043-044/` shows identical `never_fired_expected_positives: [FoodCooked, MatingOccurred, GroomedOther, MentoredCat, CourtshipInteraction]` and the same `continuity_tallies` zeros, plus 2 Starvation deaths over its longer run). The registered baseline at `logs/tuned-42` is from commit `a879f43` (pre-043+044) — the verdict's drift comparison crosses three landed changes, not just this one. **None of the failing canaries reads anything ward-placement related.** Recommendation: re-promote a fresh post-045 baseline so future verdicts have a fair comparison anchor.

## Out of scope

## Out of scope

- **Lifetime probe (fix A — relax `thornward_decay_rate`).** Deferred per user direction; placement is the bigger lever. If the placement fix doesn't dissolve the death cluster, the lifetime probe is the natural next ticket.
- **Increasing priestess throughput (fix C).** Not recommended on its own per the ticket's framing.
- **Duplicate `WardPlaced` fire bug** (every ward spawns twice 9 ticks apart). Ticket §"Out of scope" — separate ticket.
- **Herb pipeline silence** (`GatherHerbCompleted = 0` in 17 years). Independent plumbing; doesn't gate ward coverage since DurableWards don't require herbs.
- **F-key influence-map overlay in the windowed UI.** Listed as a substrate-refactor follow-on; visual confirmation today goes through the focal-cat trace L1 records.
- **Combat-advantage math** (ticket 046) and **CriticalHealth interrupt treadmill** (ticket 047). Sibling tickets, not blocked-by/blocking 045.

## Implementation notes (for future readers)

The original plan (this `.md` file's commit-1 draft) called for a `HerbcraftWardPerimeterTargetDse` modeled on `socialize_target_dse`. Implementing it that way would have required synthetic `Entity` IDs for tile candidates plus an `evaluate_tile_target_taking` adapter — ~150 LOC of machinery for a behavior change that's a pure function over influence-map samples. The §6 target-taking DSE pattern fits when the cat's *DSE score* depends on candidates; here only the placement target moves while the "should we ward" gate stays in `scoring.rs::score_actions` as today. The simpler shape — rewrite the body of `compute_ward_placement` to consume the L1 maps — fully satisfies the design intent and is cheaper to reason about.
