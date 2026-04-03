# Clowder

Terminal-based colony simulation. Cats in a Redwall/Mausritter-inspired fantasy world.

## Tech Stack

- **Language:** Rust (2021 edition)
- **ECS:** `bevy_ecs` (standalone, no renderer)
- **TUI:** `ratatui` + `crossterm`
- **RNG:** `rand` + `rand_chacha` (deterministic, seeded)
- **Terrain:** `noise` (Perlin)
- **Template data:** RON files (not used yet — Phase 2)
- **VCS:** `jj` (git-compatible)
- **Task runner:** `just`

## Architecture

bevy_ecs provides entity-component-system infrastructure. The main loop ticks the
ECS schedule, then renders the terminal UI via ratatui. Simulation ticks and render
frames are decoupled — the TUI renders at ~30fps regardless of sim speed.

Key architectural decisions:
- ECS over agent-based: cross-cutting systems (weather, corruption) need to affect
  all spatial entities uniformly. ECS queries handle this naturally.
- Template-driven narrative: text output is data (RON files), not code. Narrative
  richness scales with content, not engineering.
- Utility AI: each cat scores available actions per-tick based on needs, personality,
  relationships, and context. No behavior trees, no LLMs.
- Maslow hierarchy needs: 5 levels (physiological → self-actualization). Lower levels
  suppress higher levels when critical.

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

- Components are plain structs with `#[derive(Component)]`
- Resources use `#[derive(Resource)]`
- Systems are standalone functions, registered via `app.add_systems()`
- Group related components in `src/components/`, resources in `src/resources/`, systems in `src/systems/`
- Prefer `Query<>` with explicit component access over broad world access
- Keep systems focused — one responsibility per system function
