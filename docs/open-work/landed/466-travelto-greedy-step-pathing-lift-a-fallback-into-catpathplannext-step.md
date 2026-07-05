---
id: 466
title: TravelTo greedy-step pathing — lift A* fallback into CatPathPlan::next_step
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-25
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 07acc090
landed-on: 2026-07-05
---

## Why

Ticket 465 landed an inline A* fallback in `resolve_engage_prey`'s
approach loop — when greedy `step_toward` returns `None`, call
`find_path` and take the first step. The same greedy-stuck defect
applies to *every* caller of `step_toward` that isn't through a
`RouteCostField` gradient walk: `CatPathPlan::next_step`'s
`AStarFallback` and `NoOverlay` arms both delegate to greedy
`step_toward` (`src/ai/route_cost.rs:266-274`) — the "A* fallback"
name describes only the *overlay* shape, not the *routing* shape.

Post-465 soak (`logs/tuned-42-59e26d68`) shows `TravelTo(HerbPatch): no
path and stuck` at 452 events (up from 278 baseline pre-fix; the
colony travels more now that hunts complete faster, so the same
defect surfaces more frequently in a different consumer). The
substrate-correct fix is to lift the inline A*-fallback pattern from
465's `goap.rs:9523-9538` into `CatPathPlan::next_step` so every caller
benefits — TravelTo, PatrolTo, FleeTravel, ApplyRemedy, Tend, Repair,
Construct, PickupMaterial, MoveTo (the ten existing callers at
`src/steps/`).

## Scope

- Modify `CatPathPlan::next_step` (`src/ai/route_cost.rs:263-275`):
  in the `AStarFallback` and `NoOverlay` arms, when `step_toward`
  returns `None`, fall back to `find_path` and return the first step.
- Verify with `just soak-trace 42 Simba`: `TravelTo(*): no path and
  stuck` events should drop comparable to the hunt-approach drop.
- Verify no regression on the `Field` arm (already gradient-walks).
- Balance-doc entry if welfare metrics shift > ±10%.

## Out of scope

- Adding A* path caching on the per-cat plan struct (premature
  optimization; 465's inline pattern shows per-tick A* on stuck-trigger
  ticks is affordable).
- Per-personality pathfinder profiles.
- Replacing greedy `step_toward` entirely (the rustdoc says it's
  "intentionally simple"; the fallback shape preserves the cheap path
  and adds A* only at the trap-trigger ticks).

## Current state

Blocked on 465 landing (which establishes the inline pattern and
demonstrates the soak-level safety).

## Approach

1. Land 465 first (this ticket's `blocked-by` clears via `just land`).
2. Single-file edit at `src/ai/route_cost.rs:263-275`. Mirror the
   shape from `goap.rs:9529-9531`:
   ```rust
   step_toward(&from, &to, map, &overlays).or_else(|| {
       find_path(from, to, map, &overlays).and_then(|p| p.into_iter().next())
   })
   ```
3. Same for the `NoOverlay` arm (with `&[]` overlays).
4. `just check` + `just test` + `just soak-trace 42 Simba` + `just
   verdict` + `just q footer logs/tuned-42-<sha> --field=plan_failures_by_reason --top-keys=10`
   to confirm TravelTo stuck events drop.

## Verification

- `just q footer logs/tuned-42-<sha> --field=plan_failures_by_reason
  --top-keys=10`: `TravelTo(*): no path and stuck` ≤ 100 (down from 452
  post-465).
- `just verdict logs/tuned-42-<sha>` survival/never-fired gates pass.
- Welfare drift bands documented if > ±10%.

## Log

- 2026-05-25: opened from 465's outcome. Same greedy-stuck defect at a
  different consumer; substrate-correct generalization of 465's inline
  fix.
- 2026-07-05: closed as superseded by 493 (the step_toward A*-lift landed there; carried its gate: TravelTo no-path-and-stuck <= 100 — post-493 soak plan_failures show HoldUntilSafe timeout 144 but no TravelTo stuck-gate breach)
