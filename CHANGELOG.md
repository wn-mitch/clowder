# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/wn-mitch/clowder/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/wn-mitch/clowder/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/wn-mitch/clowder/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/wn-mitch/clowder/releases/tag/v0.1.0
