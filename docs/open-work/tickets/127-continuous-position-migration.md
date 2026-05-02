---
id: 127
title: Continuous-position migration — epic (Vec2<f32> substrate, smooth motion, species speed)
status: ready
cluster: substrate-migration
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [project-vision.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Clowder today is a discrete-grid sim: every `Position` is `(i32, i32)`, every move is one cardinal/diagonal step per tick, and "speed" is therefore a binary "moved or didn't." This was the right substrate to bootstrap the AI / scoring / save / render layers (cheap pathfinding, deterministic, simple save format), but the discrete-grid contract is starting to limit primitives we now want:

- **Speed differentiation** — ticket 103's `escape_viability` mobility term was punted because every entity moves at exactly 1 tile/tick. The grid forbids "the snake is half as fast" without ad-hoc cooldown shims.
- **Render texture** — cats teleport tile-to-tile every tick. Smooth motion is currently impossible without a render-only interpolation layer that diverges from sim state.
- **Continuous behaviors** — pursuit dynamics, flee arcs, social spacing, courtship dance. Each of these is awkward to express on a discrete grid (Manhattan distance ≠ Euclidean intuition; diagonal pursuit aliases).

This epic plans the migration from "discrete grid" to **shape 2 of the work-103 thinking exercise**: continuous `Vec2<f32>` positions over a discrete terrain palette. Tiles remain the terrain semantic ("Grass", "Wall", "Den", "FairyRing") and the cost field for pathfinding; cats live in world space and interpolate between tile centers as they move. Influence maps (`KittenUrgencyMap`, `FoxScentMap`, `ExplorationMap`) stay as tile grids. Buildings stay tile-aligned. Memory entries store world-space points. The render gets smoothness; the AI substrate gets speed primitives; the perception layer gains Euclidean intuition.

## Why an epic

The grid isn't a layer — it's a vocabulary woven through perception, scoring, memory, save format, and tests. Migrating it in one shot would create a multi-thousand-line PR with weeks of integration risk. Splitting into four phases lets each ship independently, each phase deliver a visible win, and each phase be revertable in isolation:

- **Phase 0 (#129)** — `Vec2<f32>` *render layer* alongside integer `Position`. Cats interpolate visually; sim state unchanged. Pure render improvement.
- **Phase 1 (#130)** — per-entity `MovementBudget` for speed differentiation. Re-enables `escape_viability`'s mobility term. Sim still on integer grid; budget gates per-tick step opportunity. Independent of Phase 0.
- **Phase 2 (#131)** — `Position` itself becomes `Vec2<f32>`. Manhattan → Euclidean (with Chebyshev for tactical-reach reads). Pathfinding becomes A*/flow-field over the tile cost grid. Save migration with version bump. The big lift.
- **Phase 3 (#132)** — steering, avoidance, smooth pursuit / flee curves. Cleanup pass on top of the f32 substrate.

Phases ship in any order Phase 0 / Phase 1 are mutually independent; Phase 2 should land before Phase 3 but can land before or after Phases 0/1. Recommendation: 0 first (visible win, low risk), then 1 (gameplay payoff via 103's punted term), then 2 (the big lift), then 3 (polish).

## Design constraints (apply across all phases)

1. **Tile semantics survive.** `Terrain` enum, `TileMap`, `is_passable`, `occludes_sight`, `shelter_value`, `tremor_transmission`, `foraging_yield` all stay. The grid is a palette + cost field, not a positioning system.

2. **Influence maps stay tile-grids.** `KittenUrgencyMap`, `FoxScentMap`, `ExplorationMap`, corruption smell radius, ward coverage maps — all dense tile grids today. Keep them. Cats query their *containing tile* via `(pos.x.floor() as i32, pos.y.floor() as i32)`. Migrating to point-cloud kernels is out of scope.

3. **Buildings stay tile-aligned.** Structures occupy integer tile cells. Transitioning to bounded polygon regions is a separate epic (probably never — narrative readability of "the kitchen is at (12, 7)" is load-bearing).

4. **Memory entries become world-space.** `MemoryEntry::location: Option<Vec2<f32>>`. Save migration snaps existing i32 entries to `f32 + 0.5` (tile center).

5. **Anchors become world-space.** `LandmarkAnchor::*` resolves to `Option<Vec2<f32>>`; the spatial-consideration `LandmarkSource::Anchor` reads world-space distance.

6. **Determinism.** Single-platform-deterministic (macOS dev, fixed iteration order) is sufficient for now. Cross-platform determinism is out of scope; if Clowder ever ships networked or to other platforms, revisit with fixed-point arithmetic.

7. **Distance metric.** Euclidean for "how close" reads (perception, pursuit, social spacing). Keep an explicit Chebyshev helper for tactical reads ("can I be hit this tick?"). Manhattan retires from sim code; tests get a `Vec2::distance` helper to replace `manhattan_distance`.

8. **Save format.** Bumps version. One-shot loader migrates pre-127 saves by snapping integer Position to `Vec2::new(x as f32 + 0.5, y as f32 + 0.5)`. Memory entries the same.

## Out of scope (across the whole epic)

- **Sub-tile influence maps.** Tile-grid influence maps stay. Continuous-domain kernels are out.
- **Building-as-region.** Tile-aligned buildings stay.
- **Cross-platform determinism.** Single-platform-deterministic sim is the current bar.
- **Behavioral richness on top of steering.** Pursuit AI improvements (flank, cut-off, formation) are downstream of this epic and not bundled.
- **Render polish beyond smooth interpolation.** Sprite scaling, animation curves, rotation — separate scope.
- **Rendering swap from `Sprite` to `bevy_ecs_tilemap`.** Pinned by CLAUDE.md to plain `Sprite` because of the macOS Metal pipeline issue with `TilemapBundle`. Out of scope.

## Hypothesis register

Each phase carries its own balance hypothesis. Aggregating across the epic:

- **Phase 0 (visual only)** — no expected sim-behavior drift. Verdict gate: footer match.
- **Phase 1 (cadence)** — *predicted: Snake-driven injuries drop 30–50% (cats outpace slow snakes); ShadowFox ambush deaths unchanged in v1 (shadow-fox cadence unchanged pending burst-ability ticket). Cat-side flee earlier when mobility advantage exists.*
- **Phase 2 (Vec2 substrate)** — *predicted: continuity canaries unchanged within ±5% (perception now Euclidean, but tile-aligned threats / herbs / kittens land in the same containing tiles). Some tactical-reach reads may shift if Manhattan-vs-Euclidean rounding bites; canary gate flags this.*
- **Phase 3 (steering)** — *predicted: chase / flee paths become measurably curvier; ambush success rates may shift as targets execute smoother evasion. Re-baseline.*

Each phase MUST land its own four-artifact balance check (`just hypothesize <spec.yaml>`) before promotion.

## Risks

- **Test churn.** Hundreds of tests assume integer positions. Migration is mechanical (`Position::new(5, 5)` → `Vec2::new(5.0, 5.0)`) but volume is real. Mitigation: a `Position` type alias to `Vec2<f32>` in Phase 2 minimizes textual diff; tests that did `assert_eq!` on positions migrate to `assert!((p - expected).length() < ε)`.
- **Save format break.** Pre-127 saves don't load on post-127 binaries without the migration loader. Mitigation: version-bump in Phase 2; loader is small and well-tested in isolation.
- **Determinism regressions.** f32 arithmetic order matters. Mitigation: pin Bevy query iteration order via sorted-by-Entity-id traversal in any system that does positional reductions; test with N-tick deterministic-replay assertions.
- **Pathfinding cost.** Phase 2 introduces A* / flow-field. Today's `step_toward` is O(1); A* is O(tiles in frontier). For 80×60 maps with ~30 cats, this is fine; verify with `just soak` wall-clock budget.

## Log

- 2026-05-02: Opened as a deferred follow-on of work 103 (escape_viability mobility-term punt). Originally drafted as a small "per-species cooldowns" ticket; reframed via user feedback into the epic-shape continuous-position migration. Phase tickets 129/130/131/132 opened in the same commit.
