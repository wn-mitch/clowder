---
id: 493
title: Phase 2c — step_toward retires to find_path wrapper (pathfinding cleanup)
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
landed-at: 07acc090
landed-on: 2026-07-05
---

## Why

Sub-phase 2c of the 135 continuous-position epic. After 491
(substrate switch) and 492 (Euclidean perception), the pathfinding
layer still uses the legacy greedy `step_toward` for per-tick
adjacency. This ticket cleans up the API into the shape #140 (Phase 3
steering) will consume.

A* (`find_path`) already exists at `src/ai/pathfinding.rs:248`. This
ticket doesn't *invent* A* — it makes `step_toward` a thin wrapper
that returns the next step along the planned route (with the existing
greedy-toward fallback preserved for concave-terrain
local-minimum cases — see existing comment in `src/systems/goap.rs`).

## Scope

1. **`step_toward` becomes a thin wrapper over `find_path`** —
   returns the next tile center along the route. Existing greedy
   diagonal-toward fallback preserved for concave-terrain
   local-minimum cases.
2. **Function signature stays** so caller compatibility is
   preserved. Internal CPU profile shifts modestly (A* over a small
   frontier instead of O(1) greedy).
3. **`find_free_adjacent`** stays tile-grid-based; signature unchanged.

## Out of scope

- **Steering / continuous movement.** Cats still move to tile centers
  per-tick; smoothness comes from #137 (already landed). Phase 3 (#140)
  introduces actual steering between tile centers.
- **Flow-field pathfinding.** Sub-tile influence-map kernels are an
  epic-level constraint and out of scope here.

## Approach

The existing `step_toward` in `src/ai/pathfinding.rs` is greedy:
takes the next single step toward the goal, falling back through
diagonal → cardinal candidates. The new shape consults `find_path`
once per call, returns the first step of the returned route, and
falls back to greedy only when A* returns `None`.

Caller sites in `src/systems/disposition.rs` and `src/systems/goap.rs`
are signature-compatible; no call-site changes needed.

## Verification

- `just check` + `just test` green.
- `just soak 42 && just verdict` — expect zero functional drift
  (greedy fallback semantics preserved); modest CPU-budget shift
  acceptable for 80×60 maps with ~30 cats. Watch wall-clock budget.

## Log

- 2026-05-31: opened as sub-phase 2c sibling of 491. Pathfinding
  cleanup precursor to #140 (Phase 3 steering).
- 2026-07-05: A*-first wrapper landed (greedy fallback retained; 466 gate carried). Verdict pass (+12.6% tps, canaries green) with hypothesis-carried drift: route concentration exposed threat-blind routing — 6 hotspot deaths, kittens 0; balance doc docs/balance/astar-first-stepping.md; structural follow-on 508 (ThreatBeliefOverlay) lands next, before step 6
