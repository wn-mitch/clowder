# Smooth Movement Interpolation

## Context

Cats and prey currently snap between grid tiles once per simulation tick. At
Normal speed (1 tick/sec), a cat in approach mode jumps 3 tiles (144 px at
default zoom) per second — visually jarring and hard to track. The simulation
pacing and food economy are tuned correctly; the problem is purely visual.

## Approach

**Render-side lerp** between previous and current grid positions using Bevy's
`FixedUpdate` / `Update` split. Zero changes to simulation logic.

Simulation systems run in `FixedUpdate` (1–20 Hz depending on `SimSpeed`).
Rendering runs in `Update` (~60 Hz). Each rendered frame, the sprite system
interpolates between the entity's position at the start of the tick and its
position after the tick, using `Time<Fixed>::overstep_fraction()` as the
interpolation factor.

## Components

### `PreviousPosition`

```rust
// src/components/physical.rs
#[derive(Component, Clone, Copy)]
pub struct PreviousPosition {
    pub x: i32,
    pub y: i32,
}
```

Stored alongside `Position` on every entity that has an `EntitySpriteMarker`.

## Systems

### `snapshot_previous_positions` (FixedUpdate — runs first)

Copies current `Position` into `PreviousPosition` for every entity that has
both. Runs **before** all simulation systems so it captures the pre-tick state.

```
Query<(&Position, &mut PreviousPosition)>
```

### `sync_entity_positions` (Update — modified)

Currently snaps `Transform` to `Position`. Modified to:

1. Read `PreviousPosition` and `Position`.
2. Compute interpolation factor `t = Time<Fixed>::overstep_fraction()`.
3. Compute interpolated world coordinates:
   ```
   visual_x = prev_world_x + (curr_world_x - prev_world_x) * t
   visual_y = prev_world_y + (curr_world_y - prev_world_y) * t
   ```
4. **Snap threshold**: if `manhattan_distance(prev, pos) > 5`, skip lerp and
   snap directly to `Position`. Prevents visual sliding on spawn, teleport, or
   other large repositioning.
5. Add existing deterministic sub-tile jitter offset on top.

### `attach_entity_sprites` (Update — modified)

When first attaching a sprite to an entity, also insert `PreviousPosition`
initialized to the current `Position`. Prevents fly-in-from-origin on spawn.

## Registration

### `RenderingPlugin` (`src/rendering/mod.rs`)

- `sync_entity_positions` query signature changes to include
  `&PreviousPosition` and `Res<Time<Fixed>>`.
- No new systems registered here — the snapshot system belongs in the
  simulation schedule.

### `SimulationPlugin` (`src/plugins/simulation.rs`)

- Register `snapshot_previous_positions` in `FixedUpdate`, ordered **before**
  all existing simulation systems (before `check_anxiety_interrupts`).

## Speed transitions

When `SimSpeed` changes, `Time<Fixed>`'s timestep changes. `overstep_fraction()`
automatically adapts because Bevy recomputes it relative to the current
timestep. No special handling needed.

## Headless mode

Headless mode has no `RenderingPlugin`, so `EntitySpriteMarker` is never
attached, `PreviousPosition` is never inserted, and `sync_entity_positions`
never runs.

`snapshot_previous_positions` runs in `FixedUpdate` for both modes but is a
no-op in headless because no entities have `PreviousPosition`.

## Files modified

| File | Change |
|------|--------|
| `src/components/physical.rs` | Add `PreviousPosition` component |
| `src/rendering/entity_sprites.rs` | Modify `sync_entity_positions` to lerp using `PreviousPosition` + `Time<Fixed>`; modify `attach_entity_sprites` to insert `PreviousPosition` |
| `src/rendering/mod.rs` | Update system signatures (no new systems here) |
| `src/plugins/simulation.rs` | Register `snapshot_previous_positions` in `FixedUpdate` before all sim systems |

## Verification

1. `just run` at Normal speed — cats and prey glide smoothly between tiles.
2. Toggle Fast / VeryFast — still smooth, just faster.
3. Pause (Space) — entities freeze, no jitter.
4. Spawn events (new kittens, prey births) — entities appear at correct
   position, no slide-in artifact.
5. Headless baseline: `cargo run -- --headless --duration 15 --seed 42` —
   `ColonyScore` identical to pre-change run (no simulation behavior changed).
6. Multi-seed (42, 99, 7, 2025, 314) headless — no regressions.
