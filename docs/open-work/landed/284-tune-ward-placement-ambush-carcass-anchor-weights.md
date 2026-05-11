---
id: 284
title: tune ward placement ambush + carcass anchor weights
status: done
cluster: balance
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-11
---

## Why
210's post-soak diagnosis: 29 wards placed, 38 ShadowFox ambushes
still landed, 60-70% of ambushes concentrated in 2-3 hot-zone tile
clusters near colony center — wards were elsewhere because the
placement scorer used `max(fox_scent, corruption)` (where foxes
*patrol*, i.e. the geometric perimeter) rather than where they
actually *strike*. Ticket 220 plumbed the fix — added two sigmoid
lifts to `compute_ward_placement()`'s threat term consuming
`RecentAmbushMap` (event memory, 219) and `CarcassScentMap`
(kill-site scent, Phase 2C) — but landed both weights at 0.0 so
the substrate change carried no behavioral delta. This ticket
**activates the substrate** by lifting both weights off 0.0 and
verifying wards land at empirical ambush hot zones.

`ShadowFoxAmbush` deaths are a hard survival gate
(`deaths_by_cause.ShadowFoxAmbush <= 10` per CLAUDE.md
§Verification). The 210 baseline reproduced 38 — that's a 3.8×
gate violation. Closing it is load-bearing for hitting the
healthy-colony continuity canaries.

## Scope
- Run a `just hypothesize` four-artifact methodology cycle: spec
  with `ward_ambush_anchor_weight` and `ward_recency_anchor_weight`
  as the swept parameters. Hypothesis: lifting both weights
  shifts `WardPlaced` event positions toward tiles with non-zero
  `recent_ambush_at_position` / `carcass_scent_at_position` and
  reduces `deaths_by_cause.ShadowFoxAmbush` materially vs the
  220 dormant baseline.
- Land specific positive values in
  `src/resources/sim_constants.rs`'s
  `default_ward_ambush_anchor_weight()` and
  `default_ward_recency_anchor_weight()`. The two weights share
  the same threat term (additive lifts before `.min(1.0)`) so the
  spec should sweep them jointly, not independently — every cat
  death from ambush is *both* an ambush event and a kill site,
  so the two signals are correlated by construction and
  independent tuning double-counts.
- Update the doc-comments on both constants to cite the tuning
  ticket and the empirical landing values (mirror the 211 pattern
  on `coordinate_food_security_weight`).
- Append the balance writeup to `docs/balance/` (new file or
  extend `181-*` / `210-*` if they exist as living threads).

## Out of scope
- Adding new axes / curve shapes / substrate sources — that's
  220's surface, not this ticket's. Tuning operates on the
  existing two `Logistic(8.0, 0.5)` lifts.
- Reactive ward removal / migration. If static placement still
  drifts after this tunes, open a separate follow-on for
  ward-cycling.
- Cleansing/banishment knobs. Ward placement is upstream of
  ambush outcomes; those knobs are separate concerns.
- Sweeping at the `compute_ward_placement` formula level
  (e.g. changing the `0.3 × cat_value` term). Stay inside the
  two new weights.

## Current state
220 landed at `5348be2d` (2026-05-11). The substrate is in
place; both weights default to `0.0`; dormancy invariant unit
test passes (`ward_placement_dormant_at_default_weights`); the
shifts-to-hotspot unit tests pass at test-local weight 1.0. The
substrate change ships behaviorally inert by design — this
ticket flips the switch.

220's verification step (seed-42 dormancy soak via
`just soak-trace 42 Wren` + `just verdict`) should run first.
If that soak fails, 284 parks until the substrate is fixed; if
it passes, the dormant 220 baseline becomes this ticket's
comparison baseline.

## Approach
1. Write `docs/balance/284-ward-anchor-tuning.yaml` (or
   `docs/hypothesis/...` — whichever the `just hypothesize`
   spec convention is at land time). Hypothesis:
   `{ recent ambushes cluster spatially, and wards placed at
   those clusters intercept future ambushes } ⇒ { lifting
   ward_ambush_anchor_weight to 0.5 + ward_recency_anchor_weight
   to 0.3 reduces deaths_by_cause.ShadowFoxAmbush by ≥30% vs
   the 220 dormant baseline, with WardPlaced events visibly
   clustering on high recent_ambush_at_position tiles }`.
2. Run `just hypothesize <spec>` for the baseline + treatment
   sweeps + concordance check.
3. If concordance passes (direction match + magnitude within
   ~2×), land the treatment values as the new defaults.
4. If concordance fails, iterate: smaller weight, or asymmetric
   weights (ambush dominant, carcass minor), or rethink the
   curve midpoint. Document each iteration in the balance writeup.

Starting-point intuition for the spec:
- `ward_ambush_anchor_weight = 0.5` — strong enough to dominate
  the baseline `fox_scent.max(corruption)` term when an ambush
  cluster is present, but not so strong it overrides cat_presence
  and distance_cost considerations.
- `ward_recency_anchor_weight = 0.3` — carcass scent persists
  longer than the event-decay ambush map, so a smaller weight
  avoids the chronic-corpse case dragging wards to old
  kill-sites long after the threat has moved.

These are *prior intuitions*, not commitments — the four-artifact
methodology picks the actual landing values.

## Verification
- `just hypothesize <spec>` exits with concordance pass.
- Treatment soak via `just soak-trace 42 Wren`:
  - `deaths_by_cause.ShadowFoxAmbush` drops materially (target
    ≥30% below the 220 dormant baseline; the hard gate at ≤10
    is the floor, but the goal is closing the gap toward 0).
  - `WardPlaced` event positions cluster on tiles with
    non-zero `recent_ambush_at_position` or
    `carcass_scent_at_position` in the focal-trace sidecar.
  - All continuity canaries hold (grooming · play · mentoring ·
    courtship · mythic-texture).
  - `Starvation == 0` and `never_fired_expected_positives == 0`
    (hard survival gates regardless of the tuning's intent).
- `just verdict <run-dir>` concords against
  `logs/baselines/current.json`.
- The balance writeup names the four artifacts (hypothesis ·
  prediction · observation · concordance) and any iteration
  history.

## Log
- 2026-05-11: opened as the tuning follow-on to 220's dormant
  substrate landing. Discipline-failure recovery: should have
  been opened in 220's landing commit per the
  "antipattern-migration follow-ups are non-optional"
  CLAUDE.md rule.
- 2026-05-11: first-light soak (logs/tuned-42, commit 81e555db dirty): wards visibly cluster on (29,23-24) and (37-39, 20-23) ambush hotzones; 27 Ambush events (vs 37 post-210 reference, -27%); macro counters identical to current.json baseline (magnitude flat); 5 continuity canaries hold; Starvation==1 hard-gate breach is baseline carryover not 284-induced; magnitude-tightening follow-on opened as 285
