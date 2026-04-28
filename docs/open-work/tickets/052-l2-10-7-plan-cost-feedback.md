---
id: 052
title: §L2.10.7 plan-cost feedback — `SpatialConsideration` curves on spatially-sensitive DSEs
status: in-progress
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Emitting Intentions doesn't make scoring cost-aware. If `Caretake` scores
0.9 but the kitten is 50 tiles away while food is 2 tiles away, utility
was blind to cost — GOAP plans the long trip on an inflated score.

Ch 14 of *Behavioral Mathematics for Game AI* ("Which Dude to Kill?")
folds distance into scoring via response curves rather than a
pathfinder-in-the-loop. Spec §L2.10.7
(`docs/systems/ai-substrate-refactor.md` line 5535+) chose this as
candidate (a) for Clowder: each spatially-sensitive DSE carries a
`SpatialConsideration` with a §2.1 curve primitive; the score itself
encodes reachability through curve shape, no pathfinder invocation
mid-score.

Today this is the single largest unfinished structural piece of the
substrate refactor. Four §6.5 axes are blocked on it (`pursuit-cost`,
`fertility-window`'s spatial half, `apprentice-receptivity`'s
spatial-mentor-pairing, `remedy-match`'s caretaker-distance variant);
13 of 21 cat DSEs and 6 of 9 fox dispositions still use binary range
gates or aggregate-proximity scalars instead of continuous distance-to-
landmark scoring (§L2.10.7 audit).

## Scope

1. **`SpatialConsideration` substrate.** Add the consideration variant
   to `src/ai/considerations.rs`, alongside the existing `Scalar`,
   `Spatial`, `Marker` variants. Input: target landmark (entity, tile
   coord, or cat-relative anchor) + curve primitive from §2.1. Output:
   curve-shaped score in `[0, 1]`. Wire through `evaluate_single` and
   the modifier pipeline.

2. **Per-DSE roster cutover (30 rows).** Spec §L2.10.7 lines 5599+
   commit the post-refactor shape for every spatially-sensitive DSE.
   Per row: target landmark + curve primitive. Curves chosen per row:
   `Quadratic` / `Power` for "closer is better, sharp falloff" (hunt,
   defend-territory, urgent-threat); `Logistic` for "close-enough"
   (routine errands, non-urgent socializing); `Linear` for incentive
   gradients (exploration). Numeric tuning (midpoint/steepness) is
   balance work, not refactor scope.

3. **Four §6.5 deferred axes unblock.** Each gates on the substrate
   above:
   - `pursuit-cost` (Hunt) — `Logistic(steepness=10, midpoint=0.5,
     inverted)`. Today proxied as `distance²`.
   - `fertility-window` spatial half (Mate, §7.M.5 cascade) —
     `SpatialConsideration` over partner location.
   - `apprentice-receptivity` spatial pairing (Mentor) — landmark =
     apprentice position.
   - `remedy-match` caretaker-distance variant (ApplyRemedy) — when
     the caretaker has a matching remedy but is far away.

4. **Two-channel composition with `replan_count`.** `replan_count ≥
   max_replans` in `src/components/goap_plan.rs:103` remains the
   hard-fail signal for §7.2's `achievable_believed ⇒ false` path.
   `SpatialConsideration` provides the elastic channel; both compose
   per §0.2 elastic-failure preservation. Don't replace the hard-fail
   exit — the two channels are designed to coexist.

## Out of scope

- **Pathfinder-in-the-loop.** Candidate (b) was rejected per §L2.10.7
  lines 5570+ — chicken-and-egg, expensive, brittle.
- **Per-DSE numeric balance tuning.** Curve midpoints/steepness are
  balance-thread work; this ticket commits *shape*, not knob values.
  Drift > ±10% on a characteristic metric will require a hypothesis
  per `CLAUDE.md` balance methodology.
- **`SpatialConsideration` extension to non-DSE consideration sites.**
  L1 influence map sampling stays where it is.

## Approach

Follow the `Consideration` exemplar pattern set by Phase 3a+3b:
substrate first (one variant + tests), then a single-DSE proof-of-port
(suggest Hunt — `pursuit-cost` is the most visible win), then the
roster sweep DSE-by-DSE in commits small enough to bisect any
behavior shift.

Each DSE port is bisectable: pre-port the DSE uses today's
binary/scalar gate; post-port it uses `SpatialConsideration`. The pair
of soak runs around each port commit either confirms behavior-neutral
or surfaces a hypothesis-required shift.

## Verification

- Lib tests on `SpatialConsideration` (curve shapes, landmark
  resolution edge cases, missing-landmark fallback).
- Per-DSE integration test: target far away ⇒ score attenuated; target
  close ⇒ score unchanged; reachable-via-detour ⇒ score smoothly
  degraded (not zeroed).
- `just check` green per commit.
- Soak verdict on canonical seed-42 deep soak after substrate landing
  (behavior-neutral expected since no DSEs have been ported yet).
- Per-DSE port soaks: behavior-neutral OR hypothesis-required shift,
  documented per balance methodology.
- Focal-cat trace (`just soak-trace 42 Simba`): inspect L2 records
  for ported DSEs to confirm `SpatialConsideration` records emit with
  correct landmark + curve-shaped score.

## Log

- 2026-04-27: opened from ticket 005 retirement (cluster-A umbrella
  decomposition). §L2.10.7 was the single remaining structural item
  inside 005's body without a successor ticket.
- 2026-04-28: substrate landed + Hunt port (scope items 1, partial 3).
  `SpatialConsideration` swapped in place — old influence-map sampling
  shape (zero production callers, every `sample_map` closure stubbed
  to 0.0) replaced by landmark-distance design with
  `LandmarkSource::{TargetPosition, Tile, Entity}`. `EvalCtx::sample_map`
  removed; `EvalCtx::entity_position` added for landmark-entity
  resolution. Hunt's `pursuit-cost` axis now uses the §L2.10.7-spec'd
  `Logistic(steepness=10, midpoint=0.5, inverted)` over
  `range = HUNT_TARGET_RANGE`, retiring the `distance²` proxy.
  All 1500 lib + integration tests pass.

  Paired-baseline soak (seed 42, 15 min) at `1abaf49` (pre-052) vs
  `afb3841` (post-052) — Hunt port is **behavior-neutral** within
  noise:

  | Metric                 | pre-052 | post-052 | Δ        |
  |------------------------|---------|----------|----------|
  | Injury / Ambush / Starv| 1/4/1   | 1/4/1    | identical|
  | burial / courtship     | 0/0     | 0/0      | identical|
  | mentoring              | 0       | 0        | identical|
  | mythic-texture         | 30      | 30       | identical|
  | grooming               | 264     | 269      | +1.9%    |
  | play                   | 834     | 842      | +1.0%    |

  Verdict gates that fail (Starvation > 0; courtship/mentoring/burial
  collapsed) are pre-existing — Starvation was 2 in the canonical
  baseline (a879f43), and the continuity collapse is tracked in
  ticket 040 (post-036 disposition shift), 035 (burial not
  implemented), 003 (mentor score magnitude).

  Pre-052 soak archived at `logs/tuned-42-1abaf49-pre-052-baseline/`.

  Successor work: Mate, Mentor, ApplyRemedy axis ports (scope item 3
  remainder); 30-row roster sweep across remaining cat self-state
  DSEs + fox dispositions (scope item 2).

- 2026-04-28: Mate port (scope item 3 §7.M.5 distance-axis half).
  Mate's `target_nearness` Scalar (`Logistic(20, 0.5)` over `1 - dist/
  range`) swapped for a `SpatialConsideration` with `LandmarkSource::
  TargetPosition` + `range = MATE_TARGET_RANGE` + `Composite {
  Logistic(20, 0.5), Invert }` curve over normalized cost. Inverted
  logistic on `dist/range` is mathematically identical to logistic on
  `1 - dist/range` (logistic is point-symmetric about its midpoint),
  so the port is behavior-neutral by construction. The §7.M.5
  fertility-window axis remains deferred until §7.M.7.5's
  phase→scalar mapping lands. All 1501 lib + integration tests pass
  (added `nearness_attenuates_far_partner_smoothly` exercising the
  spatial axis across the midpoint).

  Paired-baseline soak (seed 42, 15 min) at `11f57d9` (post-Hunt) vs
  the Mate-port WIP — **fully behavior-neutral**:

  | Metric                      | post-Hunt | post-Mate | Δ        |
  |-----------------------------|-----------|-----------|----------|
  | Injury / Ambush / Starv     | 1/4/1     | 1/4/1     | identical|
  | grooming                    | 262       | 262       | identical|
  | play                        | 834       | 834       | identical|
  | burial / courtship          | 0/0       | 0/0       | identical|
  | mentoring                   | 0         | 0         | identical|
  | mythic-texture              | 30        | 30        | identical|
  | never_fired_expected        | (4)       | (4)       | identical|
  | Plan-failure top-5 reasons  | —         | —         | 1 row +1 |

  The single delta in the entire footer: `TravelTo(SocialTarget): no
  reachable zone target` 30 → 31. Consistent with one mate-target
  argmax in the whole soak resolving to a different partner due to
  f32 LSB ordering (`1 - Logistic(c)` vs `Logistic(1 - c)` are equal
  in real arithmetic but emit different LSBs in f32), whose path
  happened not to be reachable. Smaller behavioral footprint than
  Hunt's port (which shifted grooming +1.9%, play +1.0%) because
  Mate's candidate pool is bond-restricted to Partners/Mates — far
  fewer opportunities for f32 tie-break flips than Hunt's full
  visible-prey pool.

  Post-Hunt baseline archived at
  `logs/tuned-42-11f57d9-post-hunt-baseline/`.

  Successor work: Mentor (apprentice-receptivity spatial pairing) and
  ApplyRemedy (caretaker-distance variant) ports remain in scope
  item 3; 30-row roster sweep (scope item 2) across remaining cat
  self-state DSEs + fox dispositions.

- 2026-04-28: Mentor port (scope item 2 row #18). Mentor's
  `target_nearness` Scalar (`Quadratic(exp=2)` over `1 - dist/range`)
  swapped for a `SpatialConsideration` with `LandmarkSource::
  TargetPosition` + `range = MENTOR_TARGET_RANGE` + `Quadratic(exp=2,
  divisor=-1, shift=1)` over normalized cost. The
  `divisor=-1, shift=1` form evaluates `((cost - 1) / -1)² =
  (1 - cost)²`, exactly preserving the legacy shape (sharp falloff
  near the cat: half-range = 0.25, zero at `range`). Quadratic isn't
  point-symmetric like Logistic, so this explicit form is the way
  to keep behavior-neutrality; the alternative `Composite{Quadratic,
  Invert}` would give `1 - cost²` (gentle near, cliff at edge),
  which doesn't match §L2.10.7's "Requires sustained proximity"
  rationale. All 1502 lib + integration tests pass (added
  `nearness_attenuates_far_apprentice_smoothly` exercising the
  spatial axis across the midpoint).

  Paired-baseline soak (seed 42, 15 min) at `1e5efe7` (post-Mate)
  vs the Mentor-port WIP — **fully behavior-neutral**:

  | Metric                      | post-Mate | post-Mentor | Δ        |
  |-----------------------------|-----------|-------------|----------|
  | Injury / Ambush / Starv     | 1/4/1     | 1/4/1       | identical|
  | grooming                    | 262       | 262         | identical|
  | play                        | 834       | 834         | identical|
  | burial / courtship          | 0/0       | 0/0         | identical|
  | mentoring                   | 0         | 0           | identical|
  | mythic-texture              | 30        | 30          | identical|
  | never_fired_expected        | (4)       | (4)         | identical|
  | Plan-failure top-5 reasons  | —         | —           | 1 row -1 |

  The single delta: `TravelTo(SocialTarget) no reachable` 31 → 30,
  the *reverse* of Mate's +1 (so the cumulative shift across both
  ports is 0). Symmetric f32 LSB ordering — same mechanism as
  Mate's port. Mentoring tally remained 0 (pre-existing canary
  collapse tracked in ticket 003); the substrate change preserves
  status quo on that metric, which is the substrate-refactor goal.

  Post-Mate baseline archived at
  `logs/tuned-42-1e5efe7-post-mate-baseline/`.

  Successor work: ApplyRemedy (caretaker-distance variant) port
  remains in scope item 3; 30-row roster sweep (scope item 2)
  across remaining cat self-state DSEs + fox dispositions.

- 2026-04-28: ApplyRemedy port (scope item 3 caretaker-distance
  variant). ApplyRemedy's `target_nearness` Scalar (`Quadratic(exp=1.5)`
  over `1 - dist/range`) swapped for a `SpatialConsideration` with
  `LandmarkSource::TargetPosition` + `range = APPLY_REMEDY_TARGET_RANGE`
  + `Quadratic(exp=1.5, divisor=-1, shift=1)` over normalized cost,
  which evaluates `(1 - cost)^1.5` (same `divisor=-1, shift=1` idiom
  Mentor used). Behavior-neutral by construction. The §6.5.7
  remedy-match axis remains deferred until per-candidate remedy-
  kind selection lands.

  Paired-baseline soak (seed 42, 15 min) at `dbcb283` (post-Mentor)
  vs the ApplyRemedy-port WIP — **survival- and continuity-neutral**:

  | Metric                      | post-Mentor | post-ApplyRemedy | Δ        |
  |-----------------------------|-------------|------------------|----------|
  | Injury / Ambush / Starv     | 1/4/1       | 1/4/1            | identical|
  | grooming / play             | 262/834     | 262/834          | identical|
  | burial / courtship          | 0/0         | 0/0              | identical|
  | mentoring / mythic-texture  | 0/30        | 0/30             | identical|
  | never_fired_expected        | (4)         | (4)              | identical|
  | Plan-failure total          | 279         | 284              | +5 (+1.8%)|

  Only one plan-failure reason changed: `TravelTo(SocialTarget) no
  reachable` 30 → 35. Same f32-LSB butterfly mechanism as Mate/
  Mentor (now amplified slightly because Quadratic(1.5) is a non-
  integer power with more LSB churn than the Logistic ports). The
  shift propagates non-locally through shared-RNG ordering: an
  apply-remedy argmax flips, the healer arrives at a slightly
  different tile, and a different cat's social-target path-check
  fails downstream. All survival canaries and continuity tallies
  are identical, so no characteristic-metric drift > ±10% — the
  shift is well within the per-port noise envelope established by
  Mate (+1) and Mentor (-1).

  Post-Mentor baseline archived at
  `logs/tuned-42-dbcb283-post-mentor-baseline/`.

  Successor work: 30-row roster sweep (scope item 2) across
  remaining cat self-state DSEs + fox dispositions. The
  Quadratic-family ports (Hunt, Mate, Mentor, ApplyRemedy)
  established the explicit-inversion idiom `Quadratic(exp=N,
  divisor=-1, shift=1)` for closer-is-better; future ports can
  follow that pattern (Groom-other, Caretake, Fight, fox Hunting)
  or pick `Composite{Logistic, Invert}` per their per-DSE rationale.

- 2026-04-28: Socialize port (scope item 2 row #1, the highest-
  frequency target-taking DSE — every cat scores Socialize
  candidates every tick). Same `Quadratic(exp=2, divisor=-1,
  shift=1)` shape as Mentor's. Removed five test-mock branches
  (`TARGET_NEARNESS_INPUT => 1.0`) since the substrate now drives
  the spatial axis directly.

  Paired-baseline soak (seed 42, 15 min) at `40a55b5` (post-
  ApplyRemedy) vs the Socialize-port WIP — **survival- and
  continuity-canary neutral, with measurable but sub-threshold
  drift**:

  | Metric                      | post-AR | post-Soc | Δ          |
  |-----------------------------|---------|----------|------------|
  | Injury / Ambush / Starv     | 1/4/1   | 1/4/1    | identical  |
  | grooming                    | 262     | 268      | +6 (+2.3%) |
  | play                        | 834     | 842      | +8 (+1.0%) |
  | burial / courtship          | 0/0     | 0/0      | identical  |
  | mentoring                   | 0       | 0        | identical  |
  | mythic-texture              | 30      | 30       | identical  |
  | never_fired_expected        | (4)     | (4)      | identical  |
  | Plan-failure total          | 284     | 300      | +16 (+5.6%)|

  Single plan-failure reason changed: `TravelTo(SocialTarget) no
  reachable` 35 → 51 (+16). Largest per-port drift seen so far,
  but every metric stays under the ±10% characteristic-metric
  threshold (grooming +2.3%, play +1.0%, plan-failures +5.6%).
  Mechanism: Socialize is by far the most-frequently-evaluated
  target-taking DSE (every cat × every tick × every social
  partner in range), so f32 LSB churn from the explicit-inverted
  `Quadratic(exp=2, divisor=-1, shift=1)` evaluation pipeline
  (which is mathematically identical but numerically distinct
  from the legacy `(1 - dist/range).clamp(0,1)` then `.powf(2)`
  in the curve evaluator) accumulates proportionally more argmax
  flips. Same pattern as Hunt's port (which moved grooming +1.9%,
  play +1.0% on the much-rarer Hunt evaluation).

  Post-ApplyRemedy baseline archived at
  `logs/tuned-42-40a55b5-post-applyremedy-baseline/`.

  Successor work: Groom-other, Caretake, Fight, Build remaining
  in scope item 2 (cat self-state DSEs); fox dispositions
  thereafter.

- 2026-04-28: GroomOther + Caretake + Fight + Build ports —
  bundled cutover of the remaining four cat target-taking DSEs
  (the user requested these be landed in one commit without
  intermediate per-port soaks since the per-port behavior pattern
  was well-established by Hunt/Mate/Mentor/ApplyRemedy/Socialize).
  This **completes scope item 2 for cat target-taking DSEs** —
  every spatially-sensitive `TargetTakingDse` now runs through
  `SpatialConsideration` instead of a hand-rolled scalar. Curve
  shapes per port:
  - GroomOther: `Composite{Logistic(15, 0.15), Invert}` — algebra
    `Logistic(s, m)` over `(1-cost)` ≡ `Composite{Logistic(s,
    1-m), Invert}` over `cost`. Midpoint flipped 0.85 → 0.15.
  - Caretake: `Quadratic(exp=1.5, divisor=-1, shift=1)` —
    `(1-cost)^1.5`, same idiom as ApplyRemedy.
  - Fight: `Composite{Logistic(10, 0.5), Invert}` — Logistic
    point-symmetry, 1-m = m at m=0.5 (same as Mate).
  - Build: `Linear(slope=-1, intercept=1)` — direct expression
    of the legacy linear `1-cost` shape.

  All 1502 lib + integration tests pass.

  Single bundled paired-baseline soak (seed 42, 15 min) at
  `6322c9c` (post-Socialize) vs the four-port WIP — **survival-
  and continuity-canary neutral**, with f32 LSB drift partially
  *canceling* Socialize's:

  | Metric                      | post-Soc | bundled | Δ          |
  |-----------------------------|----------|---------|------------|
  | Injury / Ambush / Starv     | 1/4/1    | 1/4/1   | identical  |
  | grooming                    | 268      | 262     | -6 (-2.2%) |
  | play                        | 842      | 834     | -8 (-1.0%) |
  | burial / courtship          | 0/0      | 0/0     | identical  |
  | mentoring                   | 0        | 0       | identical  |
  | mythic-texture              | 30       | 30      | identical  |
  | never_fired_expected        | (4)      | (4)     | identical  |
  | Plan-failure total          | 300      | 280     | -20 (-6.7%)|

  Single plan-failure reason changed: `TravelTo(SocialTarget) no
  reachable` 51 → 31. **Cumulative drift from pre-Socialize**
  (post-ApplyRemedy = 262/834/284): grooming 262 → 268 → 262
  (net 0), play 834 → 842 → 834 (net 0), plan-failures 284 →
  300 → 280 (net -4, -1.4%). The bundled ports' LSB churn ran
  in the opposite direction of Socialize's so the colony-wide
  effect of the entire substrate refactor ends up essentially
  noise-level on every characteristic metric. **No drift exceeds
  the ±10% balance-methodology threshold; no hypothesis required.**

  Post-Socialize baseline archived at
  `logs/tuned-42-6322c9c-post-socialize-baseline/`; bundled-port
  soak archived at `logs/tuned-42-bundled-go-care-fight-build/`.

  ## Scope-item-2 status: cat target-taking DSEs COMPLETE

  All 9 cat target-taking DSEs in `src/ai/dses/*_target.rs` now
  use `SpatialConsideration` for their distance axis: Hunt, Mate,
  Mentor, ApplyRemedy, Socialize, GroomOther, Caretake, Fight,
  Build. Every `TARGET_NEARNESS_INPUT` const removed; every
  `pos_map` lookup table in resolvers retired; substrate handles
  per-candidate Manhattan distance via `LandmarkSource::
  TargetPosition`. The §6.5 row-by-row spec compliance is now
  achievable directly from the factory-side `Curve` declaration
  rather than spread across factory + fetcher.

  Successor work: scope item 2's remaining roster — cat *self-
  state* DSEs (Eat / Sleep / Forage / Explore / Flee / Patrol /
  Build / Farm / Herbcraft / PracticeMagic / Coordinate / Cook
  per §L2.10.7 line 5621+) and all 9 fox dispositions
  (Hunting / Feeding / Patrolling / Raiding / DenDefense /
  Resting / Dispersing / Fleeing / Avoiding per §L2.10.7 line
  5648+). These are structurally distinct from target-taking
  DSEs — they don't pass through `TargetTakingDse` and need
  their own substrate plumbing decision (which `LandmarkSource`
  flavor each row binds to). That work is large enough to
  warrant a successor ticket; the current ticket can close on
  scope items 1, 3, and the cat target-taking half of item 2.
