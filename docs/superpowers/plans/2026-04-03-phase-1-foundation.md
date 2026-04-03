# Phase 1: Foundation & Survival Loop

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A running terminal application where 6-10 generated cats autonomously eat, sleep, and move around a noise-generated tile map, driven by a Maslow-hierarchy needs system and basic utility AI. A TUI displays the map, a narrative log, and colony status with configurable simulation speed.

**Architecture:** bevy_ecs standalone (no renderer) provides entity-component-system infrastructure. ratatui + crossterm renders the terminal UI. The main loop ticks the ECS schedule, then renders the TUI at ~30fps. Simulation ticks and render frames are decoupled via a configurable ticks-per-frame multiplier.

**Tech Stack:** Rust, Rust, bevy_ecs 0.18 (standalone), ratatui, crossterm, rand 0.9/rand_chacha, noise, ron, serde

**Design spec:** `~/.claude/plans/structured-napping-candle.md`

---

## File Structure

```
clowder/
├── src/
│   ├── main.rs                  # Entry point: setup ECS world, run main loop
│   ├── lib.rs                   # Re-exports, plugin registration
│   ├── components/
│   │   ├── mod.rs               # Re-exports all components
│   │   ├── identity.rs          # Name, Species, Age, Gender, Orientation, Appearance
│   │   ├── personality.rs       # Personality (drives, temperament, values)
│   │   ├── physical.rs          # Position, Health, Needs
│   │   ├── mental.rs            # Mood, Memory
│   │   └── skills.rs            # Skills, MagicAffinity, Corruption, Training
│   ├── resources/
│   │   ├── mod.rs               # Re-exports all resources
│   │   ├── time.rs              # TimeState, SimConfig
│   │   ├── map.rs               # TileMap
│   │   ├── weather.rs           # WeatherState
│   │   ├── narrative.rs         # NarrativeLog
│   │   └── rng.rs               # SimRng wrapper
│   ├── world_gen/
│   │   ├── mod.rs               # generate_world() entry point
│   │   ├── terrain.rs           # Noise-based terrain generation
│   │   └── colony.rs            # Colony site selection, starting structures, cat generation
│   ├── systems/
│   │   ├── mod.rs               # System registration, schedule setup
│   │   ├── time.rs              # advance_time system
│   │   ├── weather.rs           # update_weather system
│   │   ├── needs.rs             # decay_needs, satisfy_needs systems
│   │   ├── ai.rs                # evaluate_actions, select_action systems
│   │   ├── actions.rs           # resolve_eat, resolve_sleep, resolve_move, resolve_idle
│   │   └── narrative.rs         # generate_narrative_events system
│   ├── ai/
│   │   ├── mod.rs               # Action enum, scoring entry point
│   │   ├── scoring.rs           # Utility scoring functions
│   │   └── pathfinding.rs       # A* on tile grid
│   └── tui/
│       ├── mod.rs               # App struct, main render function, input handling
│       ├── map.rs               # MapWidget — renders tile grid + entities
│       ├── log.rs               # LogWidget — scrolling narrative log
│       └── status.rs            # StatusWidget — day/weather/population/speed
├── data/
│   └── templates/               # Empty for Phase 1 — templates come in Phase 2
├── docs/
│   └── systems/                 # Design reference stubs (created in Task 2)
├── Cargo.toml
├── CLAUDE.md
└── justfile
```

---

### Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `justfile`, `CLAUDE.md`

- [ ] **Step 1: Initialize the Rust project**

```bash
cd ~/clowder
cargo init --name clowder
```

- [ ] **Step 2: Set up Cargo.toml with dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "clowder"
version = "0.1.0"
edition = "2021"
description = "A terminal-based colony simulation"

[dependencies]
bevy_ecs = "0.18"
ratatui = "0.29"
crossterm = "0.28"
rand = "0.9"
rand_chacha = "0.3"
noise = "0.9"
ron = "0.8"
serde = { version = "1", features = ["derive"] }

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 2
```

Note: Verify these resolve with `cargo check` after adding. If `rand` 0.9 has API changes (e.g., `gen_range` or `gen` moved), adapt call sites. Similarly verify `noise` crate API — if `Perlin::new(seed)` changed, check docs.

- [ ] **Step 3: Create justfile**

```justfile
# Run the simulation
run *ARGS:
    cargo run -- {{ARGS}}

# Run with a specific seed
seed SEED:
    cargo run -- --seed {{SEED}}

# Build
build:
    cargo build

# Run tests
test:
    cargo test

# Check + clippy
check:
    cargo check && cargo clippy -- -D warnings

# Run all checks
ci: check test
```

- [ ] **Step 4: Create CLAUDE.md**

```markdown
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
```

- [ ] **Step 5: Stub src/main.rs and src/lib.rs**

`src/main.rs`:
```rust
fn main() {
    println!("Clowder — a colony of cats");
}
```

`src/lib.rs`:
```rust
pub mod components;
pub mod resources;
pub mod world_gen;
pub mod systems;
pub mod ai;
pub mod tui;
```

- [ ] **Step 6: Create module stubs so it compiles**

Create empty `mod.rs` files for each module directory:
- `src/components/mod.rs`
- `src/resources/mod.rs`
- `src/world_gen/mod.rs`
- `src/systems/mod.rs`
- `src/ai/mod.rs`
- `src/tui/mod.rs`

- [ ] **Step 7: Verify it builds**

```bash
just build
```

Expected: compiles with no errors. May have unused import warnings — that's fine.

- [ ] **Step 8: Initialize VCS and commit**

```bash
cd ~/clowder
jj git init
jj new
jj describe -m "chore: initialize clowder project with deps and CLAUDE.md"
```

---

### Task 2: System Design Stubs

**Files:**
- Create: 17 files in `docs/systems/`

- [ ] **Step 1: Create docs/systems/ directory and all stub files**

Create all 17 stub files. Each follows this template:

```markdown
# [System Name]

## Purpose
[What this system does and why it exists]

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| ...       | ...          | ...       |

## Formulas
[Key mathematical relationships]

## Tuning Notes
_Record observations and adjustments here during iteration._
```

Files to create with their initial parameter tables populated from the spec:

1. `docs/systems/needs.md` — Maslow levels, decay rates per need, suppression multiplier curve
2. `docs/systems/personality.md` — 18 axes listed with mechanical effects and generation distribution
3. `docs/systems/utility-ai.md` — scoring formula structure, weight factors, jitter range (0.05)
4. `docs/systems/skills.md` — 6 skills, growth curve formula, specialization pressure, starting ranges
5. `docs/systems/relationships.md` — fondness/familiarity/romantic ranges, interaction deltas, bond thresholds
6. `docs/systems/mood.md` — contagion radius (3 tiles), weight by proximity/fondness, modifier decay
7. `docs/systems/coordination.md` — social weight formula, directive strength, re-eval frequency (50 ticks)
8. `docs/systems/collective-memory.md` — capacity (20), transmission probability, colony threshold (3 cats), decay rates
9. `docs/systems/magic.md` — affinity distribution, misfire curve, corruption spread (0.001/tick to adjacent), ward decay
10. `docs/systems/weather.md` — 8 weather states, season transition matrices, effect multipliers
11. `docs/systems/time.md` — 25 ticks/day-phase, 2000 ticks/season, speed mappings (1x/5x/20x)
12. `docs/systems/buildings.md` — 8 structure types with construction costs, condition decay rate, effect values
13. `docs/systems/combat.md` — fight formula, group bonus (+0.2/ally), injury thresholds, threat stats
14. `docs/systems/narrative.md` — template matching algorithm, specificity weight formula, rate limit (1 micro/cat/5 ticks)
15. `docs/systems/world-gen.md` — noise octaves, terrain thresholds, colony site criteria, special location density
16. `docs/systems/activity-cascading.md` — proximity radius (5 tiles), bonus formula, complementary action pairs
17. `docs/systems/identity.md` — gender distribution weights, orientation distribution weights, name pool, appearance pools

- [ ] **Step 2: Commit**

```bash
jj new
jj describe -m "docs: add system design stubs for all tunable systems"
```

---

### Task 3: Core Types — Time & Config

**Files:**
- Create: `src/resources/time.rs`, `src/resources/rng.rs`
- Modify: `src/resources/mod.rs`
- Test: inline `#[cfg(test)]` module

- [ ] **Step 1: Write tests for time types**

In `src/resources/time.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_phase_from_tick() {
        let config = SimConfig::default();
        // With 25 ticks per phase: 0-24 = Dawn, 25-49 = Day, 50-74 = Dusk, 75-99 = Night
        assert_eq!(DayPhase::from_tick(0, &config), DayPhase::Dawn);
        assert_eq!(DayPhase::from_tick(24, &config), DayPhase::Dawn);
        assert_eq!(DayPhase::from_tick(25, &config), DayPhase::Day);
        assert_eq!(DayPhase::from_tick(50, &config), DayPhase::Dusk);
        assert_eq!(DayPhase::from_tick(75, &config), DayPhase::Night);
        assert_eq!(DayPhase::from_tick(100, &config), DayPhase::Dawn); // wraps
    }

    #[test]
    fn season_from_tick() {
        let config = SimConfig::default();
        // With 2000 ticks per season: 0-1999 = Spring, 2000-3999 = Summer, etc.
        assert_eq!(Season::from_tick(0, &config), Season::Spring);
        assert_eq!(Season::from_tick(1999, &config), Season::Spring);
        assert_eq!(Season::from_tick(2000, &config), Season::Summer);
        assert_eq!(Season::from_tick(4000, &config), Season::Autumn);
        assert_eq!(Season::from_tick(6000, &config), Season::Winter);
        assert_eq!(Season::from_tick(8000, &config), Season::Spring); // wraps
    }

    #[test]
    fn day_number_from_tick() {
        let config = SimConfig::default();
        // 100 ticks per day (4 phases * 25 ticks)
        assert_eq!(TimeState::day_number(0, &config), 1);
        assert_eq!(TimeState::day_number(99, &config), 1);
        assert_eq!(TimeState::day_number(100, &config), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
just test
```

Expected: compilation errors — types don't exist yet.

- [ ] **Step 3: Implement time types**

`src/resources/time.rs`:

```rust
use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl DayPhase {
    pub fn from_tick(tick: u64, config: &SimConfig) -> Self {
        let ticks_per_day = config.ticks_per_day_phase * 4;
        let phase_tick = (tick % ticks_per_day) / config.ticks_per_day_phase;
        match phase_tick {
            0 => DayPhase::Dawn,
            1 => DayPhase::Day,
            2 => DayPhase::Dusk,
            3 => DayPhase::Night,
            _ => unreachable!(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DayPhase::Dawn => "Dawn",
            DayPhase::Day => "Day",
            DayPhase::Dusk => "Dusk",
            DayPhase::Night => "Night",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn from_tick(tick: u64, config: &SimConfig) -> Self {
        let ticks_per_year = config.ticks_per_season * 4;
        let season_index = (tick % ticks_per_year) / config.ticks_per_season;
        match season_index {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            3 => Season::Winter,
            _ => unreachable!(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
}

#[derive(Resource)]
pub struct SimConfig {
    pub ticks_per_day_phase: u64,
    pub ticks_per_season: u64,
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            ticks_per_day_phase: 25,
            ticks_per_season: 2000,
            seed: 42,
        }
    }
}

#[derive(Resource)]
pub struct TimeState {
    pub tick: u64,
    pub paused: bool,
    pub speed: SimSpeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimSpeed {
    Normal,   // 1 tick per frame-tick
    Fast,     // 5 ticks per frame-tick
    VeryFast, // 20 ticks per frame-tick
}

impl SimSpeed {
    pub fn ticks_per_update(&self) -> u64 {
        match self {
            SimSpeed::Normal => 1,
            SimSpeed::Fast => 5,
            SimSpeed::VeryFast => 20,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SimSpeed::Normal => "1x",
            SimSpeed::Fast => "5x",
            SimSpeed::VeryFast => "20x",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            SimSpeed::Normal => SimSpeed::Fast,
            SimSpeed::Fast => SimSpeed::VeryFast,
            SimSpeed::VeryFast => SimSpeed::Normal,
        }
    }
}

impl Default for TimeState {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: false,
            speed: SimSpeed::Normal,
        }
    }
}

impl TimeState {
    pub fn day_phase(&self, config: &SimConfig) -> DayPhase {
        DayPhase::from_tick(self.tick, config)
    }

    pub fn season(&self, config: &SimConfig) -> Season {
        Season::from_tick(self.tick, config)
    }

    pub fn day_number(tick: u64, config: &SimConfig) -> u64 {
        let ticks_per_day = config.ticks_per_day_phase * 4;
        tick / ticks_per_day + 1
    }
}
```

- [ ] **Step 4: Implement SimRng**

`src/resources/rng.rs`:

```rust
use bevy_ecs::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Resource)]
pub struct SimRng(pub ChaCha8Rng);

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }
}
```

- [ ] **Step 5: Wire up resources/mod.rs**

```rust
pub mod time;
pub mod rng;

pub use time::*;
pub use rng::*;
```

- [ ] **Step 6: Run tests**

```bash
just test
```

Expected: all time tests pass.

- [ ] **Step 7: Commit**

```bash
jj new
jj describe -m "feat: add time, season, day/night types and SimRng"
```

---

### Task 4: Core Types — Map & Terrain

**Files:**
- Create: `src/resources/map.rs`
- Modify: `src/resources/mod.rs`
- Test: inline `#[cfg(test)]` module

- [ ] **Step 1: Write tests for terrain and tile map**

In `src/resources/map.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_map_dimensions() {
        let map = TileMap::new(80, 60, Terrain::Grass);
        assert_eq!(map.width, 80);
        assert_eq!(map.height, 60);
    }

    #[test]
    fn tile_map_get_set() {
        let mut map = TileMap::new(10, 10, Terrain::Grass);
        map.set(3, 4, Terrain::Water);
        assert_eq!(map.get(3, 4).terrain, Terrain::Water);
        assert_eq!(map.get(0, 0).terrain, Terrain::Grass);
    }

    #[test]
    fn tile_map_bounds() {
        let map = TileMap::new(10, 10, Terrain::Grass);
        assert!(map.in_bounds(0, 0));
        assert!(map.in_bounds(9, 9));
        assert!(!map.in_bounds(10, 0));
        assert!(!map.in_bounds(0, 10));
        assert!(!map.in_bounds(-1, 0));
    }

    #[test]
    fn terrain_movement_cost() {
        assert_eq!(Terrain::Grass.movement_cost(), 1);
        assert_eq!(Terrain::DenseForest.movement_cost(), 3);
        assert_eq!(Terrain::Water.movement_cost(), u32::MAX); // impassable
    }

    #[test]
    fn terrain_symbol() {
        assert_eq!(Terrain::Grass.symbol(), '.');
        assert_eq!(Terrain::Water.symbol(), '~');
        assert_eq!(Terrain::DenseForest.symbol(), 'T');
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
just test
```

- [ ] **Step 3: Implement map types**

`src/resources/map.rs`:

```rust
use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Terrain {
    Grass,
    LightForest,
    DenseForest,
    Water,
    Rock,
    Mud,
    Sand,
    // Settlement tiles
    Den,
    Hearth,
    Stores,
    Workshop,
    Garden,
    // Special
    FairyRing,
    StandingStone,
    DeepPool,
    AncientRuin,
}

impl Terrain {
    pub fn movement_cost(&self) -> u32 {
        match self {
            Terrain::Grass | Terrain::Sand => 1,
            Terrain::LightForest | Terrain::Mud | Terrain::Garden => 2,
            Terrain::DenseForest => 3,
            Terrain::Rock => 4,
            Terrain::Water => u32::MAX, // impassable
            // Settlement tiles are easy to move through
            Terrain::Den | Terrain::Hearth | Terrain::Stores
            | Terrain::Workshop => 1,
            // Special locations
            Terrain::FairyRing | Terrain::StandingStone
            | Terrain::DeepPool | Terrain::AncientRuin => 2,
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            Terrain::Grass => '.',
            Terrain::LightForest => 't',
            Terrain::DenseForest => 'T',
            Terrain::Water => '~',
            Terrain::Rock => '#',
            Terrain::Mud => ',',
            Terrain::Sand => ':',
            Terrain::Den => 'D',
            Terrain::Hearth => 'H',
            Terrain::Stores => 'S',
            Terrain::Workshop => 'W',
            Terrain::Garden => 'G',
            Terrain::FairyRing => '*',
            Terrain::StandingStone => '!',
            Terrain::DeepPool => 'O',
            Terrain::AncientRuin => '?',
        }
    }

    pub fn shelter_value(&self) -> f32 {
        match self {
            Terrain::Den => 1.0,
            Terrain::DenseForest => 0.6,
            Terrain::LightForest => 0.3,
            Terrain::Hearth | Terrain::Stores | Terrain::Workshop => 0.8,
            Terrain::AncientRuin => 0.5,
            _ => 0.0,
        }
    }

    pub fn foraging_yield(&self) -> f32 {
        match self {
            Terrain::LightForest => 0.3,
            Terrain::DenseForest => 0.5,
            Terrain::Garden => 0.8,
            Terrain::Grass => 0.1,
            _ => 0.0,
        }
    }

    pub fn is_passable(&self) -> bool {
        self.movement_cost() != u32::MAX
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub terrain: Terrain,
    pub corruption: f32,
    pub mystery: f32,
}

impl Tile {
    pub fn new(terrain: Terrain) -> Self {
        Self {
            terrain,
            corruption: 0.0,
            mystery: 0.0,
        }
    }
}

#[derive(Resource)]
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
}

impl TileMap {
    pub fn new(width: i32, height: i32, default_terrain: Terrain) -> Self {
        let tiles = vec![Tile::new(default_terrain); (width * height) as usize];
        Self { width, height, tiles }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    pub fn get(&self, x: i32, y: i32) -> &Tile {
        assert!(self.in_bounds(x, y), "tile ({x}, {y}) out of bounds");
        &self.tiles[(y * self.width + x) as usize]
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        assert!(self.in_bounds(x, y), "tile ({x}, {y}) out of bounds");
        &mut self.tiles[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, terrain: Terrain) {
        self.get_mut(x, y).terrain = terrain;
    }
}
```

- [ ] **Step 4: Update resources/mod.rs**

```rust
pub mod time;
pub mod map;
pub mod rng;

pub use time::*;
pub use map::*;
pub use rng::*;
```

- [ ] **Step 5: Run tests**

```bash
just test
```

Expected: all map + time tests pass.

- [ ] **Step 6: Commit**

```bash
jj new
jj describe -m "feat: add tile map, terrain types, and tile properties"
```

---

### Task 5: Core Types — Entity Components

**Files:**
- Create: `src/components/identity.rs`, `src/components/personality.rs`, `src/components/physical.rs`, `src/components/mental.rs`, `src/components/skills.rs`
- Modify: `src/components/mod.rs`
- Test: inline tests in each file

- [ ] **Step 1: Write identity components with tests**

`src/components/identity.rs`:

```rust
use bevy_ecs::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct Name(pub String);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Species {
    Cat,
}

#[derive(Component, Debug, Clone)]
pub struct Age {
    pub born_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeStage {
    Kitten,
    Young,
    Adult,
    Elder,
}

impl Age {
    pub fn stage(&self, current_tick: u64, ticks_per_season: u64) -> LifeStage {
        let age_ticks = current_tick.saturating_sub(self.born_tick);
        let age_seasons = age_ticks / ticks_per_season;
        match age_seasons {
            0..=3 => LifeStage::Kitten,      // first year
            4..=11 => LifeStage::Young,       // 1-3 years
            12..=47 => LifeStage::Adult,      // 3-12 years
            _ => LifeStage::Elder,            // 12+ years
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Tom,
    Queen,
    Nonbinary,
}

impl Gender {
    pub fn subject_pronoun(&self) -> &'static str {
        match self {
            Gender::Tom => "he",
            Gender::Queen => "she",
            Gender::Nonbinary => "they",
        }
    }

    pub fn object_pronoun(&self) -> &'static str {
        match self {
            Gender::Tom => "him",
            Gender::Queen => "her",
            Gender::Nonbinary => "them",
        }
    }

    pub fn possessive(&self) -> &'static str {
        match self {
            Gender::Tom => "his",
            Gender::Queen => "her",
            Gender::Nonbinary => "their",
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Straight,
    Gay,
    Bisexual,
    Asexual,
}

#[derive(Component, Debug, Clone)]
pub struct Appearance {
    pub fur_color: String,
    pub pattern: String,
    pub eye_color: String,
    pub distinguishing_marks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_stages() {
        let tps = 2000; // ticks per season
        let age = Age { born_tick: 0 };
        assert_eq!(age.stage(0, tps), LifeStage::Kitten);
        assert_eq!(age.stage(3 * tps, tps), LifeStage::Kitten);      // 3 seasons
        assert_eq!(age.stage(4 * tps, tps), LifeStage::Young);       // 4 seasons = 1 year
        assert_eq!(age.stage(12 * tps, tps), LifeStage::Adult);      // 12 seasons = 3 years
        assert_eq!(age.stage(48 * tps, tps), LifeStage::Elder);      // 48 seasons = 12 years
    }

    #[test]
    fn gender_pronouns() {
        assert_eq!(Gender::Tom.subject_pronoun(), "he");
        assert_eq!(Gender::Queen.possessive(), "her");
        assert_eq!(Gender::Nonbinary.object_pronoun(), "them");
    }
}
```

- [ ] **Step 2: Write personality component with tests**

`src/components/personality.rs`:

```rust
use bevy_ecs::prelude::*;
use rand::Rng;

/// Three-layer personality system. All axes 0.0-1.0.
#[derive(Component, Debug, Clone)]
pub struct Personality {
    // Core Drives — affect what a cat chooses to do
    pub boldness: f32,
    pub sociability: f32,
    pub curiosity: f32,
    pub diligence: f32,
    pub warmth: f32,
    pub spirituality: f32,
    pub ambition: f32,
    pub patience: f32,

    // Temperament — affect how a cat experiences things
    pub anxiety: f32,
    pub optimism: f32,
    pub temper: f32,
    pub stubbornness: f32,
    pub playfulness: f32,

    // Values — affect what a cat cares about long-term
    pub loyalty: f32,
    pub tradition: f32,
    pub compassion: f32,
    pub pride: f32,
    pub independence: f32,
}

impl Personality {
    /// Generate a random personality with bell-curve distribution.
    /// Each axis is the average of two uniform samples, biasing toward the middle
    /// while still allowing extremes.
    pub fn random(rng: &mut impl Rng) -> Self {
        let bell = |rng: &mut impl Rng| -> f32 {
            let a: f32 = rng.gen();
            let b: f32 = rng.gen();
            (a + b) / 2.0
        };

        Self {
            boldness: bell(rng),
            sociability: bell(rng),
            curiosity: bell(rng),
            diligence: bell(rng),
            warmth: bell(rng),
            spirituality: bell(rng),
            ambition: bell(rng),
            patience: bell(rng),
            anxiety: bell(rng),
            optimism: bell(rng),
            temper: bell(rng),
            stubbornness: bell(rng),
            playfulness: bell(rng),
            loyalty: bell(rng),
            tradition: bell(rng),
            compassion: bell(rng),
            pride: bell(rng),
            independence: bell(rng),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn random_personality_in_range() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for _ in 0..100 {
            let p = Personality::random(&mut rng);
            // All axes should be 0.0-1.0
            assert!((0.0..=1.0).contains(&p.boldness));
            assert!((0.0..=1.0).contains(&p.sociability));
            assert!((0.0..=1.0).contains(&p.curiosity));
            assert!((0.0..=1.0).contains(&p.diligence));
            assert!((0.0..=1.0).contains(&p.warmth));
            assert!((0.0..=1.0).contains(&p.spirituality));
            assert!((0.0..=1.0).contains(&p.ambition));
            assert!((0.0..=1.0).contains(&p.patience));
            assert!((0.0..=1.0).contains(&p.anxiety));
            assert!((0.0..=1.0).contains(&p.optimism));
            assert!((0.0..=1.0).contains(&p.temper));
            assert!((0.0..=1.0).contains(&p.stubbornness));
            assert!((0.0..=1.0).contains(&p.playfulness));
            assert!((0.0..=1.0).contains(&p.loyalty));
            assert!((0.0..=1.0).contains(&p.tradition));
            assert!((0.0..=1.0).contains(&p.compassion));
            assert!((0.0..=1.0).contains(&p.pride));
            assert!((0.0..=1.0).contains(&p.independence));
        }
    }

    #[test]
    fn random_personality_deterministic() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let p1 = Personality::random(&mut rng1);
        let p2 = Personality::random(&mut rng2);
        assert_eq!(p1.boldness, p2.boldness);
        assert_eq!(p1.compassion, p2.compassion);
    }

    #[test]
    fn bell_curve_biases_toward_middle() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut sum = 0.0;
        let n = 1000;
        for _ in 0..n {
            let p = Personality::random(&mut rng);
            sum += p.boldness;
        }
        let mean = sum / n as f32;
        // Bell curve of two uniforms has mean 0.5, should be close
        assert!((mean - 0.5).abs() < 0.05, "mean was {mean}");
    }
}
```

- [ ] **Step 3: Write physical components (Position, Health, Needs) with tests**

`src/components/physical.rs`:

```rust
use bevy_ecs::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn manhattan_distance(&self, other: &Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Debug, Clone)]
pub struct Injury {
    pub kind: InjuryKind,
    pub tick_received: u64,
    pub healed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjuryKind {
    Minor,
    Moderate,
    Severe,
}

#[derive(Component, Debug, Clone)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    pub injuries: Vec<Injury>,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
        }
    }
}

/// Maslow hierarchy needs. All values 0.0 (depleted) to 1.0 (full).
#[derive(Component, Debug, Clone)]
pub struct Needs {
    // Level 1 — Physiological
    pub hunger: f32,
    pub energy: f32,
    pub warmth: f32,

    // Level 2 — Safety
    pub safety: f32,

    // Level 3 — Belonging
    pub social: f32,
    pub acceptance: f32,

    // Level 4 — Esteem
    pub respect: f32,
    pub mastery: f32,

    // Level 5 — Self-actualization
    pub purpose: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Self {
            hunger: 0.8,
            energy: 0.8,
            warmth: 0.9,
            safety: 1.0,
            social: 0.6,
            acceptance: 0.5,
            respect: 0.3,
            mastery: 0.3,
            purpose: 0.2,
        }
    }
}

impl Needs {
    /// Returns the Maslow suppression multiplier for a given level.
    /// Lower levels suppress higher levels when depleted.
    pub fn level_suppression(&self, level: u8) -> f32 {
        match level {
            1 => 1.0, // physiological is never suppressed
            2 => self.physiological_satisfaction(),
            3 => self.physiological_satisfaction() * self.safety_satisfaction(),
            4 => {
                self.physiological_satisfaction()
                    * self.safety_satisfaction()
                    * self.belonging_satisfaction()
            }
            5 => {
                self.physiological_satisfaction()
                    * self.safety_satisfaction()
                    * self.belonging_satisfaction()
                    * self.esteem_satisfaction()
            }
            _ => 0.0,
        }
    }

    fn physiological_satisfaction(&self) -> f32 {
        let avg = (self.hunger + self.energy + self.warmth) / 3.0;
        // Smoothstep: below 0.3 heavily suppresses, above 0.6 barely suppresses
        smoothstep(0.15, 0.6, avg)
    }

    fn safety_satisfaction(&self) -> f32 {
        smoothstep(0.2, 0.7, self.safety)
    }

    fn belonging_satisfaction(&self) -> f32 {
        let avg = (self.social + self.acceptance) / 2.0;
        smoothstep(0.2, 0.6, avg)
    }

    fn esteem_satisfaction(&self) -> f32 {
        let avg = (self.respect + self.mastery) / 2.0;
        smoothstep(0.2, 0.6, avg)
    }
}

/// Smooth interpolation from 0.0 to 1.0 between edge0 and edge1.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_distance() {
        let a = Position::new(0, 0);
        let b = Position::new(3, 4);
        assert!((a.distance_to(&b) - 5.0).abs() < 0.001);
        assert_eq!(a.manhattan_distance(&b), 7);
    }

    #[test]
    fn needs_default_values() {
        let needs = Needs::default();
        assert_eq!(needs.hunger, 0.8);
        assert_eq!(needs.energy, 0.8);
        assert_eq!(needs.safety, 1.0);
    }

    #[test]
    fn needs_suppression_level1_never_suppressed() {
        let needs = Needs {
            hunger: 0.0, energy: 0.0, warmth: 0.0,
            ..Default::default()
        };
        assert_eq!(needs.level_suppression(1), 1.0);
    }

    #[test]
    fn needs_suppression_starving_suppresses_higher() {
        let needs = Needs {
            hunger: 0.0, energy: 0.0, warmth: 0.0,
            ..Default::default()
        };
        // Level 2+ should be heavily suppressed
        assert!(needs.level_suppression(2) < 0.1);
        assert!(needs.level_suppression(5) < 0.01);
    }

    #[test]
    fn needs_suppression_well_fed_allows_higher() {
        let needs = Needs {
            hunger: 0.9, energy: 0.9, warmth: 0.9,
            safety: 0.9,
            social: 0.7, acceptance: 0.7,
            respect: 0.7, mastery: 0.7,
            purpose: 0.5,
        };
        // All levels should be mostly unsuppressed
        assert!(needs.level_suppression(5) > 0.5);
    }

    #[test]
    fn smoothstep_boundaries() {
        assert_eq!(smoothstep(0.0, 1.0, 0.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 1.0), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 0.01);
    }
}
```

- [ ] **Step 4: Write mental components (Mood, Memory)**

`src/components/mental.rs`:

```rust
use bevy_ecs::prelude::*;
use std::collections::VecDeque;
use crate::components::physical::Position;

#[derive(Debug, Clone)]
pub struct MoodModifier {
    pub amount: f32,
    pub ticks_remaining: u64,
    pub source: String,
}

#[derive(Component, Debug, Clone)]
pub struct Mood {
    pub valence: f32, // -1.0 (miserable) to 1.0 (ecstatic)
    pub modifiers: VecDeque<MoodModifier>,
}

impl Default for Mood {
    fn default() -> Self {
        Self {
            valence: 0.2, // slightly positive baseline
            modifiers: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryType {
    ThreatSeen,
    ResourceFound,
    Death,
    MagicEvent,
    Injury,
    SocialEvent,
}

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub event_type: MemoryType,
    pub location: Option<Position>,
    pub involved: Vec<Entity>,
    pub tick: u64,
    pub strength: f32,
    pub firsthand: bool,
}

#[derive(Component, Debug, Clone)]
pub struct Memory {
    pub events: VecDeque<MemoryEntry>,
    pub capacity: usize,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            capacity: 20,
        }
    }
}
```

- [ ] **Step 5: Write skills component**

`src/components/skills.rs`:

```rust
use bevy_ecs::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct Skills {
    pub hunting: f32,
    pub foraging: f32,
    pub herbcraft: f32,
    pub building: f32,
    pub combat: f32,
    pub magic: f32,
}

impl Skills {
    pub fn total(&self) -> f32 {
        self.hunting + self.foraging + self.herbcraft
            + self.building + self.combat + self.magic
    }

    /// Growth rate multiplier based on total skill points.
    /// More total skills = slower learning for any new skill.
    pub fn growth_rate(&self) -> f32 {
        let total = self.total();
        // Diminishing returns: at total=0 rate is 1.0, at total=3.0 rate is ~0.25
        1.0 / (1.0 + total)
    }
}

impl Default for Skills {
    fn default() -> Self {
        Self {
            hunting: 0.1,
            foraging: 0.1,
            herbcraft: 0.05,
            building: 0.1,
            combat: 0.05,
            magic: 0.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct MagicAffinity(pub f32);

#[derive(Component, Debug, Clone, Copy)]
pub struct Corruption(pub f32);

#[derive(Component, Debug, Clone)]
pub struct Training {
    pub mentor: Option<Entity>,
    pub apprentice: Option<Entity>,
}

impl Default for Training {
    fn default() -> Self {
        Self { mentor: None, apprentice: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_growth_rate_diminishes() {
        let low = Skills { hunting: 0.1, ..Default::default() };
        let high = Skills {
            hunting: 0.8, foraging: 0.6, building: 0.5,
            ..Default::default()
        };
        assert!(low.growth_rate() > high.growth_rate());
    }

    #[test]
    fn skill_total() {
        let s = Skills {
            hunting: 0.2, foraging: 0.3, herbcraft: 0.1,
            building: 0.4, combat: 0.0, magic: 0.0,
        };
        assert!((s.total() - 1.0).abs() < 0.001);
    }
}
```

- [ ] **Step 6: Wire up components/mod.rs**

```rust
pub mod identity;
pub mod personality;
pub mod physical;
pub mod mental;
pub mod skills;

pub use identity::*;
pub use personality::*;
pub use physical::*;
pub use mental::*;
pub use skills::*;
```

- [ ] **Step 7: Run all tests**

```bash
just test
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
jj new
jj describe -m "feat: add entity components — identity, personality, needs, mood, skills"
```

---

### Task 6: World Generation

**Files:**
- Create: `src/world_gen/terrain.rs`, `src/world_gen/colony.rs`
- Modify: `src/world_gen/mod.rs`
- Test: inline tests

- [ ] **Step 1: Write terrain generation test**

In `src/world_gen/terrain.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn generated_map_has_correct_dimensions() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let map = generate_terrain(80, 60, &mut rng);
        assert_eq!(map.width, 80);
        assert_eq!(map.height, 60);
    }

    #[test]
    fn generated_map_has_terrain_variety() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let map = generate_terrain(80, 60, &mut rng);
        let mut has_grass = false;
        let mut has_forest = false;
        let mut has_water = false;
        for y in 0..map.height {
            for x in 0..map.width {
                match map.get(x, y).terrain {
                    Terrain::Grass => has_grass = true,
                    Terrain::DenseForest | Terrain::LightForest => has_forest = true,
                    Terrain::Water => has_water = true,
                    _ => {}
                }
            }
        }
        assert!(has_grass, "map should have grass");
        assert!(has_forest, "map should have forest");
        assert!(has_water, "map should have water");
    }

    #[test]
    fn generation_is_deterministic() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let map1 = generate_terrain(40, 30, &mut rng1);
        let map2 = generate_terrain(40, 30, &mut rng2);
        for y in 0..30 {
            for x in 0..40 {
                assert_eq!(map1.get(x, y).terrain, map2.get(x, y).terrain);
            }
        }
    }
}
```

- [ ] **Step 2: Implement terrain generation**

`src/world_gen/terrain.rs`:

```rust
use noise::{NoiseFn, Perlin, Seedable};
use rand::Rng;
use crate::resources::map::{TileMap, Terrain};

pub fn generate_terrain(width: i32, height: i32, rng: &mut impl Rng) -> TileMap {
    let mut map = TileMap::new(width, height, Terrain::Grass);
    let seed: u32 = rng.gen();

    let elevation = Perlin::new(seed);
    let moisture = Perlin::new(seed.wrapping_add(1));

    let scale = 0.05; // controls terrain feature size

    for y in 0..height {
        for x in 0..width {
            let nx = x as f64 * scale;
            let ny = y as f64 * scale;

            let e = elevation.get([nx, ny]);  // roughly -1.0 to 1.0
            let m = moisture.get([nx, ny]);

            let terrain = classify_terrain(e, m);
            map.set(x, y, terrain);
        }
    }

    map
}

fn classify_terrain(elevation: f64, moisture: f64) -> Terrain {
    // Elevation thresholds
    if elevation < -0.3 {
        return Terrain::Water;
    }
    if elevation > 0.6 {
        return Terrain::Rock;
    }

    // Mid-elevation terrain depends on moisture
    if moisture > 0.3 {
        if elevation > 0.2 {
            Terrain::DenseForest
        } else {
            Terrain::LightForest
        }
    } else if moisture < -0.2 {
        if elevation < 0.0 {
            Terrain::Mud
        } else {
            Terrain::Sand
        }
    } else {
        Terrain::Grass
    }
}
```

- [ ] **Step 3: Write colony initialization test**

In `src/world_gen/colony.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use crate::world_gen::terrain::generate_terrain;

    #[test]
    fn find_colony_site_returns_passable_area() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let map = generate_terrain(80, 60, &mut rng);
        let site = find_colony_site(&map, &mut rng);
        // Colony center should be on passable terrain
        assert!(map.get(site.x, site.y).terrain.is_passable());
    }

    #[test]
    fn generate_cats_produces_correct_count() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let cats = generate_starting_cats(8, &mut rng);
        assert_eq!(cats.len(), 8);
    }

    #[test]
    fn generated_cats_have_unique_names() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let cats = generate_starting_cats(10, &mut rng);
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "all cat names should be unique");
    }
}
```

- [ ] **Step 4: Implement colony initialization**

`src/world_gen/colony.rs`:

```rust
use rand::Rng;
use rand::seq::SliceRandom;
use crate::components::identity::*;
use crate::components::personality::Personality;
use crate::components::physical::*;
use crate::components::mental::*;
use crate::components::skills::*;
use crate::resources::map::TileMap;

const NAME_POOL: &[&str] = &[
    "Bramble", "Thistle", "Cedar", "Moss", "Fern", "Ash", "Reed",
    "Clover", "Wren", "Hazel", "Rowan", "Sage", "Ivy", "Birch",
    "Flint", "Nettle", "Sorrel", "Briar", "Ember", "Willow",
    "Thorn", "Juniper", "Lark", "Pebble", "Lichen", "Mallow",
    "Basil", "Tansy", "Finch", "Heron",
];

const FUR_COLORS: &[&str] = &[
    "ginger", "black", "white", "gray", "tabby brown", "calico",
    "tortoiseshell", "cream", "silver", "russet",
];

const EYE_COLORS: &[&str] = &[
    "amber", "green", "blue", "copper", "hazel", "gold",
];

const PATTERNS: &[&str] = &[
    "solid", "tabby", "spotted", "tuxedo", "bicolor", "van",
    "point", "mackerel", "ticked",
];

pub struct CatBlueprint {
    pub name: String,
    pub gender: Gender,
    pub orientation: Orientation,
    pub personality: Personality,
    pub appearance: Appearance,
    pub skills: Skills,
    pub magic_affinity: f32,
    pub position: Position,
}

pub fn find_colony_site(map: &TileMap, rng: &mut impl Rng) -> Position {
    // Try random positions, pick one that's passable grass with some neighbors
    for _ in 0..1000 {
        let x = rng.gen_range(15..map.width - 15);
        let y = rng.gen_range(15..map.height - 15);
        if map.get(x, y).terrain.is_passable() {
            // Check that the area is mostly passable
            let mut passable = 0;
            for dy in -5..=5 {
                for dx in -5..=5 {
                    if map.in_bounds(x + dx, y + dy)
                        && map.get(x + dx, y + dy).terrain.is_passable()
                    {
                        passable += 1;
                    }
                }
            }
            if passable > 80 {
                return Position::new(x, y);
            }
        }
    }
    // Fallback: center of map
    Position::new(map.width / 2, map.height / 2)
}

pub fn generate_starting_cats(
    count: usize,
    rng: &mut impl Rng,
) -> Vec<CatBlueprint> {
    let mut names: Vec<&str> = NAME_POOL.to_vec();
    names.shuffle(rng);

    (0..count)
        .map(|i| {
            let personality = Personality::random(rng);

            let gender = match rng.gen_range(0..20) {
                0 => Gender::Nonbinary,  // ~5%
                1..=10 => Gender::Tom,   // ~50%
                _ => Gender::Queen,      // ~45%
            };

            let orientation = match rng.gen_range(0..20) {
                0..=14 => Orientation::Straight, // ~75%
                15..=16 => Orientation::Gay,     // ~10%
                17..=18 => Orientation::Bisexual, // ~10%
                _ => Orientation::Asexual,        // ~5%
            };

            let magic_affinity = match rng.gen_range(0..20) {
                0 => rng.gen_range(0.7..1.0),     // ~5% high
                1..=3 => rng.gen_range(0.3..0.6),  // ~15% moderate
                _ => rng.gen_range(0.0..0.2),       // ~80% negligible
            };

            let mut skills = Skills::default();
            // Give one aptitude skill a boost based on personality
            if personality.boldness > 0.6 {
                skills.hunting += rng.gen_range(0.1..0.3);
                skills.combat += rng.gen_range(0.05..0.15);
            }
            if personality.diligence > 0.6 {
                skills.building += rng.gen_range(0.1..0.3);
                skills.foraging += rng.gen_range(0.05..0.15);
            }
            if personality.spirituality > 0.6 && magic_affinity > 0.3 {
                skills.magic += rng.gen_range(0.05..0.2);
            }

            CatBlueprint {
                name: names[i].to_string(),
                gender,
                orientation,
                personality,
                appearance: Appearance {
                    fur_color: FUR_COLORS.choose(rng).unwrap().to_string(),
                    pattern: PATTERNS.choose(rng).unwrap().to_string(),
                    eye_color: EYE_COLORS.choose(rng).unwrap().to_string(),
                    distinguishing_marks: Vec::new(),
                },
                skills,
                magic_affinity,
                // Position will be set relative to colony site
                position: Position::new(0, 0),
            }
        })
        .collect()
}
```

- [ ] **Step 5: Wire up world_gen/mod.rs**

```rust
pub mod terrain;
pub mod colony;
```

- [ ] **Step 6: Run tests**

```bash
just test
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
jj new
jj describe -m "feat: add noise-based terrain generation and colony initialization"
```

---

### Task 7: Time & Weather Systems

**Files:**
- Create: `src/systems/time.rs`, `src/systems/weather.rs`, `src/resources/weather.rs`
- Modify: `src/systems/mod.rs`, `src/resources/mod.rs`

- [ ] **Step 1: Write weather resource with transition tests**

`src/resources/weather.rs`:

```rust
use bevy_ecs::prelude::*;
use rand::Rng;
use crate::resources::time::Season;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weather {
    Clear,
    Overcast,
    LightRain,
    HeavyRain,
    Snow,
    Fog,
    Wind,
    Storm,
}

impl Weather {
    pub fn label(&self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Overcast => "Overcast",
            Weather::LightRain => "Light rain",
            Weather::HeavyRain => "Heavy rain",
            Weather::Snow => "Snow",
            Weather::Fog => "Fog",
            Weather::Wind => "Windy",
            Weather::Storm => "Storm",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Weather::Clear => "☀",
            Weather::Overcast => "☁",
            Weather::LightRain => "🌧",
            Weather::HeavyRain => "⛈",
            Weather::Snow => "❄",
            Weather::Fog => "🌫",
            Weather::Wind => "💨",
            Weather::Storm => "⛈",
        }
    }

    pub fn movement_multiplier(&self) -> f32 {
        match self {
            Weather::Clear | Weather::Overcast => 1.0,
            Weather::LightRain | Weather::Fog => 0.9,
            Weather::Wind => 0.85,
            Weather::HeavyRain => 0.7,
            Weather::Snow => 0.6,
            Weather::Storm => 0.4,
        }
    }

    pub fn comfort_modifier(&self) -> f32 {
        match self {
            Weather::Clear => 0.0,
            Weather::Overcast => 0.0,
            Weather::LightRain => -0.05,
            Weather::Fog => -0.02,
            Weather::Wind => -0.08,
            Weather::HeavyRain => -0.15,
            Weather::Snow => -0.2,
            Weather::Storm => -0.3,
        }
    }
}

#[derive(Resource)]
pub struct WeatherState {
    pub current: Weather,
    pub ticks_until_change: u64,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            current: Weather::Clear,
            ticks_until_change: 50,
        }
    }
}

impl WeatherState {
    pub fn next_weather(&self, season: Season, rng: &mut impl Rng) -> Weather {
        // Season-weighted transition probabilities
        let weights: &[(Weather, f32)] = match season {
            Season::Spring => &[
                (Weather::Clear, 3.0), (Weather::Overcast, 2.0),
                (Weather::LightRain, 2.0), (Weather::HeavyRain, 0.5),
                (Weather::Fog, 1.0), (Weather::Wind, 1.0),
            ],
            Season::Summer => &[
                (Weather::Clear, 5.0), (Weather::Overcast, 1.5),
                (Weather::LightRain, 1.0), (Weather::HeavyRain, 0.3),
                (Weather::Wind, 0.5), (Weather::Storm, 0.2),
            ],
            Season::Autumn => &[
                (Weather::Clear, 2.0), (Weather::Overcast, 3.0),
                (Weather::LightRain, 2.0), (Weather::HeavyRain, 1.5),
                (Weather::Fog, 1.5), (Weather::Wind, 2.0),
                (Weather::Storm, 0.5),
            ],
            Season::Winter => &[
                (Weather::Clear, 2.0), (Weather::Overcast, 3.0),
                (Weather::Snow, 3.0), (Weather::Wind, 2.0),
                (Weather::Fog, 1.0), (Weather::Storm, 0.5),
            ],
        };

        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        let mut roll: f32 = rng.gen_range(0.0..total);
        for (weather, weight) in weights {
            roll -= weight;
            if roll <= 0.0 {
                return *weather;
            }
        }
        Weather::Clear // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn weather_transitions_produce_valid_weather() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let state = WeatherState::default();
        for _ in 0..100 {
            let next = state.next_weather(Season::Summer, &mut rng);
            // Should be one of the valid weather types
            assert!(matches!(next,
                Weather::Clear | Weather::Overcast | Weather::LightRain
                | Weather::HeavyRain | Weather::Snow | Weather::Fog
                | Weather::Wind | Weather::Storm
            ));
        }
    }

    #[test]
    fn winter_produces_snow() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let state = WeatherState::default();
        let mut saw_snow = false;
        for _ in 0..100 {
            if state.next_weather(Season::Winter, &mut rng) == Weather::Snow {
                saw_snow = true;
                break;
            }
        }
        assert!(saw_snow, "winter should eventually produce snow");
    }
}
```

- [ ] **Step 2: Implement time and weather systems**

`src/systems/time.rs`:

```rust
use bevy_ecs::prelude::*;
use crate::resources::time::TimeState;

pub fn advance_time(mut time: ResMut<TimeState>) {
    if !time.paused {
        time.tick += 1;
    }
}
```

`src/systems/weather.rs`:

```rust
use bevy_ecs::prelude::*;
use crate::resources::time::{TimeState, SimConfig};
use crate::resources::weather::WeatherState;
use crate::resources::rng::SimRng;
use rand::Rng;

pub fn update_weather(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut weather: ResMut<WeatherState>,
    mut rng: ResMut<SimRng>,
) {
    if weather.ticks_until_change == 0 {
        let season = time.season(&config);
        weather.current = weather.next_weather(season, &mut rng.0);
        weather.ticks_until_change = rng.0.gen_range(30..80);
    } else {
        weather.ticks_until_change -= 1;
    }
}
```

- [ ] **Step 3: Wire up systems/mod.rs and resources/mod.rs**

`src/systems/mod.rs`:

```rust
pub mod time;
pub mod weather;
pub mod needs;
pub mod ai;
pub mod actions;
pub mod narrative;
```

`src/resources/mod.rs` — add weather:

```rust
pub mod time;
pub mod map;
pub mod weather;
pub mod rng;
pub mod narrative;

pub use time::*;
pub use map::*;
pub use weather::*;
pub use rng::*;
```

- [ ] **Step 4: Run tests**

```bash
just test
```

- [ ] **Step 5: Commit**

```bash
jj new
jj describe -m "feat: add time advancement and weather transition systems"
```

---

### Task 8: Needs Decay System

**Files:**
- Create: `src/systems/needs.rs`
- Test: inline tests

- [ ] **Step 1: Write needs decay tests**

In `src/systems/needs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;
    use crate::components::personality::Personality;
    use crate::resources::time::{TimeState, SimConfig};
    use crate::resources::weather::{WeatherState, Weather};

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_needs);
        (world, schedule)
    }

    #[test]
    fn hunger_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let mut personality = Personality::random(
            &mut rand_chacha::ChaCha8Rng::seed_from_u64(1)
        );
        let needs_before = Needs::default();
        let hunger_before = needs_before.hunger;

        world.spawn((needs_before, personality));
        schedule.run(&mut world);

        let needs_after = world.query::<&Needs>().single(&world);
        assert!(needs_after.hunger < hunger_before, "hunger should decay");
    }

    #[test]
    fn energy_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let personality = Personality::random(
            &mut rand_chacha::ChaCha8Rng::seed_from_u64(1)
        );
        let needs = Needs::default();
        let energy_before = needs.energy;

        world.spawn((needs, personality));
        schedule.run(&mut world);

        let needs_after = world.query::<&Needs>().single(&world);
        assert!(needs_after.energy < energy_before, "energy should decay");
    }
}
```

- [ ] **Step 2: Implement needs decay**

```rust
use bevy_ecs::prelude::*;
use crate::components::physical::Needs;
use crate::components::personality::Personality;
use crate::resources::time::{TimeState, SimConfig};
use crate::resources::weather::{WeatherState, Weather};

// Base decay rates per tick
const HUNGER_DECAY: f32 = 0.003;
const ENERGY_DECAY: f32 = 0.002;
const WARMTH_DECAY: f32 = 0.001;
const SAFETY_RECOVERY: f32 = 0.005;
const SOCIAL_DECAY: f32 = 0.001;
const ACCEPTANCE_DECAY: f32 = 0.0005;
const RESPECT_DECAY: f32 = 0.0003;
const MASTERY_DECAY: f32 = 0.0002;
const PURPOSE_DECAY: f32 = 0.0001;

pub fn decay_needs(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    mut query: Query<(&mut Needs, &Personality)>,
) {
    let season = time.season(&config);
    let day_phase = time.day_phase(&config);

    for (mut needs, personality) in &mut query {
        // Level 1 — Physiological
        needs.hunger = (needs.hunger - HUNGER_DECAY).max(0.0);
        needs.energy = (needs.energy - ENERGY_DECAY).max(0.0);

        // Warmth affected by weather and season
        let weather_warmth_drain = match weather.current {
            Weather::Snow => 0.004,
            Weather::Storm => 0.003,
            Weather::Wind => 0.002,
            Weather::HeavyRain => 0.002,
            Weather::LightRain => 0.001,
            _ => 0.0,
        };
        let season_warmth_drain = match season {
            crate::resources::time::Season::Winter => 0.003,
            crate::resources::time::Season::Autumn => 0.001,
            _ => 0.0,
        };
        needs.warmth = (needs.warmth - WARMTH_DECAY - weather_warmth_drain - season_warmth_drain).max(0.0);

        // Level 2 — Safety (recovers toward 1.0 slowly)
        if needs.safety < 1.0 {
            needs.safety = (needs.safety + SAFETY_RECOVERY).min(1.0);
        }

        // Level 3 — Belonging (scaled by personality)
        let social_rate = SOCIAL_DECAY * (1.0 + personality.sociability * 0.5);
        needs.social = (needs.social - social_rate).max(0.0);

        let acceptance_rate = ACCEPTANCE_DECAY * (1.0 + personality.warmth * 0.5);
        needs.acceptance = (needs.acceptance - acceptance_rate).max(0.0);

        // Level 4 — Esteem
        let respect_rate = RESPECT_DECAY * (1.0 + personality.ambition * 0.5);
        needs.respect = (needs.respect - respect_rate).max(0.0);

        let mastery_rate = MASTERY_DECAY * (1.0 + personality.diligence * 0.5);
        needs.mastery = (needs.mastery - mastery_rate).max(0.0);

        // Level 5 — Self-actualization
        let purpose_rate = PURPOSE_DECAY * (1.0 + personality.curiosity * 0.5);
        needs.purpose = (needs.purpose - purpose_rate).max(0.0);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
just test
```

- [ ] **Step 4: Commit**

```bash
jj new
jj describe -m "feat: add Maslow needs decay system with personality scaling"
```

---

### Task 9: Basic Utility AI

**Files:**
- Create: `src/ai/mod.rs`, `src/ai/scoring.rs`
- Test: inline tests

- [ ] **Step 1: Define action enum and current action component**

`src/ai/mod.rs`:

```rust
pub mod scoring;
pub mod pathfinding;

use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Eat,
    Sleep,
    Wander,  // simplified movement for Phase 1
    Idle,
}

#[derive(Component, Debug, Clone)]
pub struct CurrentAction {
    pub action: Action,
    pub ticks_remaining: u64,
    pub target_position: Option<crate::components::physical::Position>,
}

impl Default for CurrentAction {
    fn default() -> Self {
        Self {
            action: Action::Idle,
            ticks_remaining: 0,
            target_position: None,
        }
    }
}
```

- [ ] **Step 2: Write scoring tests**

`src/ai/scoring.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Needs;
    use crate::components::personality::Personality;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn starving_cat_scores_eat_highest() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let personality = Personality::random(&mut rng);
        let needs = Needs {
            hunger: 0.1, // very hungry
            energy: 0.8,
            ..Default::default()
        };
        let scores = score_actions(&needs, &personality, true, &mut rng);
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert!(eat_score > sleep_score, "starving cat should want to eat");
    }

    #[test]
    fn exhausted_cat_scores_sleep_highest() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let personality = Personality::random(&mut rng);
        let needs = Needs {
            hunger: 0.8,
            energy: 0.1, // very tired
            ..Default::default()
        };
        let scores = score_actions(&needs, &personality, true, &mut rng);
        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        assert!(sleep_score > eat_score, "exhausted cat should want to sleep");
    }

    #[test]
    fn satisfied_cat_wanders_or_idles() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut personality = Personality::random(&mut rng);
        personality.curiosity = 0.9; // very curious
        let needs = Needs {
            hunger: 0.9, energy: 0.9, warmth: 0.9,
            safety: 1.0, social: 0.8, acceptance: 0.8,
            respect: 0.5, mastery: 0.5, purpose: 0.5,
        };
        let scores = score_actions(&needs, &personality, true, &mut rng);
        let best = scores.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        assert!(
            best.0 == Action::Wander || best.0 == Action::Idle,
            "satisfied curious cat should wander or idle, got {:?}",
            best.0
        );
    }
}
```

- [ ] **Step 3: Implement scoring**

```rust
use crate::ai::Action;
use crate::components::physical::Needs;
use crate::components::personality::Personality;
use rand::Rng;

const JITTER: f32 = 0.05;

pub fn score_actions(
    needs: &Needs,
    personality: &Personality,
    food_available: bool,
    rng: &mut impl Rng,
) -> Vec<(Action, f32)> {
    let mut scores = Vec::new();

    // Eat — driven by hunger (Level 1)
    if food_available {
        let urgency = 1.0 - needs.hunger; // higher urgency when hunger is low
        let score = urgency * 2.0 * needs.level_suppression(1);
        scores.push((Action::Eat, score + jitter(rng)));
    }

    // Sleep — driven by energy (Level 1)
    {
        let urgency = 1.0 - needs.energy;
        let score = urgency * 2.0 * needs.level_suppression(1);
        scores.push((Action::Sleep, score + jitter(rng)));
    }

    // Wander — driven by curiosity (higher Maslow levels)
    {
        let base = personality.curiosity * 0.5;
        let suppression = needs.level_suppression(5);
        let score = base * suppression;
        scores.push((Action::Wander, score + jitter(rng)));
    }

    // Idle — always available, low base score
    {
        let score = 0.1;
        scores.push((Action::Idle, score + jitter(rng)));
    }

    scores
}

pub fn select_best_action(scores: &[(Action, f32)]) -> Action {
    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(a, _)| *a)
        .unwrap_or(Action::Idle)
}

fn jitter(rng: &mut impl Rng) -> f32 {
    rng.gen_range(-JITTER..JITTER)
}
```

- [ ] **Step 4: Create stub pathfinding module**

`src/ai/pathfinding.rs`:

```rust
use crate::components::physical::Position;
use crate::resources::map::TileMap;

/// Simple movement toward target — move one tile closer per tick.
/// Full A* pathfinding will be added when we need it (Phase 3).
pub fn step_toward(from: &Position, to: &Position, map: &TileMap) -> Option<Position> {
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();

    // Try direct step first
    let direct = Position::new(from.x + dx, from.y + dy);
    if map.in_bounds(direct.x, direct.y) && map.get(direct.x, direct.y).terrain.is_passable() {
        return Some(direct);
    }

    // Try horizontal only
    if dx != 0 {
        let horiz = Position::new(from.x + dx, from.y);
        if map.in_bounds(horiz.x, horiz.y) && map.get(horiz.x, horiz.y).terrain.is_passable() {
            return Some(horiz);
        }
    }

    // Try vertical only
    if dy != 0 {
        let vert = Position::new(from.x, from.y + dy);
        if map.in_bounds(vert.x, vert.y) && map.get(vert.x, vert.y).terrain.is_passable() {
            return Some(vert);
        }
    }

    None // stuck
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;

    #[test]
    fn step_toward_moves_closer() {
        let map = TileMap::new(10, 10, Terrain::Grass);
        let from = Position::new(0, 0);
        let to = Position::new(5, 5);
        let next = step_toward(&from, &to, &map).unwrap();
        assert_eq!(next, Position::new(1, 1));
    }

    #[test]
    fn step_toward_avoids_water() {
        let mut map = TileMap::new(10, 10, Terrain::Grass);
        map.set(1, 1, Terrain::Water); // block direct path
        let from = Position::new(0, 0);
        let to = Position::new(5, 5);
        let next = step_toward(&from, &to, &map).unwrap();
        // Should try horizontal or vertical instead of diagonal
        assert!(next == Position::new(1, 0) || next == Position::new(0, 1));
    }
}
```

- [ ] **Step 5: Run tests**

```bash
just test
```

- [ ] **Step 6: Commit**

```bash
jj new
jj describe -m "feat: add utility AI scoring, action selection, and basic pathfinding"
```

---

### Task 10: AI & Action Resolution Systems

**Files:**
- Create: `src/systems/ai.rs`, `src/systems/actions.rs`
- Modify: `src/systems/mod.rs`

- [ ] **Step 1: Implement AI system**

`src/systems/ai.rs`:

```rust
use bevy_ecs::prelude::*;
use crate::ai::{Action, CurrentAction};
use crate::ai::scoring::{score_actions, select_best_action};
use crate::components::physical::{Needs, Position};
use crate::components::personality::Personality;
use crate::resources::rng::SimRng;
use crate::resources::map::TileMap;
use rand::Rng;

/// Cats with no current action (or finished action) evaluate and pick a new one.
pub fn evaluate_actions(
    mut query: Query<(&Needs, &Personality, &Position, &mut CurrentAction)>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
) {
    for (needs, personality, pos, mut current) in &mut query {
        // Only re-evaluate if current action is done
        if current.ticks_remaining > 0 {
            continue;
        }

        // For Phase 1: food_available is simplified — assume stores exist
        let food_available = true;

        let scores = score_actions(needs, personality, food_available, &mut rng.0);
        let action = select_best_action(&scores);

        let (ticks, target) = match action {
            Action::Eat => (5, None),
            Action::Sleep => (20, None),
            Action::Wander => {
                // Pick a random nearby passable tile
                let dx = rng.0.gen_range(-5..=5);
                let dy = rng.0.gen_range(-5..=5);
                let target = Position::new(pos.x + dx, pos.y + dy);
                (10, Some(target))
            }
            Action::Idle => (5, None),
        };

        *current = CurrentAction {
            action,
            ticks_remaining: ticks,
            target_position: target,
        };
    }
}
```

- [ ] **Step 2: Implement action resolution**

`src/systems/actions.rs`:

```rust
use bevy_ecs::prelude::*;
use crate::ai::{Action, CurrentAction};
use crate::ai::pathfinding::step_toward;
use crate::components::physical::{Needs, Position};
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;

pub fn resolve_actions(
    mut query: Query<(Entity, &mut CurrentAction, &mut Needs, &mut Position)>,
    map: Res<TileMap>,
    mut log: ResMut<NarrativeLog>,
) {
    for (entity, mut current, mut needs, mut pos) in &mut query {
        if current.ticks_remaining == 0 {
            continue;
        }

        current.ticks_remaining -= 1;

        match current.action {
            Action::Eat => {
                // Gradually restore hunger over the action duration
                needs.hunger = (needs.hunger + 0.04).min(1.0);
            }
            Action::Sleep => {
                // Gradually restore energy
                needs.energy = (needs.energy + 0.02).min(1.0);
                needs.warmth = (needs.warmth + 0.01).min(1.0);
            }
            Action::Wander => {
                if let Some(target) = current.target_position {
                    if let Some(next) = step_toward(&pos, &target, &map) {
                        pos.x = next.x;
                        pos.y = next.y;
                    }
                }
            }
            Action::Idle => {
                // Nothing happens — but this generates micro-behavior narration
            }
        }
    }
}
```

- [ ] **Step 3: Create NarrativeLog resource**

`src/resources/narrative.rs`:

```rust
use bevy_ecs::prelude::*;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct NarrativeEntry {
    pub tick: u64,
    pub text: String,
    pub tier: NarrativeTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeTier {
    Micro,       // ambient flavor
    Action,      // action completion
    Significant, // rare important events
}

#[derive(Resource)]
pub struct NarrativeLog {
    pub entries: VecDeque<NarrativeEntry>,
    pub capacity: usize,
}

impl Default for NarrativeLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 200,
        }
    }
}

impl NarrativeLog {
    pub fn push(&mut self, tick: u64, text: String, tier: NarrativeTier) {
        self.entries.push_back(NarrativeEntry { tick, text, tier });
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}
```

- [ ] **Step 4: Update resources/mod.rs**

Add `pub mod narrative;` and `pub use narrative::*;`.

- [ ] **Step 5: Run tests**

```bash
just test
```

- [ ] **Step 6: Commit**

```bash
jj new
jj describe -m "feat: add AI evaluation, action resolution, and narrative log"
```

---

### Task 11: Basic Narrative Generation

**Files:**
- Create: `src/systems/narrative.rs`

- [ ] **Step 1: Implement simple narrative generation**

For Phase 1, this is hardcoded text (templates come in Phase 2). It generates text from completed actions.

`src/systems/narrative.rs`:

```rust
use bevy_ecs::prelude::*;
use crate::ai::{Action, CurrentAction};
use crate::components::identity::Name;
use crate::components::physical::Needs;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::time::TimeState;
use crate::resources::rng::SimRng;
use rand::Rng;

/// Generate narrative entries when actions complete.
pub fn generate_narrative(
    query: Query<(&Name, &CurrentAction, &Needs)>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    for (name, current, needs) in &query {
        // Only narrate on the last tick of an action
        if current.ticks_remaining != 1 {
            continue;
        }

        let text = match current.action {
            Action::Eat => {
                let options = [
                    format!("{} eats from the stores.", name.0),
                    format!("{} has a quick meal.", name.0),
                    format!("{} chews thoughtfully.", name.0),
                ];
                options[rng.0.gen_range(0..options.len())].clone()
            }
            Action::Sleep => {
                let options = [
                    format!("{} curls up and sleeps.", name.0),
                    format!("{} naps in a quiet corner.", name.0),
                    format!("{} dozes off.", name.0),
                ];
                options[rng.0.gen_range(0..options.len())].clone()
            }
            Action::Wander => {
                let options = [
                    format!("{} wanders about.", name.0),
                    format!("{} explores nearby.", name.0),
                    format!("{} stretches and strolls.", name.0),
                ];
                options[rng.0.gen_range(0..options.len())].clone()
            }
            Action::Idle => {
                // Rate-limit idle narration
                if rng.0.gen_range(0..5) != 0 {
                    continue;
                }
                let options = if needs.hunger < 0.3 {
                    vec![format!("{}'s stomach growls.", name.0)]
                } else if needs.energy < 0.3 {
                    vec![format!("{} yawns widely.", name.0)]
                } else {
                    vec![
                        format!("{} sits quietly.", name.0),
                        format!("{} grooms a paw.", name.0),
                        format!("{} watches the sky.", name.0),
                    ]
                };
                options[rng.0.gen_range(0..options.len())].clone()
            }
        };

        log.push(time.tick, text, NarrativeTier::Action);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
just test
```

- [ ] **Step 3: Commit**

```bash
jj new
jj describe -m "feat: add basic narrative generation for action completion"
```

---

### Task 12: TUI — Map, Log, and Status Widgets

**Files:**
- Create: `src/tui/mod.rs`, `src/tui/map.rs`, `src/tui/log.rs`, `src/tui/status.rs`

- [ ] **Step 1: Implement TUI app structure**

`src/tui/mod.rs`:

```rust
pub mod map;
pub mod log;
pub mod status;

use ratatui::prelude::*;
use ratatui::widgets::Block;
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::time::{TimeState, SimConfig};
use crate::resources::weather::WeatherState;
use crate::components::identity::Name;
use crate::components::physical::Position;

pub struct AppView<'a> {
    pub map: &'a TileMap,
    pub cat_positions: Vec<(&'a str, Position)>,
    pub narrative: &'a NarrativeLog,
    pub time: &'a TimeState,
    pub config: &'a SimConfig,
    pub weather: &'a WeatherState,
    pub cat_count: usize,
}

impl<'a> AppView<'a> {
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Split into main area and bottom status bar
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        // Split main area into map (left) and log (right)
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(vertical[0]);

        // Render widgets
        map::render_map(frame, horizontal[0], self.map, &self.cat_positions);
        log::render_log(frame, horizontal[1], self.narrative, self.time, self.config, self.weather, self.cat_count);
        status::render_status(frame, vertical[1], self.time, self.config);
    }
}
```

- [ ] **Step 2: Implement map widget**

`src/tui/map.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::resources::map::{TileMap, Terrain};
use crate::components::physical::Position;

pub fn render_map(
    frame: &mut Frame,
    area: Rect,
    map: &TileMap,
    cats: &[(&str, Position)],
) {
    let block = Block::default().borders(Borders::ALL).title(" Map ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate visible area based on terminal size
    let view_w = inner.width as i32;
    let view_h = inner.height as i32;

    // Center on map center (TODO: scroll to follow focused cat)
    let center_x = map.width / 2;
    let center_y = map.height / 2;
    let start_x = (center_x - view_w / 2).max(0);
    let start_y = (center_y - view_h / 2).max(0);

    let mut lines: Vec<Line> = Vec::new();

    for vy in 0..view_h.min(map.height) {
        let my = start_y + vy;
        if my >= map.height { break; }

        let mut spans: Vec<Span> = Vec::new();
        for vx in 0..view_w.min(map.width) {
            let mx = start_x + vx;
            if mx >= map.width { break; }

            // Check if a cat is here
            if let Some((name, _)) = cats.iter().find(|(_, p)| p.x == mx && p.y == my) {
                let initial = name.chars().next().unwrap_or('?');
                spans.push(Span::styled(
                    initial.to_string(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            } else {
                let tile = map.get(mx, my);
                let (ch, color) = terrain_display(tile.terrain);
                spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
            }
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn terrain_display(terrain: Terrain) -> (char, Color) {
    match terrain {
        Terrain::Grass => ('.', Color::Green),
        Terrain::LightForest => ('t', Color::DarkGreen),
        Terrain::DenseForest => ('T', Color::DarkGreen),
        Terrain::Water => ('~', Color::Blue),
        Terrain::Rock => ('#', Color::Gray),
        Terrain::Mud => (',', Color::DarkGreen),
        Terrain::Sand => (':', Color::Yellow),
        Terrain::Den => ('D', Color::LightMagenta),
        Terrain::Hearth => ('H', Color::LightRed),
        Terrain::Stores => ('S', Color::LightCyan),
        Terrain::Workshop => ('W', Color::Cyan),
        Terrain::Garden => ('G', Color::LightGreen),
        Terrain::FairyRing => ('*', Color::Magenta),
        Terrain::StandingStone => ('!', Color::White),
        Terrain::DeepPool => ('O', Color::DarkGreen),
        Terrain::AncientRuin => ('?', Color::DarkGreen),
    }
}
```

- [ ] **Step 3: Implement log widget**

`src/tui/log.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::time::{TimeState, SimConfig};
use crate::resources::weather::WeatherState;

pub fn render_log(
    frame: &mut Frame,
    area: Rect,
    log: &NarrativeLog,
    time: &TimeState,
    config: &SimConfig,
    weather: &WeatherState,
    cat_count: usize,
) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Header: day, season, weather, population
    let day = TimeState::day_number(time.tick, config);
    let season = time.season(config);
    let phase = time.day_phase(config);
    let header = format!(
        " Day {} — {} — {} — {} — {} cats",
        day, season.label(), phase.label(),
        weather.current.label(), cat_count,
    );

    let header_line = Line::from(Span::styled(
        header,
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));

    // Log entries — show most recent that fit
    let available_height = inner.height.saturating_sub(2) as usize; // header + separator
    let entries: Vec<Line> = log.entries
        .iter()
        .rev()
        .take(available_height)
        .map(|entry| {
            let style = match entry.tier {
                NarrativeTier::Micro => Style::default().fg(Color::DarkGray),
                NarrativeTier::Action => Style::default().fg(Color::White),
                NarrativeTier::Significant => Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            };
            Line::from(Span::styled(&entry.text, style))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut all_lines = vec![header_line, Line::from("─".repeat(inner.width as usize))];
    all_lines.extend(entries);

    let paragraph = Paragraph::new(all_lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
```

- [ ] **Step 4: Implement status bar**

`src/tui/status.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::resources::time::{TimeState, SimSpeed};
use crate::resources::time::SimConfig;

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    time: &TimeState,
    _config: &SimConfig,
) {
    let speed_label = time.speed.label();
    let pause_indicator = if time.paused { " PAUSED " } else { "" };

    let text = format!(
        " [S]peed: {}  [P]ause{}  [Q]uit",
        speed_label, pause_indicator,
    );

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Black).bg(Color::White));
    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 5: Run tests (compilation check)**

```bash
just build
```

- [ ] **Step 6: Commit**

```bash
jj new
jj describe -m "feat: add TUI widgets — map, narrative log, and status bar"
```

---

### Task 13: Main Loop — Wire Everything Together

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement the main loop**

Replace `src/main.rs`:

```rust
use std::io;
use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;
use ratatui::backend::CrosstermBackend;

use bevy_ecs::prelude::*;

use clowder::components::identity::*;
use clowder::components::personality::*;
use clowder::components::physical::*;
use clowder::components::mental::*;
use clowder::components::skills::*;
use clowder::ai::CurrentAction;
use clowder::resources::time::*;
use clowder::resources::map::*;
use clowder::resources::weather::*;
use clowder::resources::narrative::*;
use clowder::resources::rng::*;
use clowder::world_gen::terrain::generate_terrain;
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats};
use clowder::tui::AppView;

fn main() -> io::Result<()> {
    // Parse seed from args
    let seed: u64 = std::env::args()
        .skip_while(|a| a != "--seed")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup ECS world
    let config = SimConfig { seed, ..Default::default() };
    let mut sim_rng = SimRng::new(seed);

    // Generate world
    let mut map = generate_terrain(80, 60, &mut sim_rng.0);
    let colony_site = find_colony_site(&map, &mut sim_rng.0);

    // Place starting structures
    map.set(colony_site.x, colony_site.y, Terrain::Hearth);
    map.set(colony_site.x - 2, colony_site.y, Terrain::Den);
    map.set(colony_site.x + 2, colony_site.y, Terrain::Stores);

    // Generate cats
    let cat_blueprints = generate_starting_cats(8, &mut sim_rng.0);

    let mut world = World::new();
    world.insert_resource(TimeState::default());
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    // Spawn cats
    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2;
        let offset_y = (i as i32 / 5) - 1;
        world.spawn((
            Name(cat.name),
            Species::Cat,
            Age { born_tick: 0 },
            cat.gender,
            cat.orientation,
            cat.personality,
            cat.appearance,
            Position::new(colony_site.x + offset_x, colony_site.y + offset_y),
            Health::default(),
            Needs::default(),
            Mood::default(),
            Memory::default(),
            cat.skills,
            MagicAffinity(cat.magic_affinity),
            Corruption(0.0),
            Training::default(),
            CurrentAction::default(),
        ));
    }

    // Build schedule
    let mut schedule = Schedule::default();
    schedule.add_systems((
        clowder::systems::time::advance_time,
        clowder::systems::weather::update_weather,
        clowder::systems::needs::decay_needs,
        clowder::systems::ai::evaluate_actions,
        clowder::systems::actions::resolve_actions,
        clowder::systems::narrative::generate_narrative,
    ).chain());

    // Main loop
    let target_frame_time = Duration::from_millis(33); // ~30fps

    // Add initial log entry
    {
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(0, "A small group of cats settles in a clearing.".to_string(), NarrativeTier::Significant);
    }

    loop {
        let frame_start = Instant::now();

        // Handle input
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('p') => {
                            let mut time = world.resource_mut::<TimeState>();
                            time.paused = !time.paused;
                        }
                        KeyCode::Char('s') => {
                            let mut time = world.resource_mut::<TimeState>();
                            time.speed = time.speed.cycle();
                        }
                        _ => {}
                    }
                }
            }
        }

        // Tick simulation
        let ticks = {
            let time = world.resource::<TimeState>();
            if time.paused { 0 } else { time.speed.ticks_per_update() }
        };
        for _ in 0..ticks {
            schedule.run(&mut world);
        }

        // Render
        terminal.draw(|frame| {
            let map = world.resource::<TileMap>();
            let narrative = world.resource::<NarrativeLog>();
            let time = world.resource::<TimeState>();
            let sim_config = world.resource::<SimConfig>();
            let weather = world.resource::<WeatherState>();

            let mut cat_positions = Vec::new();
            let mut cat_count = 0;
            let mut query = world.query::<(&Name, &Position)>();
            for (name, pos) in query.iter(&world) {
                cat_positions.push((name.0.as_str(), *pos));
                cat_count += 1;
            }

            let view = AppView {
                map,
                cat_positions,
                narrative,
                time,
                config: sim_config,
                weather,
                cat_count,
            };
            view.render(frame);
        })?;

        // Frame timing
        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
```

- [ ] **Step 2: Build and run**

```bash
just run
```

Expected: terminal shows a map with terrain, 8 cat initials scattered near the colony center, a narrative log that starts updating as cats eat/sleep/wander, and a status bar. Press `s` to cycle speed, `p` to pause, `q` to quit.

- [ ] **Step 3: Test determinism**

```bash
# Run twice with same seed, watch first few narrative entries
just seed 42
# ... note first 3-4 narrative entries, quit
just seed 42
# ... should see identical entries
```

- [ ] **Step 4: Commit**

```bash
jj new
jj describe -m "feat: wire main loop — ECS schedule, TUI rendering, input handling"
```

---

### Task 14: Integration Smoke Test

**Files:**
- Create: `tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
use bevy_ecs::prelude::*;
use clowder::components::identity::*;
use clowder::components::personality::*;
use clowder::components::physical::*;
use clowder::components::mental::*;
use clowder::components::skills::*;
use clowder::ai::CurrentAction;
use clowder::resources::time::*;
use clowder::resources::map::*;
use clowder::resources::weather::*;
use clowder::resources::narrative::*;
use clowder::resources::rng::*;
use clowder::world_gen::terrain::generate_terrain;
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats};

#[test]
fn simulation_is_deterministic() {
    let log1 = run_simulation(42, 100);
    let log2 = run_simulation(42, 100);

    assert_eq!(log1.len(), log2.len(), "same seed should produce same number of events");
    for (a, b) in log1.iter().zip(log2.iter()) {
        assert_eq!(a, b, "narrative entries should match");
    }
}

#[test]
fn cats_eat_when_hungry() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();

    // Make all cats very hungry
    let mut query = world.query::<&mut Needs>();
    for mut needs in query.iter_mut(&mut world) {
        needs.hunger = 0.1;
    }

    // Run some ticks
    for _ in 0..50 {
        schedule.run(&mut world);
    }

    // At least one cat should have eaten (hunger > 0.1)
    let mut any_ate = false;
    let mut query = world.query::<&Needs>();
    for needs in query.iter(&world) {
        if needs.hunger > 0.15 {
            any_ate = true;
        }
    }
    assert!(any_ate, "at least one hungry cat should have eaten");
}

#[test]
fn simulation_runs_1000_ticks_without_panic() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();
    for _ in 0..1000 {
        schedule.run(&mut world);
    }
    // If we get here without panic, the test passes
}

fn run_simulation(seed: u64, ticks: usize) -> Vec<String> {
    let mut world = setup_world(seed);
    let mut schedule = build_schedule();
    for _ in 0..ticks {
        schedule.run(&mut world);
    }
    world.resource::<NarrativeLog>()
        .entries.iter().map(|e| e.text.clone()).collect()
}

fn setup_world(seed: u64) -> World {
    let config = SimConfig { seed, ..Default::default() };
    let mut rng = SimRng::new(seed);
    let mut map = generate_terrain(80, 60, &mut rng.0);
    let site = find_colony_site(&map, &mut rng.0);
    map.set(site.x, site.y, Terrain::Hearth);
    map.set(site.x - 2, site.y, Terrain::Den);
    map.set(site.x + 2, site.y, Terrain::Stores);

    let cats = generate_starting_cats(8, &mut rng.0);

    let mut world = World::new();
    world.insert_resource(TimeState::default());
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(map);
    world.insert_resource(rng);

    for (i, cat) in cats.into_iter().enumerate() {
        let ox = (i as i32 % 5) - 2;
        let oy = (i as i32 / 5) - 1;
        world.spawn((
            Name(cat.name),
            Species::Cat,
            Age { born_tick: 0 },
            cat.gender,
            cat.orientation,
            cat.personality,
            cat.appearance,
            Position::new(site.x + ox, site.y + oy),
            Health::default(),
            Needs::default(),
            Mood::default(),
            Memory::default(),
            cat.skills,
            MagicAffinity(cat.magic_affinity),
            Corruption(0.0),
            Training::default(),
            CurrentAction::default(),
        ));
    }

    world
}

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        clowder::systems::time::advance_time,
        clowder::systems::weather::update_weather,
        clowder::systems::needs::decay_needs,
        clowder::systems::ai::evaluate_actions,
        clowder::systems::actions::resolve_actions,
        clowder::systems::narrative::generate_narrative,
    ).chain());
    schedule
}
```

- [ ] **Step 2: Run integration tests**

```bash
just test
```

Expected: all tests pass — determinism verified, hungry cats eat, 1000 ticks without panic.

- [ ] **Step 3: Commit**

```bash
jj new
jj describe -m "test: add integration tests — determinism, hunger behavior, stability"
```

---

## Post-Implementation Checklist

After all tasks are complete:

- [ ] `just ci` passes (check + clippy + test)
- [ ] `just run` shows cats on a map, narrative flowing, speed/pause controls work
- [ ] `just seed 42` produces identical first 10 narrative entries on consecutive runs
- [ ] All docs/systems/ stubs exist with populated parameter tables
- [ ] CLAUDE.md is accurate and complete
