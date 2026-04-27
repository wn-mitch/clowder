---
id: 052
title: §L2.10.7 plan-cost feedback — `SpatialConsideration` curves on spatially-sensitive DSEs
status: ready
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
