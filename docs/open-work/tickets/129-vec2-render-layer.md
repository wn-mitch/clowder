---
id: 129
title: Phase 0 — Vec2 render layer (visual interpolation, no sim-state change)
status: ready
cluster: substrate-migration
added: 2026-05-02
parked: null
blocked-by: [127]
supersedes: []
related-systems: [project-vision.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase 0 of the continuous-position migration (epic ticket 127). Lands the **render-side smoothness** without touching sim state. Cats still live on the integer grid (`Position { x: i32, y: i32 }`); the render system reads a `RenderPosition: Vec2<f32>` that interpolates between the previous and current `Position` over the tick. Pure visual win, zero AI / scoring / save / test churn.

Lands first because it's the lowest-risk, highest-visibility phase: shippable in isolation, revertable in isolation, gives the project a smoother feel without committing to the substrate migration.

## Scope

1. **`RenderPosition` component.** `Vec2<f32>`. Inserted on every entity that has `Position`. Authored by a render-layer system that runs in `Update` (not the sim schedule).

2. **Interpolation system.** Per render frame: read `Position` (current sim tile) and `PreviousPosition` (sim tile last tick). Interpolate `RenderPosition` between them based on `tick_progress: f32` (0.0 at tick start, 1.0 at next tick). Use `smoothstep` for ease-in/out.

3. **`PreviousPosition` component.** Authored by a sim-layer system that copies `Position` to `PreviousPosition` at the *start* of every sim tick, before any movement steps run.

4. **Sprite Transform binding.** Existing render path that reads `Position` switches to read `RenderPosition` for `Transform.translation`. Tile texture index and z-layer reads still come from `Position` (containing tile).

5. **Tick-progress resource.** `RenderTickProgress(f32)` resource updated each render frame from `(now - last_tick_at) / tick_duration`. Clamped `[0, 1]`.

## Files to modify

- `src/components/physical.rs` — add `RenderPosition`, `PreviousPosition` components.
- `src/systems/render.rs` (or wherever `Sprite` Transform binding lives) — switch read source.
- `src/plugins/simulation.rs` — register the `previous_position_snapshot` system at sim-tick start; register the `render_position_interpolate` system in `Update`.
- `src/resources/time.rs` — add `RenderTickProgress` resource.

## Verification

- `just check` / `just test` green; no test changes (sim state is untouched).
- `just soak 42 && just verdict` — verdict 0. Footer match required (perception is unchanged).
- **Visual check** — `just run-game`; cats glide between tiles instead of teleporting. Tile transitions feel smooth at default tick rate; rapid-tick speeds (testing UI) still readable.

## Out of scope

- Animation curves, sprite rotation, frame-based animation. Position interpolation only.
- Smoothing across tick *skips* (e.g. when sim runs at 4× speed). v1 just reads `RenderTickProgress` clamped; if rendering can't keep up the sprite snaps at tile boundaries (acceptable degraded behavior).

## Log

- 2026-05-02: Opened as Phase 0 of the 127 continuous-position-migration epic.
