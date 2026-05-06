---
id: 191
title: Tune PickingUp scavenge_urgency curve + add scenario test (185 follow-on)
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

Ticket 185 wired the `HasGroundCarcass` marker and lifted the
`PickingUp` DSE curve from `Linear { slope: 0.0, intercept: 0.0 }`
to a single inverted-Logistic axis on `colony_food_security`
(`Logistic(8.0, 0.5)` + `PostOp::Invert`). The midpoint and
steepness are structurally chosen, not empirically validated.

Two follow-on items deferred from 185's landing:

1. **Balance hypothesize loop.** The 0.5 midpoint may over- or
   under-fire scavenging vs Hunt. Run a `just hypothesize`
   four-artifact loop measuring `OverflowToGround` count drops
   and `food_stockpile_peak` rises against the pre-wave
   baseline.
2. **Scenario test** (`src/scenarios/picking_up_scavenging.rs`).
   Sister to `hunt_deposit_chain`: one cat empty inventory, one
   Stores, three carcasses spawned on the ground, no prey alive.
   Assert the cat elects PickingUp, picks up a carcass,
   completes the deposit chain to Stores. Final stockpile ≥ 1.

## Direction

- Form a hypothesis on the Logistic params: e.g., "lifting
  midpoint to 0.6 will narrow scavenging to seasons of genuine
  food shortage; lowering to 0.4 will broaden it to opportunistic
  scavenging year-round."
- Add the scenario as `src/scenarios/picking_up_scavenging.rs`.
  Use the existing `Carcass` spawn helper (mirror
  `disposal_election.rs`'s inventory-stuffing helper for the
  carcass spawn path).
- Validate via `just hypothesize <spec.yaml>`. Concordance check
  against pre-wave baseline; survival hard-gates pass.

## Out of scope

- The DSE wiring itself (185 landed it).
- New constants for the Logistic params (extract only if
  hypothesize iteration calls for it).

## Verification

- Scenario test passes deterministically.
- `OverflowToGround` count drops to <50% of pre-185 baseline.
- `food_stockpile_peak` rises ≥10% on the canonical seed-42
  soak.
- Survival hard-gates pass.

## Log

- 2026-05-06: opened by 185's land-day follow-on. The unit-test
  surface in `picking_up.rs::tests` covers curve shape and
  eligibility; the integration scenario + balance loop are
  deferred to here per CLAUDE.md antipattern-migration discipline.
