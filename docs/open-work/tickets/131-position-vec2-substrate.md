---
id: 131
title: Phase 2 — Position becomes Vec2<f32> (continuous-position substrate migration)
status: ready
cluster: substrate-migration
added: 2026-05-02
parked: null
blocked-by: [127]
supersedes: []
related-systems: [project-vision.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase 2 of the continuous-position migration (epic ticket 127). The big lift: `Position` itself becomes `Vec2<f32>`. Every component, every memory entry, every landmark anchor stores world-space coordinates. Manhattan retires from sim code in favor of Euclidean. `TileMap` stays as the terrain palette + cost field; cats query their containing tile via `(pos.x.floor() as i32, pos.y.floor() as i32)`. Influence maps stay tile-grids. Buildings stay tile-aligned.

This phase is the substrate change that unblocks Phase 3 (#132) steering / smooth pursuit and gives perception its Euclidean intuition.

## Scope

### Type & component changes

1. **`Position` → `Vec2<f32>`.** Either a thin newtype `Position(pub Vec2)` (preserves textual diff in tests; `Position::new(5, 5)` becomes `Position::new(5.0, 5.0)`) or a direct `pub type Position = Vec2;` alias. Recommend the newtype — preserves documentation, lets us add tile-snap helpers (`pos.tile() -> (i32, i32)`).

2. **`PreviousPosition`, `RenderPosition` (from #129) update.** All become `Vec2`-based. Phase-0 interpolation work simplifies — no integer→float conversion at the render seam.

3. **Memory entries.** `MemoryEntry::location: Option<Vec2>`. Save migration snaps existing i32 locations to `Vec2::new(x as f32 + 0.5, y as f32 + 0.5)` (tile center).

4. **`CatAnchorPositions`.** All `Option<Position>` fields become `Option<Vec2>` automatically via the newtype.

### Distance metric retirement

5. **`manhattan_distance` retires from sim code.** Replace every call site with `Vec2::distance` (Euclidean) or a new `chebyshev_distance` helper for tactical-reach reads ("can this entity strike me this tick?"). Both produce a continuous `f32`; downstream comparisons stay against existing radius constants (which become `f32`).

6. **All `i32` radius constants in `sim_constants.rs` migrate to `f32`.** `wildlife_threat_range: i32 = 10` → `f32 = 10.0`. Update read-sites mechanically.

7. **`count_walkable_tiles_in_box` (ticket 103).** Operates on the cat's containing tile — unchanged on the input side; the tile coords come from `pos.tile()`.

### Pathfinding

8. **`step_toward(from, to, map)` retires.** Replaced by `path_toward(from, to, map) -> Option<Vec2>` that returns the next-frame world-space target. v1 implementation: A* over the tile cost grid, returning the next tile center along the path; the steering layer (#132) then steers toward that center.

9. **`find_free_adjacent` and friends.** Stay tile-based, return `(i32, i32)` tile coords; callers convert to `Vec2` via `Vec2::new(tx as f32 + 0.5, ty as f32 + 0.5)`.

### Save / load migration

10. **Save format version bump.** `SAVE_FORMAT_VERSION` increments. Loader detects pre-131 format by version and applies the snap-to-tile-center migration to every Position and MemoryEntry::location.

11. **Migration test.** `tests/save_migration_131.rs` loads a checked-in pre-131 save and asserts post-load positions snap to tile centers; sim runs N ticks without panic.

### Tests

12. **Test churn.** Hundreds of test files reference `Position::new(int, int)` — most need a `0.0 + tile_center` conversion. Mechanical sweep via `sed`-style migration; assertions like `assert_eq!(pos, Position::new(5, 5))` migrate to `assert!((pos - Position::new(5.5, 5.5)).length() < 1e-4)`.

13. **Determinism test.** `tests/deterministic_replay.rs` runs N ticks twice on seed 42 and asserts byte-identical event logs. Catches f32 ordering regressions.

## Files to modify (rough scope, not exhaustive)

- `src/components/physical.rs` — `Position` newtype.
- `src/components/mental.rs` — `MemoryEntry::location`.
- `src/ai/scoring.rs` — `CatAnchorPositions` fields, `ScoringContext` fields touching position math.
- `src/ai/pathfinding.rs` — `path_toward`, `find_free_adjacent`.
- `src/ai/considerations.rs` — `LandmarkSource::Anchor` resolver Euclidean read.
- `src/resources/sim_constants.rs` — radius constants `i32` → `f32`.
- `src/systems/sensing.rs` — Bresenham LoS, sensory range comparisons.
- `src/systems/wildlife.rs` — wildlife AI movement.
- `src/systems/disposition.rs` / `src/systems/goap.rs` — populator distance reads.
- `src/steps/**/*.rs` — every step resolver that takes positional arguments.
- `src/save_load.rs` (or wherever) — version bump + migration.
- `tests/**` — mass position-literal migration.

## Determinism strategy

- Pin Bevy query iteration order in any system that does positional reductions (e.g. nearest-X-by-Euclidean-distance) — sort by entity id before the reduction.
- Single-platform-deterministic is the bar (macOS dev). Cross-platform deterministic replay is out of scope; if Clowder ever ships networked or to other platforms, revisit with fixed-point arithmetic or a deterministic float library.
- Add the deterministic-replay test (item 13 above) to `just ci` so regressions block.

## Verification

- `just check` / `just test` green after the migration sweep.
- `tests/deterministic_replay.rs` green.
- `just soak 42 && just verdict` — expect bounded drift on continuity canaries (perception now Euclidean; tactical reads using Manhattan-vs-Chebyshev may shift). Promote new baseline.
- **Hypothesis (per CLAUDE.md balance methodology):**

  *Position → Vec2 migration with Euclidean perception will:*
  *(a) Leave continuity canaries within ±5% (grooming/play/mentoring/burial/courtship/mythic-texture rates depend on perception RANGE not metric; tile-center snapping preserves containing-tile logic).*
  *(b) Shift "nearest-X" picks at tied Manhattan-distance — Euclidean picks the diagonal neighbor over the cardinal one. Expect occasional small drift on Hunt / Forage targeting in degenerate-tie cases.*
  *(c) Save migration round-trips losslessly for tile-aligned data.*

  Run `just hypothesize` end-to-end. Promote new baseline.

## Out of scope

- **Steering / continuous movement.** Phase 2 still moves cats *to* tile centers; the smoothness comes from Phase 0 interpolation. Phase 3 (#132) introduces actual steering between tile centers.
- **Sub-tile influence maps.** Tile-grid influence maps stay (epic constraint).
- **Building-as-region.** Tile-aligned (epic constraint).
- **Cross-platform determinism.** Single-platform bar.

## Log

- 2026-05-02: Opened as Phase 2 of the 127 continuous-position-migration epic.
