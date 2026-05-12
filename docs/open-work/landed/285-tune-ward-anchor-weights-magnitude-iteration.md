---
id: 285
title: tune ward anchor weights — magnitude iteration
status: done
cluster: balance
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md]
landed-at: pending
landed-on: 2026-05-12
---

## Why
284 activated `ward_ambush_anchor_weight` and `ward_recency_anchor_weight`
off the 220-dormant `0.0` at first-light values `0.5 / 0.3` and confirmed
the substrate fires (post-soak ward placements visibly cluster on the
empirical ambush corridor; soft continuity tallies drift +1-3% vs the
current.json baseline). But macro outcome counters did not move:
`wards_placed_total`, `shadow_fox_spawn_total`,
`shadow_foxes_avoided_ward_total`, and every `deaths_by_cause` field
landed identical to baseline on seed-42. The lifts redirect WHERE wards
land without (yet) shifting HOW MANY foxes are deterred or HOW MANY
ambushes connect.

285 carries the magnitude-tightening burden with hypothesize-grade rigor
that the first-light pass deferred. The Starvation==1 hard-gate breach is
a baseline carryover (not a 284 regression) but should be re-checked once
ward efficacy moves.

## Scope
- Run a `just hypothesize` four-artifact methodology cycle against
  `ward_ambush_anchor_weight` and `ward_recency_anchor_weight`. Hypothesis:
  larger weights (or different relative balance) materially shift
  `shadow_foxes_avoided_ward_total` upward and `Ambush` event counts
  downward vs the post-284 baseline.
- Sweep candidates: `(0.5, 0.3)` (post-284 anchor), `(0.7, 0.4)`,
  `(0.9, 0.5)`, plus an asymmetric `(1.0, 0.0)` to test whether
  carcass scent contributes anything beyond what ambush memory already
  captures. Pick the joint pair the methodology selects.
- Land the iterated values as new defaults; update the doc-comments at
  `src/resources/sim_constants.rs:1966-1987` to cite 285's empirical
  landing values (mirror the 211 pattern).
- Append to `docs/balance/284-ward-anchor-tuning.md` as iter-2 (and
  iter-N as iteration history grows).

## Out of scope
- Adding new axes / curve shapes / substrate sources to
  `compute_ward_placement` — that's 220's surface.
- Reactive ward removal / migration.
- Cleansing / banishment knobs.
- Sweeping at the `compute_ward_placement` formula level.

## Current state
284 landed `0.5 / 0.3` at commit <TBD-on-land> as the first-light
activation. Both unit tests pass:
`ward_placement_dormant_when_weights_forced_to_zero` (regression guard
that forces 0.0 explicitly) and the two
`ward_placement_shifts_to_*_hotspot_when_tuned` tests at test-local
weight 1.0. The substrate is wired and fires; magnitude is the open
question.

The current.json baseline at `4bcae2de` ("post-127-joint-intention",
2026-05-11 12:00 EDT) carries `Starvation: 1`, `ShadowFoxAmbush: 2`,
`wards_placed_total: 16`. 285's treatment is measured against the
post-284 footer, not against this older snapshot.

## Approach
1. Write `docs/balance/hypothesis-285-ward-anchor-magnitude.yaml`
   mirroring `hypothesis-102-acute-health-adrenaline-fight.yaml`. Sweep
   both weights jointly in `constants_patch` (they share the threat
   term — independent gridding double-counts; see 284's writeup).
   Predicted metric: `shadow_foxes_avoided_ward_total`, direction
   `increase`, rough_magnitude_pct `[50, 300]`.
2. Run `just hypothesize <spec>`; iterate per concordance band.
3. Land the values the methodology selects. Mirror 211's
   doc-comment update pattern (cite ticket + final value).

## Verification
- `just hypothesize <spec>` exits with concordance pass.
- Treatment soak: `just soak-trace 42 Wren` + `just verdict`.
- `shadow_foxes_avoided_ward_total` materially above the post-284
  baseline of 2.
- `Ambush` event count materially below the post-284 baseline of 27.
- All hard survival gates and continuity canaries hold.
- Balance writeup appends iter-2 to `284-ward-anchor-tuning.md`.

## Log
- 2026-05-11: opened as the magnitude-tightening follow-on to 284's
  first-light landing. 284's writeup ends with "macro counters
  magnitude-flat at 0.5/0.3" — that's this ticket's starting evidence.
- 2026-05-12: iter-2 four-artifact methodology across seeds 42/99/7; three-seed evidence locks Logistic-saturation finding; follow-ons 296 (curve shape) and 297 (perception axis) opened in same commit
