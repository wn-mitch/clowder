---
id: 492
title: Phase 2b — Euclidean perception + radius constants i32→f32
status: done
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-31
parked: null
blocked-by: []
supersedes: []
related-systems: [project-vision.md, ai-substrate-refactor.md]
related-balance: []
landed-at: 1f6d1b28
landed-on: 2026-06-02
---

## Why

Sub-phase 2b of the 135 continuous-position epic. 491 (sub-phase 2a)
made `Position` a `Vec2<f32>` newtype but preserved every existing
call-site semantic via `manhattan_distance` reads through `pos.tile()`.
This ticket lifts perception from grid Manhattan to world-space
Euclidean — the actual substrate switch the 139 epic was opened for.

Tactical-tie reads — where Manhattan picks the cardinal neighbor and
Euclidean picks the diagonal — will shift Hunt/Forage target
selection. Expect bounded drift; canary gate plus
`just hypothesize` verifies.

## Scope

1. **Retire `manhattan_distance` from sim code.** Method stays on
   `Position` (or moves to a free function) but no `src/` reader calls
   it for perception/pursuit/scoring. Existing 314 call sites in 64
   files migrate to `distance_to` (Euclidean) or `chebyshev_distance`.
2. **Add `chebyshev_distance`** — max of |dx|, |dy| on `pos.tile()`.
   Used for tactical-reach reads ("can this entity strike me this
   tick?") where Manhattan was wrong but Euclidean overshoots.
3. **Radius constants `i32` → `f32` in `src/resources/sim_constants.rs`**
   — e.g. `wildlife_threat_range: i32 = 10` → `f32 = 10.0`. Mechanical
   sweep; update read-sites. Comparisons now `f32 <= f32` instead of
   `i32 <= i32`.
4. **Per-call-site classification.** Audit each migrated call and
   pick Euclidean vs Chebyshev based on intent:
   - Euclidean: perception, pursuit, social spacing, gestalt distance.
   - Chebyshev: one-tick-reachability, "can attack this tick", strict
     tactical-reach reads.

## Out of scope

- **Continuous (sub-tile) positions.** Every `Position` is still
  tile-center (Phase 2a invariant). Vec2 internals are exercised but
  no actual sub-tile state exists until #140 steering.
- **Pathfinding cleanup.** Deferred to sibling 493 (sub-phase 2c).

## Approach

Mechanical sweep of `manhattan_distance` callers + audit. Test
fixtures that used Manhattan-tied positions may need ε-aware
distance asserts.

`Position::distance_to` already exists (returns `f32` via
`Vec2::distance`); 491 added the Vec2-backed inner — this ticket
just routes more callers through it.

## Verification

- `just check` + `just test` green.
- Balance hypothesis (4-artifact methodology, per CLAUDE.md):

  *Predicted: continuity canaries within ±5% (perception range
  preserved; threats/herbs/kittens land in same containing tile under
  Euclidean as under Manhattan). Tactical-tie reads — where
  Manhattan-tied targets pick the cardinal neighbor and Euclidean
  picks the diagonal — shift Hunt/Forage target selection in
  degenerate-tie cases. Survival gates unchanged.*

- `just hypothesize <spec.yaml>` end-to-end. Promote new baseline.

## Log

- 2026-05-31: opened as sub-phase 2b sibling of 491. 491 landed the
  substrate type-switch with zero behavior drift; this ticket lifts
  perception to Euclidean and is the balance-drift step.
- 2026-06-02: subsumed by 494/496/497 cascade — Chebyshev as default, Euclidean as radial escape hatch (inverted vs original Euclidean-first framing); call-sites all retired at 1f6d1b28. The 4-artifact hypothesize step left for a separate balance follow-on if needed.
