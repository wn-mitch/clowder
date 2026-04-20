mod bird;
mod fish;
mod mouse;
mod rabbit;
mod rat;

pub use bird::Bird;
pub use fish::Fish;
pub use mouse::Mouse;
pub use rabbit::Rabbit;
pub use rat::Rat;

use bevy_ecs::prelude::*;

use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyConfig, PreyKind, PreyState};
use crate::resources::map::Terrain;
use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// PreyProfile trait
// ---------------------------------------------------------------------------

/// Defines the complete behavioral and ecological profile for a prey species.
///
/// Each species implements this trait once. At spawn time, `to_config()` flattens
/// the profile into a `PreyConfig` component that systems query directly — no
/// trait dispatch at runtime.
pub trait PreyProfile: Send + Sync + 'static {
    // --- Identity ---
    fn kind(&self) -> PreyKind;
    fn name(&self) -> &'static str;
    fn plural_name(&self) -> &'static str;
    fn symbol(&self) -> char;

    // --- Population ---
    fn breed_rate(&self) -> f32;
    fn population_cap(&self) -> usize;
    fn habitat(&self) -> &'static [Terrain];
    fn seasonal_breed_modifier(&self, season: Season) -> f32;

    // --- Drops ---
    fn item_kind(&self) -> ItemKind;

    // --- Behavior ---
    fn flee_speed(&self) -> u32;
    fn graze_cadence(&self) -> u64;
    fn alert_radius(&self) -> i32;
    fn freeze_ticks(&self) -> u64;
    fn catch_difficulty(&self) -> f32;
    fn flee_strategy(&self) -> FleeStrategy;
    fn flee_duration(&self) -> u64;

    // --- Den ---
    fn den_name(&self) -> &'static str;
    fn den_capacity(&self) -> u32;
    fn den_spawn_rate(&self) -> f32;
    fn den_habitat(&self) -> &'static [Terrain];
    fn den_raid_drop(&self) -> u32;
    /// Minimum manhattan distance between dens of this species during world gen.
    fn den_spacing(&self) -> i32;
    /// Target habitat tiles per den (lower = more dens).
    fn den_density(&self) -> usize;

    // --- Flatten to component ---

    fn to_config(&self) -> PreyConfig {
        PreyConfig {
            kind: self.kind(),
            name: self.name(),
            item_kind: self.item_kind(),
            flee_speed: self.flee_speed(),
            graze_cadence: self.graze_cadence(),
            alert_radius: self.alert_radius(),
            freeze_ticks: self.freeze_ticks(),
            catch_difficulty: self.catch_difficulty(),
            flee_strategy: self.flee_strategy(),
            flee_duration: self.flee_duration(),
            habitat: self.habitat(),
        }
    }

    fn to_state(&self) -> PreyState {
        PreyState::default()
    }
}

// ---------------------------------------------------------------------------
// SpeciesRegistry
// ---------------------------------------------------------------------------

/// Resource holding all prey species profiles for systems that need to iterate
/// (population management, world gen, den lifecycle).
#[derive(Resource)]
pub struct SpeciesRegistry {
    pub profiles: Vec<Box<dyn PreyProfile>>,
}

impl SpeciesRegistry {
    pub fn find(&self, kind: PreyKind) -> &dyn PreyProfile {
        self.profiles
            .iter()
            .find(|p| p.as_ref().kind() == kind)
            .map(|p| p.as_ref())
            .expect("all PreyKind variants must be in the registry")
    }
}

pub fn build_registry() -> SpeciesRegistry {
    SpeciesRegistry {
        profiles: vec![
            Box::new(Mouse),
            Box::new(Rat),
            Box::new(Rabbit),
            Box::new(Fish),
            Box::new(Bird),
        ],
    }
}
