# Phase 1: Material Chain & Ecosystem — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace abstract food production with a real material chain — prey are entities, hunting produces items, food stores hold actual items — and build a self-regulating ecosystem with needs-based wildlife AI.

**Architecture:** Items are ECS entities with an `Item` component and a location enum (carried, on ground, in building). The existing `FoodStores` float resource is replaced by querying food items stored in the Stores building. Prey species are new wildlife entities with simplified needs-based AI. Hunting targets a specific prey entity, kills it, and produces a raw food item. The old herb `Inventory` is widened to hold generic items. Migration uses a strangler-fig pattern — new systems run alongside old ones, then old code is removed once verified.

**Tech Stack:** Rust, bevy_ecs (standalone ECS), rand_chacha (deterministic RNG)

**Design spec:** `docs/superpowers/specs/2026-04-06-systems-rework-design.md`

---

## File Structure

### New files
| File | Responsibility |
|------|---------------|
| `src/components/items.rs` | `Item` component, `ItemKind` enum, `ItemLocation` enum, quality/decay types |
| `src/systems/items.rs` | Item decay system, item spatial helpers |
| `src/components/prey.rs` | `PreyAnimal` component, `PreySpecies` enum |
| `src/systems/prey.rs` | Prey AI, population dynamics, breeding, density pressure |
| `tests/ecosystem.rs` | Integration tests for prey populations and food web |

### Modified files
| File | Changes |
|------|---------|
| `src/components/mod.rs` | Declare + re-export `items` and `prey` modules |
| `src/systems/mod.rs` | Declare `items` and `prey` modules |
| `src/components/magic.rs` | Refactor `Inventory` from `Vec<HerbKind>` to `Vec<ItemSlot>` |
| `src/components/building.rs` | Add `StoredItems` component to buildings |
| `src/resources/food.rs` | Deprecate `FoodStores`, add `FoodSupply` query helper |
| `src/systems/actions.rs` | Hunt/Forage produce items; Eat consumes items |
| `src/ai/scoring.rs` | `ScoringContext` fields derived from item queries |
| `src/components/wildlife.rs` | Add prey species to `WildSpecies`, extend AI states |
| `src/systems/wildlife.rs` | Prey spawning, predator hunting prey entities |
| `src/world_gen/colony.rs` | Spawn initial prey populations |
| `src/main.rs` | Register new systems in schedule |
| `src/lib.rs` | No changes (components/systems already declared) |

---

## Task 1: Item Component and Types

**Files:**
- Create: `src/components/items.rs`
- Modify: `src/components/mod.rs`

- [ ] **Step 1: Write unit tests for Item**

Create `src/components/items.rs` with the test module first:

```rust
use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemKind {
    // Raw prey
    RawMouse,
    RawRat,
    RawFish,
    RawBird,
    // Foraged
    Berries,
    Nuts,
    Roots,
    WildOnion,
    Mushroom,
    Moss,
    DriedGrass,
    Feather,
    // Herbs (mirror existing HerbKind for now; full merge in later task)
    HerbHealingMoss,
    HerbMoonpetal,
    HerbCalmroot,
    HerbThornbriar,
    HerbDreamroot,
    // Curiosities
    ShinyPebble,
    GlassShard,
    ColorfulShell,
}

impl ItemKind {
    /// Base decay rate per tick. 0.0 means no decay.
    pub fn decay_rate(self) -> f32 {
        match self {
            // Raw prey spoils fast
            Self::RawMouse | Self::RawRat | Self::RawFish | Self::RawBird => 0.01,
            // Foraged organic items decay moderately
            Self::Berries | Self::Nuts | Self::Roots | Self::WildOnion
            | Self::Mushroom | Self::Moss | Self::DriedGrass | Self::Feather => 0.005,
            // Herbs use existing rates (moderate)
            Self::HerbHealingMoss | Self::HerbMoonpetal | Self::HerbCalmroot
            | Self::HerbThornbriar | Self::HerbDreamroot => 0.003,
            // Inorganic items don't decay
            Self::ShinyPebble | Self::GlassShard | Self::ColorfulShell => 0.0,
        }
    }

    /// Whether this item can be eaten as food.
    pub fn is_food(self) -> bool {
        matches!(
            self,
            Self::RawMouse | Self::RawRat | Self::RawFish | Self::RawBird
                | Self::Berries | Self::Nuts | Self::Roots | Self::WildOnion
                | Self::Mushroom
        )
    }

    /// Hunger satisfaction when eaten (per item, not per tick).
    pub fn food_value(self) -> f32 {
        match self {
            Self::RawRat => 0.4,
            Self::RawMouse => 0.25,
            Self::RawFish => 0.35,
            Self::RawBird => 0.3,
            Self::Berries => 0.1,
            Self::Nuts => 0.15,
            Self::Roots | Self::WildOnion | Self::Mushroom => 0.1,
            _ => 0.0,
        }
    }
}

/// Where an item currently exists in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemLocation {
    /// Carried by a cat entity.
    Carried(Entity),
    /// On the ground at a position (item entity also has a Position component).
    OnGround,
    /// Stored inside a building entity.
    StoredIn(Entity),
}

/// A physical item in the simulation.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    /// 0.0–1.0; derived from harvester/crafter skill.
    pub quality: f32,
    /// 1.0 = fresh, 0.0 = destroyed. Decays over time.
    pub condition: f32,
    /// Where this item currently is.
    pub location: ItemLocation,
}

impl Item {
    pub fn new(kind: ItemKind, quality: f32, location: ItemLocation) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
        }
    }

    /// Tick decay. Returns true if item is destroyed (condition <= 0).
    pub fn tick_decay(&mut self) -> bool {
        let rate = self.kind.decay_rate();
        if rate > 0.0 {
            self.condition -= rate;
        }
        self.condition <= 0.0
    }

    pub fn is_destroyed(&self) -> bool {
        self.condition <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_prey_is_food() {
        assert!(ItemKind::RawMouse.is_food());
        assert!(ItemKind::RawFish.is_food());
        assert!(!ItemKind::ShinyPebble.is_food());
        assert!(!ItemKind::HerbMoonpetal.is_food());
    }

    #[test]
    fn item_decays_over_time() {
        let mut item = Item::new(ItemKind::RawFish, 0.5, ItemLocation::OnGround);
        assert!((item.condition - 1.0).abs() < f32::EPSILON);

        // Fish decays at 0.01/tick — should be destroyed after 100 ticks
        for _ in 0..99 {
            assert!(!item.tick_decay());
        }
        assert!(item.tick_decay()); // tick 100: condition hits 0.0
    }

    #[test]
    fn inorganic_items_do_not_decay() {
        let mut item = Item::new(ItemKind::ShinyPebble, 0.8, ItemLocation::OnGround);
        for _ in 0..1000 {
            assert!(!item.tick_decay());
        }
        assert!((item.condition - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn quality_is_clamped() {
        let item = Item::new(ItemKind::RawMouse, 1.5, ItemLocation::OnGround);
        assert!((item.quality - 1.0).abs() < f32::EPSILON);

        let item = Item::new(ItemKind::RawMouse, -0.5, ItemLocation::OnGround);
        assert!(item.quality.abs() < f32::EPSILON);
    }

    #[test]
    fn food_values_are_positive_for_food_items() {
        for kind in [
            ItemKind::RawMouse, ItemKind::RawRat, ItemKind::RawFish,
            ItemKind::RawBird, ItemKind::Berries, ItemKind::Nuts,
        ] {
            assert!(kind.food_value() > 0.0, "{kind:?} should have positive food value");
        }
    }
}
```

- [ ] **Step 2: Register the module**

Add to `src/components/mod.rs`:
```rust
pub mod items;
```

And add re-exports:
```rust
pub use items::{Item, ItemKind, ItemLocation};
```

- [ ] **Step 3: Run tests**

Run: `just test`
Expected: All tests pass including the new `items::tests` module.

- [ ] **Step 4: Commit**

```bash
jj new -m "feat: add Item component, ItemKind enum, and ItemLocation"
```

---

## Task 2: Item Decay System

**Files:**
- Create: `src/systems/items.rs`
- Modify: `src/systems/mod.rs`

- [ ] **Step 1: Write the item decay system**

Create `src/systems/items.rs`:

```rust
use bevy_ecs::prelude::*;

use crate::components::items::Item;

/// Tick item condition decay. Despawn destroyed items.
pub fn decay_items(mut commands: Commands, mut items: Query<(Entity, &mut Item)>) {
    for (entity, mut item) in items.iter_mut() {
        if item.tick_decay() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::{ItemKind, ItemLocation};

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_items);
        (world, schedule)
    }

    #[test]
    fn destroyed_items_are_despawned() {
        let (mut world, mut schedule) = setup();

        // Spawn an item with condition almost gone
        let entity = world
            .spawn(Item {
                kind: ItemKind::RawFish,
                quality: 0.5,
                condition: 0.005, // Will hit 0 after 1 tick at 0.01 rate
                location: ItemLocation::OnGround,
            })
            .id();

        schedule.run(&mut world);

        // Entity should be despawned
        assert!(
            world.get::<Item>(entity).is_none(),
            "destroyed item should be despawned"
        );
    }

    #[test]
    fn healthy_items_survive() {
        let (mut world, mut schedule) = setup();

        let entity = world
            .spawn(Item::new(ItemKind::RawFish, 0.5, ItemLocation::OnGround))
            .id();

        schedule.run(&mut world);

        let item = world.get::<Item>(entity).expect("item should still exist");
        assert!(
            item.condition < 1.0 && item.condition > 0.0,
            "item should have decayed but not be destroyed"
        );
    }
}
```

- [ ] **Step 2: Register the module**

Add to `src/systems/mod.rs`:
```rust
pub mod items;
```

- [ ] **Step 3: Run tests**

Run: `just test`
Expected: All tests pass.

- [ ] **Step 4: Wire into schedule**

In `src/main.rs`, add `decay_items` to the world simulation chain (after weather, before cat needs):

```rust
use crate::systems::items::decay_items;
```

Add it to the world simulation chain in `build_schedule()`.

- [ ] **Step 5: Commit**

```bash
jj new -m "feat: add item decay system — despawns destroyed items each tick"
```

---

## Task 3: Building Item Storage

**Files:**
- Modify: `src/components/building.rs`

- [ ] **Step 1: Add StoredItems component**

Add to `src/components/building.rs`:

```rust
/// Tracks items stored inside a building. Capacity depends on building type.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredItems {
    pub items: Vec<Entity>,
}

impl StoredItems {
    pub fn capacity(kind: StructureType) -> usize {
        match kind {
            StructureType::Stores => 30,
            StructureType::Den => 5,
            StructureType::Workshop => 10,
            _ => 0,
        }
    }

    pub fn is_full(&self, kind: StructureType) -> bool {
        self.items.len() >= Self::capacity(kind)
    }

    pub fn add(&mut self, item: Entity, kind: StructureType) -> bool {
        if self.is_full(kind) {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, item: Entity) -> bool {
        if let Some(idx) = self.items.iter().position(|&e| e == item) {
            self.items.swap_remove(idx);
            true
        } else {
            false
        }
    }
}
```

- [ ] **Step 2: Add re-export**

In `src/components/mod.rs`, add `StoredItems` to the building re-exports:
```rust
pub use building::{ConstructionSite, CropState, GateState, StoredItems, Structure, StructureType};
```

- [ ] **Step 3: Add StoredItems to existing Stores buildings on spawn**

In `src/world_gen/colony.rs`, wherever Stores buildings are spawned, add the `StoredItems::default()` component to the entity bundle.

- [ ] **Step 4: Write unit tests**

Add to `src/components/building.rs` `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_has_capacity_30() {
        assert_eq!(StoredItems::capacity(StructureType::Stores), 30);
    }

    #[test]
    fn den_has_capacity_5() {
        assert_eq!(StoredItems::capacity(StructureType::Den), 5);
    }

    #[test]
    fn wall_has_no_storage() {
        assert_eq!(StoredItems::capacity(StructureType::Wall), 0);
    }

    #[test]
    fn add_respects_capacity() {
        let mut world = World::new();
        let item_a = world.spawn_empty().id();
        let item_b = world.spawn_empty().id();

        let mut stored = StoredItems::default();
        // Wall has 0 capacity
        assert!(!stored.add(item_a, StructureType::Wall));
        // Stores has 30
        assert!(stored.add(item_a, StructureType::Stores));
        assert!(stored.add(item_b, StructureType::Stores));
        assert_eq!(stored.items.len(), 2);
    }

    #[test]
    fn remove_returns_false_for_missing() {
        let mut world = World::new();
        let item = world.spawn_empty().id();
        let mut stored = StoredItems::default();
        assert!(!stored.remove(item));
    }
}
```

- [ ] **Step 5: Run tests and commit**

Run: `just test`

```bash
jj new -m "feat: add StoredItems component for building item storage"
```

---

## Task 4: Prey Species Components

**Files:**
- Create: `src/components/prey.rs`
- Modify: `src/components/mod.rs`

- [ ] **Step 1: Define prey types**

Create `src/components/prey.rs`:

```rust
use bevy_ecs::prelude::*;

use crate::resources::map::Terrain;

/// Species of prey animal in the ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PreySpecies {
    Mouse,
    Rat,
    Fish,
    Bird,
}

impl PreySpecies {
    pub fn symbol(self) -> char {
        match self {
            Self::Mouse => 'm',
            Self::Rat => 'r',
            Self::Fish => '~',
            Self::Bird => 'b',
        }
    }

    /// Breeding rate per tick when food is available.
    pub fn breed_rate(self) -> f32 {
        match self {
            Self::Mouse => 0.003,
            Self::Rat => 0.005,
            Self::Fish => 0.002,
            Self::Bird => 0.001,
        }
    }

    /// Maximum population per map.
    pub fn population_cap(self) -> usize {
        match self {
            Self::Mouse => 30,
            Self::Rat => 50,
            Self::Fish => 20,
            Self::Bird => 15,
        }
    }

    /// Valid habitat terrain for this species.
    pub fn habitat(self) -> &'static [Terrain] {
        match self {
            Self::Mouse => &[Terrain::Grass, Terrain::LightForest],
            Self::Rat => &[Terrain::Grass, Terrain::LightForest, Terrain::DenseForest],
            Self::Fish => &[Terrain::Water],
            Self::Bird => &[Terrain::Grass, Terrain::LightForest],
        }
    }

    /// What ItemKind this prey becomes when killed.
    pub fn item_kind(self) -> crate::components::items::ItemKind {
        match self {
            Self::Mouse => crate::components::items::ItemKind::RawMouse,
            Self::Rat => crate::components::items::ItemKind::RawRat,
            Self::Fish => crate::components::items::ItemKind::RawFish,
            Self::Bird => crate::components::items::ItemKind::RawBird,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Rat => "rat",
            Self::Fish => "fish",
            Self::Bird => "bird",
        }
    }
}

/// Component marking an entity as a prey animal.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyAnimal {
    pub species: PreySpecies,
    pub hunger: f32, // 0.0 = full, 1.0 = starving
}

impl PreyAnimal {
    pub fn new(species: PreySpecies) -> Self {
        Self {
            species,
            hunger: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::ItemKind;

    #[test]
    fn prey_maps_to_correct_item() {
        assert_eq!(PreySpecies::Mouse.item_kind(), ItemKind::RawMouse);
        assert_eq!(PreySpecies::Fish.item_kind(), ItemKind::RawFish);
    }

    #[test]
    fn population_caps_are_reasonable() {
        assert!(PreySpecies::Rat.population_cap() > PreySpecies::Mouse.population_cap());
        assert!(PreySpecies::Bird.population_cap() < PreySpecies::Mouse.population_cap());
    }
}
```

- [ ] **Step 2: Register module**

Add to `src/components/mod.rs`:
```rust
pub mod prey;
```

And re-exports:
```rust
pub use prey::{PreyAnimal, PreySpecies};
```

- [ ] **Step 3: Run tests and commit**

Run: `just test`

```bash
jj new -m "feat: add PreyAnimal component and PreySpecies enum"
```

---

## Task 5: Prey Spawning and Population Dynamics

**Files:**
- Create: `src/systems/prey.rs`
- Modify: `src/systems/mod.rs`
- Modify: `src/world_gen/colony.rs`

- [ ] **Step 1: Write prey population system**

Create `src/systems/prey.rs`:

```rust
use bevy_ecs::prelude::*;

use crate::components::physical::{Health, Position};
use crate::components::prey::{PreyAnimal, PreySpecies};
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::rng::SimRng;

/// Count living prey of each species.
fn population_count(prey_query: &Query<&PreyAnimal>) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for prey in prey_query.iter() {
        let idx = match prey.species {
            PreySpecies::Mouse => 0,
            PreySpecies::Rat => 1,
            PreySpecies::Fish => 2,
            PreySpecies::Bird => 3,
        };
        counts[idx] += 1;
    }
    counts
}

/// Breed prey based on population density and food availability.
pub fn prey_population(
    mut commands: Commands,
    prey_query: Query<&PreyAnimal>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
) {
    let counts = population_count(&prey_query);

    for (idx, &species) in [
        PreySpecies::Mouse,
        PreySpecies::Rat,
        PreySpecies::Fish,
        PreySpecies::Bird,
    ]
    .iter()
    .enumerate()
    {
        let pop = counts[idx];
        let cap = species.population_cap();

        // Density pressure: 1.0 at empty, 0.0 at cap
        let density_pressure = 1.0 - (pop as f32 / cap as f32);

        if density_pressure <= 0.0 {
            // At or over carrying capacity — log it
            if rng.rng.random::<f32>() < 0.001 {
                log.push_micro(format!(
                    "The {} have overrun their territory — they fight over scraps.",
                    species.name()
                ));
            }
            continue;
        }

        if density_pressure < 0.2 && rng.rng.random::<f32>() < 0.002 {
            log.push_micro(format!(
                "The {} are growing restless — too many mouths, not enough food.",
                species.name()
            ));
        }

        // Breeding check
        // food_availability approximated as 1.0 for now (terrain-based)
        let food_availability = 1.0;
        let breed_chance = species.breed_rate() * food_availability * density_pressure;

        if rng.rng.random::<f32>() < breed_chance {
            // Find a valid habitat tile to spawn on
            if let Some(pos) = find_habitat_tile(*species, &map, &mut rng) {
                commands.spawn((
                    PreyAnimal::new(*species),
                    Health::default(),
                    pos,
                ));
            }
        }
    }
}

/// Prey starve when hunger is critical.
pub fn prey_hunger(
    mut commands: Commands,
    mut prey_query: Query<(Entity, &mut PreyAnimal, &mut Health)>,
    prey_count: Query<&PreyAnimal>,
) {
    let counts = population_count(&prey_count);

    for (entity, mut prey, mut health) in prey_query.iter_mut() {
        let idx = match prey.species {
            PreySpecies::Mouse => 0,
            PreySpecies::Rat => 1,
            PreySpecies::Fish => 2,
            PreySpecies::Bird => 3,
        };
        let pop = counts[idx];
        let cap = prey.species.population_cap();

        // Base hunger increase
        prey.hunger = (prey.hunger + 0.002).min(1.0);

        // Overcrowding stress: extra hunger when near cap
        if pop as f32 / cap as f32 > 0.8 {
            prey.hunger = (prey.hunger + 0.001).min(1.0);
        }

        // Fish "eat" implicitly from water; mice/birds forage terrain
        // Simplified: hunger recovers when on appropriate terrain
        // (Full implementation will check terrain; for now, slow constant recovery)
        prey.hunger = (prey.hunger - 0.003).max(0.0);

        // Starvation
        if prey.hunger > 0.9 {
            health.current -= 0.01;
            if health.current <= 0.0 {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn find_habitat_tile(
    species: PreySpecies,
    map: &TileMap,
    rng: &mut SimRng,
) -> Option<Position> {
    let habitat = species.habitat();
    // Try 50 random tiles
    for _ in 0..50 {
        let x = rng.rng.random_range(0..map.width as i32);
        let y = rng.rng.random_range(0..map.height as i32);
        let tile = map.get(x, y);
        if habitat.contains(&tile.terrain) {
            return Some(Position::new(x, y));
        }
    }
    None
}

/// Spawn initial prey populations during world generation.
pub fn spawn_initial_prey(
    commands: &mut Commands,
    map: &TileMap,
    rng: &mut SimRng,
) {
    for species in [
        PreySpecies::Mouse,
        PreySpecies::Rat,
        PreySpecies::Fish,
        PreySpecies::Bird,
    ] {
        let initial_count = species.population_cap() / 3; // Start at ~33% capacity
        let mut spawned = 0;
        for _ in 0..initial_count * 10 {
            // Try many times; habitat may be scarce
            if spawned >= initial_count {
                break;
            }
            if let Some(pos) = find_habitat_tile(species, map, rng) {
                commands.spawn((
                    PreyAnimal::new(species),
                    Health::default(),
                    pos,
                ));
                spawned += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, crate::resources::map::Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(NarrativeLog::default());

        let mut schedule = Schedule::default();
        schedule.add_systems((prey_population, prey_hunger).chain());
        (world, schedule)
    }

    #[test]
    fn prey_breed_when_below_cap() {
        let (mut world, mut schedule) = setup();

        // Spawn 5 mice (cap is 30) — should breed
        for i in 0..5 {
            world.spawn((
                PreyAnimal::new(PreySpecies::Mouse),
                Health::default(),
                Position::new(i, 0),
            ));
        }

        // Run many ticks to give breeding a chance
        for _ in 0..200 {
            schedule.run(&mut world);
        }

        let count = world.query::<&PreyAnimal>().iter(&world).count();
        assert!(count > 5, "mice should have bred; got {count}");
    }

    #[test]
    fn prey_do_not_exceed_cap() {
        let (mut world, mut schedule) = setup();

        // Spawn mice at cap
        for i in 0..30 {
            world.spawn((
                PreyAnimal::new(PreySpecies::Mouse),
                Health::default(),
                Position::new(i % 20, i / 20),
            ));
        }

        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyAnimal>()
            .iter(&world)
            .filter(|p| p.species == PreySpecies::Mouse)
            .count();
        assert!(count <= 30, "mice should not exceed cap; got {count}");
    }
}
```

- [ ] **Step 2: Register module and wire into schedule**

Add to `src/systems/mod.rs`:
```rust
pub mod prey;
```

In `src/main.rs` `build_schedule()`, add after `spawn_wildlife`:
```rust
crate::systems::prey::prey_population,
crate::systems::prey::prey_hunger,
```

- [ ] **Step 3: Add initial prey spawning to world gen**

In `src/world_gen/colony.rs` or wherever `build_new_world` is, after spawning initial wildlife, add:
```rust
crate::systems::prey::spawn_initial_prey(&mut commands, &map, &mut rng);
```

Where `commands` is obtained from the world using `world.commands()` or by passing `&mut world` to a helper.

- [ ] **Step 4: Run tests and commit**

Run: `just test`

```bash
jj new -m "feat: add prey population dynamics — breeding, hunger, density pressure"
```

---

## Task 6: Hunt Targets Real Prey

**Files:**
- Modify: `src/systems/actions.rs`
- Modify: `src/ai/scoring.rs`

This is the core migration: hunting kills a prey entity and produces a food item. For backward compatibility during migration, also deposit to old FoodStores.

- [ ] **Step 1: Add prey-targeting to Hunt action**

In `src/systems/actions.rs`, the Hunt resolution block (lines ~170-209) currently does:
```rust
let success = rng.rng.random::<f32>() < 0.25 + skills.hunting * 0.55;
if success {
    let yield_amount = 1.0 + skills.hunting * 2.0;
    food.deposit(yield_amount);
    ...
}
```

Replace with prey-targeting logic. The system needs access to prey entities:

```rust
Action::Hunt => {
    // Move toward hunting ground.
    if let Some(target) = current.target_position {
        if pos.manhattan_distance(&target) > 1 {
            if let Some(next) = step_toward(&pos, &target, &map) {
                *pos = next;
            }
        }
    }

    // On last tick: resolve the hunt.
    if current.ticks_remaining == 0 {
        let success = rng.rng.random::<f32>() < 0.25 + skills.hunting * 0.55;
        if success {
            // Find nearest prey entity within 3 tiles
            let mut nearest_prey: Option<(Entity, f32)> = None;
            for (prey_entity, prey, prey_pos) in prey_query.iter() {
                let dist = pos.manhattan_distance(prey_pos) as f32;
                if dist <= 3.0 {
                    if nearest_prey.is_none() || dist < nearest_prey.unwrap().1 {
                        nearest_prey = Some((prey_entity, dist));
                    }
                }
            }

            if let Some((prey_entity, _)) = nearest_prey {
                // Kill the prey
                let prey = prey_query.get(prey_entity).unwrap().0;
                let item_kind = prey.species.item_kind();
                let quality = 0.3 + skills.hunting * 0.4; // Skill affects quality

                // Despawn prey entity
                commands.entity(prey_entity).despawn();

                // Spawn food item in cat's inventory location
                commands.spawn(Item::new(
                    item_kind,
                    quality,
                    ItemLocation::Carried(entity),
                ));

                // Also deposit to old FoodStores for backward compat
                let yield_amount = item_kind.food_value();
                food.deposit(yield_amount);

                memory.remember(MemoryEntry {
                    event_type: MemoryType::ResourceFound,
                    location: current.target_position,
                    involved: vec![],
                    tick: time.tick,
                    strength: 1.0,
                    firsthand: true,
                });
                mood.modifiers.push_back(MoodModifier {
                    amount: 0.1,
                    ticks_remaining: 30,
                    source: "successful hunt".to_string(),
                });
            } else {
                // No prey nearby — hunt fails even if skill check passed
                mood.modifiers.push_back(MoodModifier {
                    amount: -0.05,
                    ticks_remaining: 20,
                    source: "failed hunt".to_string(),
                });
            }
        } else {
            mood.modifiers.push_back(MoodModifier {
                amount: -0.05,
                ticks_remaining: 20,
                source: "failed hunt".to_string(),
            });
        }
        skills.hunting += skills.growth_rate() * 0.02;
    }
}
```

The system function signature needs to add `prey_query`:
```rust
prey_query: Query<(&PreyAnimal, &Position), Without<Dead>>,
```

And `commands: Commands` plus `Item`/`ItemLocation` imports.

- [ ] **Step 2: Update scoring context for prey availability**

In `src/ai/scoring.rs`, add a field to `ScoringContext`:
```rust
pub prey_nearby: bool,     // Any prey within hunting range
pub prey_count_nearby: usize, // How many prey within range
```

Populate it in the context builder by querying prey positions relative to the cat. Update Hunt scoring to consider `prey_nearby`:

```rust
// Hunt: only attractive if prey exists nearby
if ctx.can_hunt && ctx.prey_nearby {
    let food_scarcity = (1.0 - ctx.food_fraction) * 0.5;
    let urgency = ((1.0 - ctx.needs.hunger) + food_scarcity)
        * ctx.personality.boldness * 1.5
        * ctx.needs.level_suppression(1);
    scores.push((Action::Hunt, urgency + jitter(rng)));
}
```

- [ ] **Step 3: Run tests and commit**

Run: `just test`
Fix any compilation errors from added query parameters.

```bash
jj new -m "feat: hunt targets real prey entities — kills prey, produces food item"
```

---

## Task 7: Forage Produces Items

**Files:**
- Modify: `src/systems/actions.rs`

- [ ] **Step 1: Modify Forage to spawn item entities**

In the Forage resolution block, alongside depositing to FoodStores, spawn an item entity:

```rust
Action::Forage => {
    if let Some(target) = current.target_position {
        if pos.manhattan_distance(&target) > 1 {
            if let Some(next) = step_toward(&pos, &target, &map) {
                *pos = next;
            }
        } else {
            let mut yielded = false;
            if map.in_bounds(pos.x, pos.y) {
                let tile = map.get(pos.x, pos.y);
                let yield_amount = tile.terrain.foraging_yield()
                    * (0.15 + skills.foraging * 0.6)
                    * season.foraging_multiplier();
                if yield_amount > 0.0 {
                    food.deposit(yield_amount); // Keep for backward compat

                    // Spawn foraged item on last tick only (not every tick)
                    if current.ticks_remaining == 0 {
                        let forage_kind = pick_forage_item(&tile.terrain, &mut rng);
                        let quality = 0.3 + skills.foraging * 0.3;
                        commands.spawn(Item::new(
                            forage_kind,
                            quality,
                            ItemLocation::Carried(entity),
                        ));
                    }

                    yielded = true;
                }
            }

            if yielded && current.ticks_remaining == 0 {
                memory.remember(MemoryEntry {
                    event_type: MemoryType::ResourceFound,
                    location: current.target_position,
                    involved: vec![],
                    tick: time.tick,
                    strength: 0.8,
                    firsthand: true,
                });
                mood.modifiers.push_back(MoodModifier {
                    amount: 0.05,
                    ticks_remaining: 15,
                    source: "good foraging".to_string(),
                });
            }

            skills.foraging += skills.growth_rate() * 0.01;
        }
    }
}
```

- [ ] **Step 2: Add forage item picker helper**

Add to `src/systems/actions.rs`:

```rust
fn pick_forage_item(terrain: &Terrain, rng: &mut SimRng) -> ItemKind {
    use crate::components::items::ItemKind;
    use crate::resources::map::Terrain;

    let roll: f32 = rng.rng.random();
    match terrain {
        Terrain::LightForest | Terrain::DenseForest => {
            if roll < 0.3 { ItemKind::Berries }
            else if roll < 0.5 { ItemKind::Nuts }
            else if roll < 0.7 { ItemKind::Mushroom }
            else if roll < 0.85 { ItemKind::Moss }
            else { ItemKind::DriedGrass }
        }
        Terrain::Grass => {
            if roll < 0.4 { ItemKind::Roots }
            else if roll < 0.6 { ItemKind::WildOnion }
            else if roll < 0.8 { ItemKind::DriedGrass }
            else { ItemKind::Berries }
        }
        _ => ItemKind::Roots, // Fallback
    }
}
```

- [ ] **Step 3: Run tests and commit**

Run: `just test`

```bash
jj new -m "feat: forage produces item entities alongside legacy food deposit"
```

---

## Task 8: Predator Food Web

**Files:**
- Modify: `src/systems/wildlife.rs`

- [ ] **Step 1: Add prey hunting to predator AI**

Extend the existing `wildlife_ai` system so that foxes and hawks target prey entities. When a predator kills prey, the prey entity is despawned (no item produced — predators eat their kills immediately).

Add a new system `predator_hunt_prey` in `src/systems/wildlife.rs`:

```rust
/// Predators (fox, hawk, snake) hunt nearby prey entities.
pub fn predator_hunt_prey(
    mut commands: Commands,
    predators: Query<(&WildAnimal, &Position), Without<PreyAnimal>>,
    prey: Query<(Entity, &PreyAnimal, &Position)>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
) {
    for (predator, pred_pos) in predators.iter() {
        // Only hunt when hungry (simplified: 10% chance per tick)
        if rng.rng.random::<f32>() > 0.1 {
            continue;
        }

        let hunt_range: i32 = match predator.species {
            WildSpecies::Fox => 3,
            WildSpecies::Hawk => 5,
            WildSpecies::Snake => 1, // Ambush range
            WildSpecies::ShadowFox => 3,
        };

        // Find nearest prey in range
        let mut nearest: Option<(Entity, i32)> = None;
        for (prey_entity, prey_animal, prey_pos) in prey.iter() {
            // Hawks prefer small prey; foxes eat anything
            let dist = pred_pos.manhattan_distance(prey_pos);
            if dist <= hunt_range {
                if nearest.is_none() || dist < nearest.unwrap().1 {
                    nearest = Some((prey_entity, dist));
                }
            }
        }

        if let Some((prey_entity, _)) = nearest {
            let prey_animal = prey.get(prey_entity).unwrap().1;
            if rng.rng.random::<f32>() < 0.3 {
                // Successful predator hunt
                let species_name = prey_animal.species.name();
                let predator_name = predator.species.name();
                commands.entity(prey_entity).despawn();

                if rng.rng.random::<f32>() < 0.05 {
                    log.push_micro(format!(
                        "A {predator_name} snatches a {species_name} from the undergrowth."
                    ));
                }
            }
        }
    }
}
```

- [ ] **Step 2: Wire into schedule**

In `src/main.rs`, add `predator_hunt_prey` after `wildlife_ai` in the world simulation chain.

- [ ] **Step 3: Run tests and commit**

Run: `just test`

```bash
jj new -m "feat: predators hunt prey entities — fox, hawk, snake thin prey populations"
```

---

## Task 9: Remove Legacy FoodStores

**Files:**
- Modify: `src/resources/food.rs`
- Modify: `src/systems/actions.rs`
- Modify: `src/ai/scoring.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create FoodSupply query helper**

Add to `src/resources/food.rs`:

```rust
use bevy_ecs::prelude::*;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::{Item, ItemLocation};

/// Query helper: count food items in all Stores buildings.
pub fn count_stored_food(
    stores: &Query<(&Structure, &StoredItems)>,
    items: &Query<&Item>,
) -> (usize, usize) {
    let mut food_count = 0usize;
    let mut capacity = 0usize;
    for (structure, stored) in stores.iter() {
        if structure.kind == StructureType::Stores {
            capacity += StoredItems::capacity(StructureType::Stores);
            for &item_entity in &stored.items {
                if let Ok(item) = items.get(item_entity) {
                    if item.kind.is_food() {
                        food_count += 1;
                    }
                }
            }
        }
    }
    (food_count, capacity)
}

/// Derived food fraction for scoring (replaces FoodStores.fraction()).
pub fn food_fraction(
    stores: &Query<(&Structure, &StoredItems)>,
    items: &Query<&Item>,
) -> f32 {
    let (count, cap) = count_stored_food(stores, items);
    if cap == 0 {
        return 0.0;
    }
    count as f32 / cap as f32
}
```

- [ ] **Step 2: Update ScoringContext to use item-based food**

In `src/ai/scoring.rs`, change `food_available` and `food_fraction` to be populated from the new `food_fraction()` helper instead of `FoodStores`:

```rust
// In context builder:
let (food_count, food_cap) = crate::resources::food::count_stored_food(&stores_query, &items_query);
let ctx = ScoringContext {
    food_available: food_count > 0,
    food_fraction: if food_cap > 0 { food_count as f32 / food_cap as f32 } else { 0.0 },
    // ... rest unchanged
};
```

Add the required query parameters to the `evaluate_actions` system signature.

- [ ] **Step 3: Add `find_nearest_store` helper**

Add to `src/systems/actions.rs`:

```rust
/// Find the nearest Stores building to a position.
fn find_nearest_store<'a>(
    cat_pos: &Position,
    stores_query: &'a Query<(Entity, &mut StoredItems, &Structure, &Position)>,
) -> Option<(Entity, Mut<'a, StoredItems>, &'a Structure, &'a Position)> {
    let mut nearest: Option<(Entity, i32)> = None;
    for (entity, _, structure, pos) in stores_query.iter() {
        if structure.kind == StructureType::Stores {
            let dist = cat_pos.manhattan_distance(pos);
            if nearest.is_none() || dist < nearest.unwrap().1 {
                nearest = Some((entity, dist));
            }
        }
    }
    nearest.and_then(|(entity, _)| {
        stores_query.get_mut(entity).ok().map(|(e, stored, structure, pos)| (e, stored, structure, pos))
    })
}
```

Note: The exact borrow-checker-friendly version may need adjustment — bevy_ecs's `Query::get_mut` returns a tuple of mutable refs. The key interface is: given a position, find the nearest Stores building and return a mutable reference to its `StoredItems`.

- [ ] **Step 4: Update Eat to consume item from stores**

In `src/systems/actions.rs`, replace the Eat block:

```rust
Action::Eat => {
    // Find a food item in the nearest Stores building
    if let Some((store_entity, mut stored, structure)) = find_nearest_store(&pos, &mut stores_query) {
        // Find first food item
        let food_item = stored.items.iter()
            .find(|&&e| items_query.get(e).map_or(false, |i| i.kind.is_food()))
            .copied();

        if let Some(item_entity) = food_item {
            let item = items_query.get(item_entity).unwrap();
            let food_val = item.kind.food_value();
            needs.hunger = (needs.hunger + food_val).min(1.0);
            stored.remove(item_entity);
            commands.entity(item_entity).despawn();
            current.ticks_remaining = 0; // Eat completes in one action
        } else {
            // No food in stores
            current.ticks_remaining = 0;
        }
    } else {
        current.ticks_remaining = 0;
    }
}
```

- [ ] **Step 5: Remove FoodStores resource**

Remove `FoodStores` from `src/resources/food.rs` (keep the file for the new helpers). Remove it from resource insertion in `src/main.rs`. Remove all remaining references — search for `FoodStores` and `food.deposit` / `food.withdraw` / `food.spoil`.

Remove the backward-compat `food.deposit()` calls in Hunt and Forage from Tasks 6 and 7.

- [ ] **Step 6: Update Hunt/Forage to deposit items into Stores building**

When a cat finishes hunting and is carrying an item, the AI should score a "deposit to stores" behavior. For now, simplify: when Hunt/Forage completes, if a Stores building exists, spawn the item with `ItemLocation::StoredIn(store_entity)` and add it to `StoredItems` directly:

```rust
// After spawning the item:
if let Some((store_entity, mut stored, _)) = find_nearest_store(&pos, &mut stores_query) {
    let item_entity = commands.spawn(Item::new(
        item_kind,
        quality,
        ItemLocation::StoredIn(store_entity),
    )).id();
    stored.add(item_entity, StructureType::Stores);
} else {
    // No stores — drop on ground
    commands.spawn((
        Item::new(item_kind, quality, ItemLocation::OnGround),
        pos.clone(),
    ));
}
```

- [ ] **Step 7: Run tests and commit**

Run: `just test`
Many tests will need updating where they reference `FoodStores`. For tests that need food, spawn a Stores building with `StoredItems` and pre-populate with food items.

```bash
jj new -m "refactor: replace FoodStores float with item-based food storage"
```

---

## Task 10: Inventory Refactor

**Files:**
- Modify: `src/components/magic.rs`
- Modify: `src/systems/actions.rs` (herbcraft references)
- Modify: `src/systems/magic.rs`

- [ ] **Step 1: Widen Inventory to hold ItemKind alongside HerbKind**

Replace in `src/components/magic.rs`:

```rust
/// A slot in a cat's inventory — either an old-style herb or a new item entity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ItemSlot {
    Herb(HerbKind),
    Item(ItemKind),
}

#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub slots: Vec<ItemSlot>,
}

impl Inventory {
    pub const MAX_SLOTS: usize = 5;

    pub fn is_full(&self) -> bool {
        self.slots.len() >= Self::MAX_SLOTS
    }

    // --- Herb compatibility methods ---

    pub fn has_herb(&self, kind: HerbKind) -> bool {
        self.slots.iter().any(|s| matches!(s, ItemSlot::Herb(k) if *k == kind))
    }

    pub fn take_herb(&mut self, kind: HerbKind) -> bool {
        if let Some(idx) = self.slots.iter().position(|s| matches!(s, ItemSlot::Herb(k) if *k == kind)) {
            self.slots.swap_remove(idx);
            true
        } else {
            false
        }
    }

    pub fn add_herb(&mut self, kind: HerbKind) -> bool {
        if self.is_full() {
            return false;
        }
        self.slots.push(ItemSlot::Herb(kind));
        true
    }

    pub fn has_remedy_herb(&self) -> bool {
        self.slots.iter().any(|s| matches!(s,
            ItemSlot::Herb(HerbKind::HealingMoss | HerbKind::Moonpetal | HerbKind::Calmroot)
        ))
    }

    pub fn has_ward_herb(&self) -> bool {
        self.has_herb(HerbKind::Thornbriar)
    }

    /// Count of herb slots specifically (for backward compat).
    pub fn herb_count(&self) -> usize {
        self.slots.iter().filter(|s| matches!(s, ItemSlot::Herb(_))).count()
    }

    // --- New item methods ---

    pub fn has_item(&self, kind: ItemKind) -> bool {
        self.slots.iter().any(|s| matches!(s, ItemSlot::Item(k) if *k == kind))
    }

    pub fn add_item(&mut self, kind: ItemKind) -> bool {
        if self.is_full() {
            return false;
        }
        self.slots.push(ItemSlot::Item(kind));
        true
    }

    pub fn take_item(&mut self, kind: ItemKind) -> bool {
        if let Some(idx) = self.slots.iter().position(|s| matches!(s, ItemSlot::Item(k) if *k == kind)) {
            self.slots.swap_remove(idx);
            true
        } else {
            false
        }
    }

    pub fn food_items(&self) -> impl Iterator<Item = &ItemKind> {
        self.slots.iter().filter_map(|s| match s {
            ItemSlot::Item(k) if k.is_food() => Some(k),
            _ => None,
        })
    }
}
```

- [ ] **Step 2: Fix all compilation errors**

The old `Inventory` had `herbs: Vec<HerbKind>`. Now it's `slots: Vec<ItemSlot>`. Search for all `inventory.herbs` references and replace:
- `inventory.herbs.len()` → `inventory.herb_count()` or `inventory.slots.len()`
- `inventory.herbs.contains()` → `inventory.has_herb()`
- `inventory.herbs.push()` → `inventory.add_herb()`

The compatibility methods on Inventory handle most of this, but direct field access needs fixing.

- [ ] **Step 3: Update serialization**

If save files exist with old `herbs: Vec<HerbKind>` format, add a `#[serde(alias = "herbs")]` or migration. For now, since this is pre-release, the old save format can break.

- [ ] **Step 4: Run tests and commit**

Run: `just test`

```bash
jj new -m "refactor: widen Inventory from herb-only to generic ItemSlot"
```

---

## Task 11: Integration Tests

**Files:**
- Create: `tests/ecosystem.rs`

- [ ] **Step 1: Write ecosystem balance tests**

```rust
use bevy_ecs::prelude::*;
use clowder::components::prey::{PreyAnimal, PreySpecies};
use clowder::components::physical::{Health, Position};
use clowder::resources::map::TileMap;
use clowder::resources::rng::SimRng;
use clowder::resources::narrative::NarrativeLog;

fn setup_ecosystem() -> (World, Schedule) {
    let mut world = World::new();
    let map = TileMap::new(40, 40, clowder::resources::map::Terrain::Grass);
    world.insert_resource(map);
    world.insert_resource(SimRng::new(42));
    world.insert_resource(NarrativeLog::default());

    let mut schedule = Schedule::default();
    schedule.add_systems((
        clowder::systems::prey::prey_population,
        clowder::systems::prey::prey_hunger,
    ).chain());
    (world, schedule)
}

#[test]
fn rats_grow_when_unchecked() {
    let (mut world, mut schedule) = setup_ecosystem();

    // Start with 5 rats
    for i in 0..5 {
        world.spawn((
            PreyAnimal::new(PreySpecies::Rat),
            Health::default(),
            Position::new(i * 2, 0),
        ));
    }

    for _ in 0..500 {
        schedule.run(&mut world);
    }

    let rat_count = world
        .query::<&PreyAnimal>()
        .iter(&world)
        .filter(|p| p.species == PreySpecies::Rat)
        .count();

    assert!(
        rat_count > 5,
        "rat population should grow when unchecked; got {rat_count}"
    );
}

#[test]
fn population_respects_carrying_capacity() {
    let (mut world, mut schedule) = setup_ecosystem();

    // Start near cap
    for i in 0..45 {
        world.spawn((
            PreyAnimal::new(PreySpecies::Rat),
            Health::default(),
            Position::new(i % 40, i / 40),
        ));
    }

    for _ in 0..500 {
        schedule.run(&mut world);
    }

    let rat_count = world
        .query::<&PreyAnimal>()
        .iter(&world)
        .filter(|p| p.species == PreySpecies::Rat)
        .count();

    assert!(
        rat_count <= 50,
        "rat population should not exceed cap of 50; got {rat_count}"
    );
}

#[test]
fn density_pressure_logged_near_cap() {
    let (mut world, mut schedule) = setup_ecosystem();

    // Spawn rats at 90% of cap
    for i in 0..45 {
        world.spawn((
            PreyAnimal::new(PreySpecies::Rat),
            Health::default(),
            Position::new(i % 40, i / 40),
        ));
    }

    for _ in 0..500 {
        schedule.run(&mut world);
    }

    let log = world.resource::<NarrativeLog>();
    let has_pressure_log = log.entries.iter().any(|e| {
        e.text.contains("restless") || e.text.contains("overrun")
    });
    assert!(has_pressure_log, "should log density pressure near cap");
}
```

- [ ] **Step 2: Run all tests**

Run: `just test`
Expected: All pass.

- [ ] **Step 3: Run full CI**

Run: `just ci`
Expected: Check + clippy + test all pass.

- [ ] **Step 4: Commit**

```bash
jj new -m "test: add ecosystem integration tests — population dynamics and carrying capacity"
```

---

## Verification Checklist

After all tasks are complete:

- [ ] `just test` — all tests pass (existing + new)
- [ ] `just check` — no clippy warnings
- [ ] `just run` — simulation runs; observe:
  - Prey animals visible on map (mouse 'm', rat 'r', fish '~', bird 'b')
  - Prey populations grow over time when unchecked
  - Cats hunt prey → prey count decreases
  - Predators (fox, hawk) also hunt prey
  - Narrative log shows density pressure messages when prey populations are high
  - Food items appear in Stores building
  - Item decay is visible (raw prey spoils if not eaten)
- [ ] Run with `just seed 42` twice — verify deterministic output
