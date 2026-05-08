---
id: 228
title: fox-territory hunt/forage/patrol decision-time suppression
status: ready
cluster: pathfinder-risk-awareness
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

223's soak surfaced a regression that 223's path-cost overlay alone can't
close. With the legacy `FoxTerritorySuppression` damp branch retired,
cats land in fox-scent corridors at *decision time* (Hunt/Forage/Patrol
scores no longer suppressed near zero in fox territory) and then thrash
when `acute_health_adrenaline_flee` preempts every plan tick after their
hunger crashes. Three adult Starvation deaths clustered at adjacent
tiles in the [33,21]–[31,22] fox corridor in the verification soak;
0 courtship events; `MatingOccurred` / `CourtshipInteraction` /
`PairingIntentionEmitted` never fired.

The path-cost overlay (223) is *route-time* substrate — it gates which
A* path the cat takes *given* a destination. The damp branch was
*decision-time* substrate — it gates whether the cat picks a destination
*in* fox territory at all. These compose orthogonally per §4.7
substrate-vs-search-state — both are substrate, but they answer
different questions:

- **Score-time gate** ("should I do this *here*?") — modifier suppresses
  Hunt / Forage / Patrol / Wander / Explore when the cat's local fox
  scent exceeds threshold, so the L3 softmax tilts toward Eat /
  StockpileForage / Move-out-of-area.
- **Route-time gate** ("which route do I take?") — path-cost overlay
  raises A* edge cost on fox-scent tiles so paths to non-fox
  destinations route around the corridor. (Landed 223.)

Removing the score-time gate without a replacement leaves cats with no
reason not to hunt prey in fox territory; the path-cost overlay then
just produces longer-but-still-traversable detour paths *into* the same
risky corridor. Net effect: cats trapped in fox territory, starved.

## Scope

- New `ScoreModifier` impl `FoxTerritoryHuntSuppression` (or similar
  name; ship as a single-purpose modifier sibling to
  `FleeFoxScentBoost`). Reads `FOX_SCENT_LEVEL` perception scalar.
  Multiplicative damp on Hunt / Forage / Patrol / Wander / Explore
  using the existing `fox_scent_suppression_threshold` and
  `fox_scent_suppression_scale` constants from `ScoringConstants`.
- Register in `default_modifier_pipeline` next to `FleeFoxScentBoost`
  (matching the legacy `FoxTerritorySuppression` registration slot).
- Tests: parallel the deleted `fox_suppression_damps_hunt_*` tests
  from before 223. Plus a coverage test that asserts the modifier
  pairs cleanly with `FleeFoxScentBoost` (boost on Flee + damp on
  Hunt fire on the same tick without interaction).

## Out of scope

- Personality-conditioned suppression (bold cats accept more risk at
  decision time) — that's a parallel of ticket 224's path-weight
  conditioning at the score layer; can land as a follow-on if the
  symmetric shape proves desirable.
- Re-introducing the combined `FoxTerritorySuppression` modifier
  (boost+damp in one impl). Per option B in the 223 regression
  investigation, two single-purpose modifiers compose more cleanly
  with the substrate-over-override discipline than the legacy
  combined shape.

## Current state

- 223 landed the path-cost overlay (`FoxScentOverlay`,
  `CorruptionOverlay`) + retired the legacy
  `FoxTerritorySuppression` modifier (renamed to `FleeFoxScentBoost`,
  Flee branch only).
- This ticket lands the missing decision-time damp. After this lands,
  the cluster's intent is fully expressed: cats avoid choosing
  high-risk destinations AND route around fox scent on the way to
  non-fox destinations.

## Approach

1. Mirror `FleeFoxScentBoost`'s shape:
   ```rust
   pub struct FoxTerritoryHuntSuppression {
       threshold: f32,  // sc.fox_scent_suppression_threshold
       scale: f32,      // sc.fox_scent_suppression_scale
   }
   ```
   Reuse the existing `ScoringConstants` fields — no new constants.
2. `apply` matches on `HUNT | EXPLORE | FORAGE | PATROL | WANDER`,
   reads `FOX_SCENT_LEVEL`, computes the same suppression term
   `((fox_scent − threshold) / (1 − threshold)) × scale`, returns
   `score × (1 − suppression).max(0.0)`.
3. Register in `default_modifier_pipeline` adjacent to
   `FleeFoxScentBoost`. Order doesn't load-bear among the
   multiplicative damps (each gated by a different DSE-id matrix and
   a different scalar trigger).
4. Tests in `src/ai/modifier.rs::tests`:
   - `fox_hunt_suppression_damps_hunt_when_scent_above_threshold`
     (mirrors deleted `fox_suppression_damps_hunt_*`).
   - `fox_hunt_suppression_skips_when_below_threshold`.
   - `fox_hunt_suppression_skips_non_applicable_dses` (verifies Eat,
     Sleep, Mate, etc. are untouched).
   - `flee_boost_and_hunt_damp_compose` — same fixture, both
     modifiers run in pipeline; assert Flee gets +boost and Hunt
     gets ×damp on the same tick.

## Verification

- `just check && just test` — all unit tests pass.
- `just soak-trace 42 Wren` + `just verdict logs/tuned-42`.
  Predictions:
  - Adult Starvation deaths drop to 0 (the regression shape).
  - Courtship events return to non-zero (a cluster of cats stuck in
    fox-scent corridors blocks mating in 223 alone; restoring the
    score-time gate frees them to leave the corridor).
  - `MatingOccurred` / `CourtshipInteraction` /
    `PairingIntentionEmitted` fire (currently silent post-223).
  - ShadowFoxAmbush deaths trend at-or-below the post-223 level
    (cats are more likely to avoid fox territory at decision time).
- `just frame-diff` between post-223 and post-228 focal traces:
  Hunt mean score should DROP in fox-scent buckets (matches
  expectation); Eat / StockpileForage / Move scores should RISE in
  the same buckets.

## Log

- 2026-05-07: opened from 223's verification regression — soak showed
  3 adult Starvation deaths clustered in the [33,21]–[31,22] fox
  corridor + 0 courtship + never-fired Mating canaries. Theory of
  the case: 223's path-cost overlay is route-time substrate; the
  retired damp branch was decision-time substrate. They compose
  orthogonally; 223 alone leaves a gap at the decision layer.
