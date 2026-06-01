---
id: 491
title: Phase 2a — Position becomes Vec2-backed newtype (substrate switch, no behavior drift)
status: ready
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-31
parked: null
blocked-by: []
supersedes: []
related-systems: [project-vision.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Sub-phase 2a of the 135 continuous-position epic. The parent ticket (139) is
correctly scoped *by content* but too large *by landing unit* — 1,315
`Position::new(` call sites, 314 `manhattan_distance` calls across 64 files,
216 src/ files touching `Position`. The epic was already split into Phases
0–3 to avoid multi-thousand-line PRs; the same logic applies inside Phase 2.

This ticket does *only* the type-system switch: `Position` becomes a
`Vec2<f32>`-backed newtype. All existing call semantics survive — i.e.
this is **zero observable behavior change**. `manhattan_distance` and
`step_toward` keep their integer-grid semantics by reading `pos.tile()`
under the hood. The Euclidean perception switch (former scope items
5–6 of 139) is deferred to 491-sibling-b; the pathfinding cleanup
(former scope items 8) to 491-sibling-c.

The forcing function for this slice is type compatibility:
- `Vec2<f32>` cannot derive `Eq` / `Hash`, so 15 `HashMap<Position, _>`
  sites must migrate.
- `CatSnapshot.position` deserializes pre-491 saves as `{x:i32, y:i32}`,
  so a `SavedPosition` shim is required this PR.
- The deterministic-replay test wires here as the canary for f32 ordering
  regressions across all three sub-phases.

## Scope

### Type change

1. `Position` becomes `pub struct Position(pub Vec2)` (newtype, not type
   alias — keeps method ergonomics and lets us add `pos.tile()`).
2. `Position::new(x: i32, y: i32)` returns `Position(Vec2::new(x as f32
   + 0.5, y as f32 + 0.5))` (tile center). API unchanged for all 1,315
   existing call sites.
3. `Position::tile() -> (i32, i32)` helper returning
   `(self.0.x.floor() as i32, self.0.y.floor() as i32)`.
4. `manhattan_distance` and `distance_to` survive as methods, reading
   `self.tile()` for grid math.
5. Drop `Eq` and `Hash` derives. Keep `PartialEq` (float compare on the
   inner `Vec2`).

### Hash/Eq audit (15 sites)

6. Every `HashMap<Position, _>` / `HashSet<Position>` migrates to
   `HashMap<(i32, i32), _>` keyed via `pos.tile()`. All 15 sites are
   tile-count maps (cats-per-tile or occupied-tile sets), so this is
   semantically a no-op:
   - `src/systems/disposition.rs` (3 sites)
   - `src/systems/goap.rs` (5 sites)
   - `src/systems/task_chains.rs` (1 site)
   - `src/systems/cat_movement.rs` (`Local<HashMap<Entity, Position>>`
     — stays keyed on Entity, value migrates trivially)
   - `src/steps/building/move_to.rs` (2 sites)
   - `src/steps/disposition/patrol_to.rs` (2 sites)
   - `src/systems/colony_knowledge.rs` (memory-group tuple key)
   - `src/ai/pathfinding.rs` (`find_free_adjacent` signature; test
     fixtures)

### Save format

7. `src/persistence.rs`: introduce
   `#[derive(Serialize, Deserialize)] pub struct SavedPosition { x: i32,
   y: i32 }` with `From<Position> for SavedPosition` (writes `pos.tile()`)
   and `From<SavedPosition> for Position` (snaps to tile center).
   `CatSnapshot.position`, `BuildingSnapshot.position`,
   `SavedMemoryEntry.location` use `SavedPosition`. Pre-491 saves
   deserialize losslessly (same wire shape).

### Determinism

8. `tests/deterministic_replay.rs`: spawn the seed-42 scenario, run
   N=2000 ticks twice, assert byte-identical `events.jsonl`. Catches f32
   ordering regressions.
9. Audit positional reductions for iteration-order dependency:
   `rg "min_by|max_by|sort_by.*distance" src/systems/ src/ai/`. Pin
   reductions by `Entity` id where they aren't already.

## Out of scope (deferred to 491 siblings)

- **`manhattan_distance` → Euclidean.** Sibling-b (former 139 scope §5).
- **Radius constants `i32 → f32`.** Sibling-b (former 139 scope §6).
- **`step_toward` → `find_path` wrapper.** Sibling-c (former 139 scope §8).
- **Steering / continuous movement.** #140.
- **Sub-tile influence maps.** Epic constraint.

## Approach

The behavior-preservation invariant: every existing call site that does
`pos.x` / `pos.y` (i32) must produce identical results before and after.
Implementation pattern: wherever `pos.x` / `pos.y` were read as `i32`,
the migration reads `pos.tile()`. The newtype's `Deref<Target = Vec2>`
gives us `pos.length()`, `pos.distance(...)`, etc. for the future
Euclidean work, but **no call site in this PR uses them**.

The save-format shim follows the additive-evolution pattern that
tickets 017 and 095 established — no `SAVE_FORMAT_VERSION` constant
introduced; we just route through `SavedPosition` whose wire format is
the pre-491 shape.

## Verification

- `just check` and `just test` green.
- `tests/deterministic_replay.rs` green twice in a row (catches
  iteration-order regressions).
- `just soak 42 && just verdict <run>` — expect **zero drift** against
  current baseline. Survival gates pass, continuity canaries within
  noise floor, footer line emits.
- Pre-491 save round-trips: spawn → save → reload → spawn → save →
  diff. Should be byte-identical post-shim.

No `just hypothesize` needed — this slice is substrate-only with no
perception change.

## Log

- 2026-05-31: opened as sub-phase 2a of 135-epic / 139-parent. Slicing
  rationale: 1,315 Position::new sites + 314 manhattan calls in 64 files
  make 139 a multi-thousand-line PR; this sub-phase is the
  type-substitution-only slice.
- 2026-05-31: implementation landed at 7ba5a40d. Migration touched
  77 files (+1139, -899). 443 `pos.x`/`pos.y` field reads converted
  to method calls via mass sed; 28 write sites manually rewritten to
  `set_tile(...)`. Hash/Eq/PartialEq keyed on `tile()` to preserve
  HashMap<Position> sites — 15 sites untouched.
- 2026-05-31: verification — 2594 lib tests pass; 1 pre-existing
  test failure (`picking_up_scavenging`) confirmed failing on main
  too, not a regression. Persistence round-trip tests pass (6/6).
  `just check`: 9/10 sub-checks pass; `check_influence_map_registry`
  flaky (passes ~25-50% per memory); `check_orchestration_frontmatter`
  fails on pre-existing ticket 490. `just soak 42 && just verdict`:
  verdict=concern, survival=pass, continuity=pass. Footer drift
  attributable to the 32 upstream commits between baseline
  (`post-482-source-promotions`) and main (MovementBudget gating
  affecting fox/ward behavior, warm-floor founder fix, etc.) — not
  to the substrate switch.
