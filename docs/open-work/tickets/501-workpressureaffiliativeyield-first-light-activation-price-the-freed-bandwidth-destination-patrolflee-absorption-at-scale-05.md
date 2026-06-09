---
id: 501
title: WorkPressureAffiliativeYield first-light activation — price the freed-bandwidth destination (Patrol/Flee absorption at scale 0.5)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-06-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Ticket 490 (R3) landed `WorkPressureAffiliativeYield` — a registered
multiplicative damp on {Socialize, GroomOther} keyed on physical-need
pressure (`1 − phys_satisfaction`) — **dormant at scale 0.0**. The
2026-06-09 first-light A/B (seed 42, 120 s, scale 0.5) confirmed the
mechanism fires but the freed bandwidth landed in the wrong place:
Patrol 13% → 22% of snapshots, Flee 23% → 33%, **Cook 19% → 11%**,
founder dispersion below the 490 canary floor. Textbook
Patrol-absorption cascade (memory `project_l3_patrol_absorption_cascade`;
same shape as the 487 gate-fix lesson, one layer up): a damp on one
class is only half a lever — the contest's runner-up must be priced
too.

## Scope
- Diagnose WHY Eat/Forage/Hunt lost the freed contest to Patrol at
  pressure > 0.5 (hypothesis candidates: Eat ineligible at range /
  stores empty at those moments; Patrol's predator-exposure cost
  unpriced; Hunt gated by HuntingPriors/beliefs). Focal-trace the L2
  contest at damped ticks (`just soak-trace` + `just q trace`).
- Structural options to draft before any parameter retune (bugfix
  discipline): (a) price predator-exposure into Patrol's score shape
  (the standing 358-family gap the L3-Patrol-absorption memory names);
  (b) make the damp conditional on a work action being *eligible*
  (substrate-honest "yield to work" rather than "yield to whatever");
  (c) narrower pressure axis (hunger-only rather than composite).
- Activation via `just hypothesize` (four-artifact); scale sweep
  0.2/0.35/0.5.

## Out of scope
- The modifier's shape/registration (landed in 490, pipeline slot 39).
- The founder-dispersion canary (landed; this ticket must keep it
  green).

## Current state
Modifier registered + unit-tested, dormant. All substrate present;
this is tuning + one structural fix.

## Approach
Start from the 490 A/B artifacts (logs at /tmp were transient — re-run
the three-variant comparison via `CLOWDER_OVERRIDES` on scale). The
activation gate: Cook/Forage/Eat shares rise when the damp fires,
Patrol/Flee shares flat, dispersion ≥ canary floor, mating canaries
green.

## Verification
- `just hypothesize` concordant on the chosen scale.
- `just verdict`: founder-dispersion floor green, MatingOccurred /
  KittenBorn fire, continuity tallies in band.
- Action-share table pre/post: freed bandwidth lands in work classes.

## Log
- 2026-06-11: opened from 490's first-light A/B (R3 shipped dormant).
