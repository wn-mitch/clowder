---
id: 190
title: Tune build_chronic_full_weight (179 follow-on)
status: ready
cluster: balance
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 179 wired the `ColonyStoresChronicallyFull` marker into
`BuildDse` as a fourth `MarkerConsideration` axis and lifted
`default_build_chronic_full_weight` from `0.0` (dormant) to `0.5`
(plausibility). The 0.5 value is structurally chosen, not
empirically validated. Balance discipline per CLAUDE.md requires
a `just hypothesize` four-artifact loop on any axis that's been
introduced or lifted on a characteristic metric.

## Direction

- Form a hypothesis: lifting `build_chronic_full_weight` from
  `0.0` to `0.5` should shift Build action share upward when
  `ColonyStoresChronicallyFull` latches, leading to non-zero
  Stores-construction completions in soaks where the colony
  has chronic deposit-rejection.
- Measure pre-179 vs post-179 (or post-wave) action shares for
  Build and chronic-full latch counts.
- If the lift is too weak, raise toward 0.7 — 1.0; if too
  strong (Build crowds out other Maslow-2 work), lower toward
  0.2 — 0.3.
- Validate via `just hypothesize <spec.yaml>` with the
  characteristic metric being either `colony_score.aggregate`,
  `final_food_stockpile`, or a Build-action-share footer field.

## Out of scope

- The DSE wiring itself (179 landed it).
- Coordinator-side directive arbitration (already covered by
  existing `assess_build_pressure`).

## Verification

- Concordance: hypothesis prediction's direction + magnitude
  match observation within ~2× per `docs/balance/*.md`.
- Survival hard-gates pass at the new value.

## Log

- 2026-05-06: opened by 179's land-day follow-on. The 0.5
  plausibility default may over- or under-weight the chronic-
  full pull; validate empirically once post-wave (179+185+188)
  baseline lands.
