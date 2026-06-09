# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-06-09

### Added
- Three-layer AI substrate (epic 060, phases 1–6): L1 influence maps + ECS marker registry, L2 IAUS DSE scoring with a unified trace-visible modifier pipeline, L3 softmax-over-Intentions commitment feeding the GOAP/HTN planner; focal-cat L2/L3 trace sidecar with a locked score-parity invariant
- Belief layer: per-cat mental-model facets replacing raw colony rollups — shelter as housing-security belief, prey-yield beliefs (HuntingPriors retired), predator-ambush beliefs (RecentAmbushMap retired), colony-reserves belief for anticipatory crafting
- HTN methods and multi-step arcs: rear_kitten (wean / teach / release), mourn_at_grave, courtship; JointIntention practices (courtship, play bouts) with witnessable cues
- Items-are-real doctrine: Source/Transfer/Sink gates on every item state transition plus a strict linter; anatomical slot inventory (worn slots + carry pouch), warrior's-kit crafting chain, Workshop recipes, equipment-modifier aggregation surfaced in the L2 trace
- Kitten substrate: sub-stage markers, BegForFood, kittens electing actions at L3 through the canonical dispatch path
- Hunting and wildlife depth: tremor influence map with Stalk/Pounce, A* fallback for concave-trap escapes, per-species MovementBudget, escape-viability mobility differential
- Body zones: WoundKind axis, Festering substrate, injury-attributed deaths
- Environment-quality five-axis influence maps; ward-siege fear map
- Tooling: `just verdict` one-call run validation (canaries + drift + colony-score channel), `just hypothesize` four-artifact balance runs, DuckDB log analytics with charts, flamegraph recipe, foreman/polecat parallel-session orchestration

### Changed
- `Position` is a Vec2-backed newtype; distance semantics migrated Manhattan → Euclidean/Chebyshev across all call sites (radial perception split from tactical reach)
- Founder relationships initialize with a warm floor (familiarity/fondness bands + social-warmth spawn floor)
- GOAP goal encoding refactored to a `GoalKind` sum type with substrate decomposition helpers

### Performance
- Event-driven retirements of per-tick hot paths: passive familiarity behind a CatMoved cache, lazy sustained-copresence eviction, near-pair cache eviction on CatDied, scratch-buffer allocation reuse

### Fixed
- Inter-cat Mates-bond exclusivity gap
- CarcassPile picker no longer targets impassable tiles
- Sprite asset path resolution; materials atlas rebind
- logdb ingest: 64-bit seeds, unreadable files skipped instead of aborting

### Known issues
- Per-tick throughput regressed ~63% over the substrate-refactor landing window; tracked as perf epic 480 (top remaining hot path: `author_joint_intentions`)
- Early-game founder spatial dispersion collapses ("cuddle puddle"); diagnosed in ticket 490 with fix direction chosen

## [0.2.0] - 2026-04-19

### Added
- GOAP planner replacing disposition-chain execution for cat AI (`src/ai/planner/`, `src/systems/goap.rs`)
- Fox GOAP planner with species-specific scoring (`src/ai/fox_planner/`, `src/systems/fox_goap.rs`)
- Four-channel sensory model (sight, hearing, scent, tremor) with terrain-aware propagation (`src/systems/sensing.rs`, `docs/systems/sensory.md`)
- Construction pipeline: Stores is now the colony's first real construction project rather than world-gen auto-spawn; ConstructionSite entities with material delivery (`src/components/coordination.rs`, `src/steps/building/construct.rs`)
- Cooking loop (Kitchen, Cook action, raw-food retrieval) scaffolding (`src/steps/disposition/cook.rs`, `src/steps/disposition/retrieve_raw_food_from_stores.rs`)
- Ward and spirit-communion step resolvers for the magic pipeline (`src/steps/magic/set_ward.rs`, `src/steps/magic/spirit_communion.rs`)
- Wildlife system with foxes, hawks, snakes, and corruption-spawned shadowfoxes (`src/systems/wildlife.rs`, `src/components/wildlife.rs`)
- Wind resource with directional scent propagation (`src/resources/wind.rs`, `src/systems/wind.rs`)
- Zodiac personality axis and component (`src/resources/zodiac.rs`, `src/components/zodiac.rs`)
- Prey species profiles (mouse, rat, rabbit, fish, bird) with per-species `PreyProfile` trait and stealth-first hunt model
- `ForcedConditions` resource + `--force-weather` CLI flag for reproducible weather-activation sweeps
- `build.rs` emitting commit metadata into event-log headers (seed, duration, commit hash/short/dirty/time, `SimConstants` dump)
- Balance telemetry: `SystemActivation`, `ColonyScore`, `FoodLevel`, `PopulationSnapshot`, `CatSnapshot` periodic events
- Weather VFX rendering (`src/rendering/weather_vfx.rs`)
- Sprite animation system (`src/rendering/sprite_animation.rs`)
- Scripts: `score_track.py`, `score_diff.py`, `sweep_compare.py`, `analyze_eat_threshold.py`, `check_canaries.sh`, `generate_wiki.py`
- Narrative editor: logs dashboard page with run switcher (`tools/narrative-editor/src/pages/LogsDashboard.svelte`)
- New narrative template files: banishment, caretake, cook, mate

### Changed
- Project thesis reframed as "a clowder of cats living in a world with its own weight" (`docs/systems/project-vision.md`, `CLAUDE.md`)
- `CLAUDE.md` adds long-horizon coordination rules (`docs/open-work.md`, `docs/wiki/systems.md`, `docs/balance/*.md` as the three thread indexes) and expanded ECS/verification guidance
- Disposition chain dispatch rerouted through GOAP; old direct-dispatch path retained only where structurally required
- Balance tuning: eat-from-inventory threshold 0.05 → 0.4; forage yield 0.25 → 0.30; sleep energy recovery +75%; shadow-fox spawn threshold 0.7 → 0.85; cooking_pressure_multiplier introduced
- Tile/sprite atlases regenerated (base terrain, grass, soil, stone) with new rune-rock atlas
- Expanded narrative templates across build, flee, forage, forage_find, groom, wander

### Removed
- `docs/systems/sleep-cycle.md` (superseded by `docs/systems/sleep-that-makes-sense.md`)
- `docs/systems/sprite-pass.md` (landed; no longer an open thread)

### Docs
- `docs/open-work.md` as the canonical cross-session backlog
- `docs/balance/` iteration logs: unified difficulty posture, Activation 1 (fog sight), eat-inventory-threshold, fox phase 2a
- `docs/systems/`: project vision, sensory, strategist coordinator, sleep-that-makes-sense, log-analytics-dashboard
- `docs/diagnostics/log-queries.md` with jq recipes for `events.jsonl` / `narrative.jsonl`
- `docs/missing-sprites.md` tracking placeholder vs real-art sprites

### Known issues
- `cargo test` fails three integration tests (`cats_eat_when_hungry`, `simulation_is_deterministic`, `simulation_runs_1000_ticks_without_panic`) with a Bevy "Resource does not exist" panic. A system added to `build_schedule()` is missing its resource in `tests/integration.rs::setup_world`. Tracked in `docs/open-work.md`. `just check` (cargo check + clippy) passes green.

## [0.1.2] - 2026-04-16

### Added
- Writer's Toolkit web app (Svelte 5 + Tailwind) for non-technical narrative contributors
  - Template editor with form-based UI, live preview, and RON import/export
  - Coverage heatmaps showing gaps across action, mood, weather, and other axes
  - Cat personality questionnaire (ported from standalone HTML)
  - Auto-loads `.ron` files from GitHub — no upload required
  - GitHub Pages deployment at wn-mitch.github.io/clowder
- Play narrative templates (social and solo play events)
- Prey and wildlife sprites
- New sprite assets (animal animations, tileset expansions)

### Changed
- Expanded narrative templates across explore, groom, hunt, idle, patrol, sleep, socialize, and wander actions
- GOAP planner and coordination system improvements

## [0.1.1] - 2026-04-15

### Fixed
- Crepuscular sleep model starvation death spiral
- Clippy warnings for CI

## [0.1.0] - 2026-04-10

### Added
- Colony simulation with procedurally generated terrain and starting cats
- Utility AI with Maslow hierarchy needs (physiological through self-actualization)
- Bevy 2D rendering with sprite-based tilemap and autotile overlays
- Personality system: 18-axis traits (drives, temperament, values) plus zodiac signs
- Social bonds, relationship decay, and personality-driven friction
- Weather system with seasonal transitions
- Magic and corruption systems with misfire mechanics
- Wildlife ecosystem with predators, prey, and herbs
- Combat with injury and morale systems
- Narrative template system (RON-based) with mood/weather/season variants
- Cat aspirations, fate connections, and disposition arcs
- Task chain system for multi-step sequential actions
- Building construction with resource requirements
- Day/night cycle with ambient lighting
- Save/load persistence (autosave)
- Headless simulation mode with event logging
- Random seed on boot; use `--seed N` to reproduce a specific world

[unreleased]: https://github.com/wn-mitch/clowder/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/wn-mitch/clowder/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/wn-mitch/clowder/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/wn-mitch/clowder/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/wn-mitch/clowder/releases/tag/v0.1.0
