---
id: 425
title: update_mentoring_target_markers retire O(N^2) snapshot scan use CatSpatialIndex
status: blocked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: [205]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Sibling perf hotspot to 423 / 205, surfaced by the 2026-05-19 O(N²)
population-scaling survey. `src/systems/aspirations.rs::
update_mentoring_target_markers` (lines 795-861) builds a per-tick
`snapshot: Vec<(Entity, Position, [f32; 6])>` of every living cat, then
iterates each cat against that snapshot with `.any()` to check whether
any other observable cat has a skill below `mentor_skill_threshold_low`
on an axis the focal cat exceeds `mentor_skill_threshold_high` on. That's
**O(N²) per tick** in the marker-author chain, scaling quadratically with
colony size — exactly the smell the user flagged as bad for "Dwarf
Fortress sized sims."

Concretely at N=30 the inner-loop count is ~900 per tick; at N=200 it
becomes 40,000 per tick. Like the cover-disc scan 423 retired, this is
work whose answer changes slowly (skills increment in tiny per-tick
deltas) but is recomputed exhaustively every tick.

## Scope

- **Consume** the `CatSpatialIndex` substrate that ticket 205 introduces
  (per-tick bucket grid over cat positions). Replace `snapshot.iter()
  .any()` with `cat_spatial_index.query_within(pos, detection_range)`
  filtered by `observer_sees_at` + the skill-axis predicate. Inner cost
  drops from O(N) to O(k) where k = neighbors in adjacent buckets
  (typically ≤ 5 at 30 cats).
- **Pre-compute** the per-cat skill array `HashMap<Entity, [f32; 6]>`
  once per tick (instead of materializing the `[f32; 6]` per cat per
  inner-iter). Bundle the snapshot rebuild into a shared SystemParam if
  multiple authors end up consuming it.
- Preserve the existing predicate semantics exactly: any cat the focal
  observes that has at least one axis below `mentor_skill_threshold_low`
  where the focal exceeds `mentor_skill_threshold_high` ⇒ marker stays.
  This ticket is a **performance refactor**, not a balance change.

## Out of scope

- Tuning `mentoring_detection_range` / `mentor_skill_threshold_*` —
  owned by the mentoring balance thread, not this ticket.
- Event-driven `HasMentoringTarget` authoring (v2). Requires a
  bucket-transition substrate (`CatBucketTransitioned` Message) that
  doesn't exist yet. Captured as a follow-on in the 205 plan's
  "Event-driven v2 follow-ons" section.

## Current state

Blocked-by ticket 205 (CatSpatialIndex substrate). 205's first attempt
on 2026-05-19 produced a 1.78× regression and was reverted; the
substrate doesn't exist yet. When 205 lands (or is replaced by an
alternative caching strategy that surfaces a spatial query), unblock
this ticket and proceed.

If 205 is restructured so `CatSpatialIndex` doesn't exist by name,
update this ticket's `## Scope` to reference whatever spatial-query
substrate replaces it.

## Approach

1. Verify `CatSpatialIndex` is wired into `SimulationPlugin::build` and
   registered to rebuild **before** Chain 2a marker authors. Currently
   `update_mentoring_target_markers` lives in Chain 2a alongside the
   sensing batch — the spatial index must rebuild upstream of it.
2. Refactor the system signature: drop the `snapshot: Vec<...>`
   construction; add `Res<CatSpatialIndex>` (and the per-cat skill cache
   resource, if introduced). Mirror the cat-iter query filter
   (`With<Species>, Without<Dead>`).
3. Replace the inner `snapshot.iter().any(|(other, other_pos, other_arr)| ...)`
   with `cat_spatial_index.query_within(*pos, detection_range as i32)
   .filter(|(other, _)| *other != entity)
   .any(|(other, other_pos)| { observer_sees_at(...) && skill-axis-check })`.
4. The skill-axis check still needs `other`'s skill array. Two paths:
   (a) read from a per-cat `HashMap<Entity, [f32; 6]>` resource built
   once per tick by a sibling system, or (b) use a `cats_skills: Query<&Skills>`
   read-only query with `.get(other)`. Path (b) is simpler if Bevy
   allows the aliased read; path (a) avoids the cross-cat query
   construction cost (see ticket 205's regression suspect #1).

## Verification

- `just check` passes (substrate-stub + InfluenceMap registry lints).
- `just soak-trace 42 Simba` → `just verdict`. **Semantic preservation
  pass-bar:** `actions.Mentor.fraction` and `HasMentoringTarget` toggle
  count match the post-205 baseline.
- `just frame-diff <pre> <post> trace-Simba.jsonl` — `mentor_target` DSE
  row mean-score delta < 1e-4 (numerical equivalence modulo float-add
  ordering inside the spatial-index iteration).
- Tick-rate sanity: `elapsed_ticks` should match or exceed the post-205
  baseline (the refactor removes O(N²) work; nothing should regress).

## Log

- 2026-05-19: opened mid-session as a follow-on to 423's perf survey.
  Identified as the second-biggest O(N²) population-scaling smell after
  ticket 205's social_status_distress.
