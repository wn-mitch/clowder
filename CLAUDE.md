# Clowder

Terminal-based colony simulation. Cats in a Redwall/Mausritter-inspired fantasy world.

**Creative inspirations:** Redwall, Mausritter, Dwarf Fortress, Rimworld, Warriors (book series).

## Tech Stack

- **Language:** Rust (2021 edition)
- **Engine:** `bevy` (full — ECS, rendering, windowing, asset loading)
- **Rendering:** Bevy `Sprite` + `TextureAtlas` (see Rendering section below)
- **TUI:** `ratatui` + `crossterm` (headless/legacy mode)
- **RNG:** `rand` + `rand_chacha` (deterministic, seeded)
- **Terrain:** `noise` (Perlin)
- **Template data:** RON files (not used yet — Phase 2)
- **VCS:** `jj` (git-compatible)
- **Task runner:** `just`

## Architecture

Bevy provides ECS, rendering, and windowing. The graphical mode uses Bevy's
renderer with a 2D camera; headless mode still uses ratatui for terminal output.
Simulation ticks and render frames are decoupled.

Key architectural decisions:
- ECS over agent-based: cross-cutting systems (weather, corruption) need to affect
  all spatial entities uniformly. ECS queries handle this naturally.
- Template-driven narrative: text output is data (RON files), not code. Narrative
  richness scales with content, not engineering.
- Utility AI: each cat scores available actions per-tick based on needs, personality,
  relationships, and context. No behavior trees, no LLMs.
- Maslow hierarchy needs: 5 levels (physiological → self-actualization). Lower levels
  suppress higher levels when critical.
- Emergent complexity: when a system can tie into the narrative layer through
  unexpected interaction, it should. Chain reactions between independent systems are
  the joy of the simulation — design for them, not against them.
- **Physical causality:** Objects don't teleport. Cats must physically carry items
  (via Inventory), walk to destinations, and deposit them. Effects require physical
  presence. Despite the world having magic, normal causality applies. Actions are
  behavioral arcs with physical movement — not instant stat changes at a distance.

## Commands

- `just run` — run the simulation
- `just seed <N>` — run with a specific RNG seed
- `just test` — run tests
- `just check` — cargo check + clippy
- `just ci` — all checks

## Conventions

- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:` (no scopes)
- Branch naming: `wnmitch/<feature-name>`
- VCS: use `jj` for all version control, not raw git
- Design docs: `docs/systems/` has one stub per tunable system with parameters and rationale
- Design spec: `~/.claude/plans/structured-napping-candle.md`

## Code Style

- Components are plain structs or enums with `#[derive(Component)]`
- Resources use `#[derive(Resource)]`
- Systems are standalone functions, registered via `app.add_systems()`
- Group related components in `src/components/`, resources in `src/resources/`, systems in `src/systems/`
- Prefer `Query<>` with explicit component access over broad world access
- Keep systems focused — one responsibility per system function

## Headless Mode

The headless `build_schedule()` in `src/main.rs` is a **manual mirror** of
`SimulationPlugin::build()` in `src/plugins/simulation.rs`. When adding or
reordering systems in the graphical plugin, you **must** update `build_schedule()`
to match. The two diverged silently before and caused headless diagnostics to
run stale code paths. Treat them as a pair — change one, change both.

Headless CLI flags for diagnostics:
- `--trace-positions <N>` — emit lightweight per-cat position+action traces every N ticks (1 = per-tick)
- `--snapshot-interval <N>` — control full CatSnapshot + economy event interval (default 100)

## Bevy ECS Guidelines

- **`run_if` over early returns**: if a condition can be expressed as a `run_if` guard on the system, prefer that over an early return inside the system body. Systems gated by `run_if` skip query iteration entirely.
- **Never `.clone()` resource data in a per-tick system.** Borrow via `as_slice()`, reference, or `Res<T>`/`ResMut<T>`. String clones for storage (e.g. copying a name into a log entry) are fine.
- **Events are verbs**: if/when Bevy events are introduced, name them as imperative or past-tense verbs (`SpawnCat`, `CatDied`), not noun-suffix (`DeathEvent`). Define in a central module. No circular event flows.

## Rendering

The tilemap is rendered with plain Bevy sprites — **not** bevy_ecs_tilemap's
`TilemapBundle`. bevy_ecs_tilemap is still a dependency (for `TilePos`,
`TileStorage` types used by the overlay toggle system) but its GPU rendering
pipeline is not used.

- **Base terrain:** One `Sprite` entity per tile with individual 16×16 PNGs
  from `assets/sprites/tiles/`. Positioned at z=0.
- **Autotile overlays:** `Sprite` + `TextureAtlas` referencing the 8×8 blob
  atlas PNGs (grass, soil, stone). Positioned at z=1/2/3 by layer.
- **F6/F7/F8** toggle overlay visibility by querying `BlobOverlayLayer`
  entities and matching their `Transform.translation.z`.

**Do not use `TilemapBundle` for rendering.** bevy_ecs_tilemap v0.18's
`texture_2d_array` pipeline silently renders all tiles as texture index 0 on
macOS Metal. Both the default array-texture path and the `atlas` feature path
fail. This was debugged extensively — the data pipeline is correct at every
step; the bug is in the GPU shader/texture binding layer. When evaluating
tilemap rendering crates in the future, verify with a multi-texture visual
test (render tiles with different indices, assert pixel colors) before
building on top.
