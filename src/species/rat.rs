use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyKind};
use crate::resources::map::Terrain;
use crate::resources::time::Season;
use super::PreyProfile;

pub struct Rat;

impl PreyProfile for Rat {
    fn kind(&self) -> PreyKind { PreyKind::Rat }
    fn name(&self) -> &'static str { "rat" }
    fn symbol(&self) -> char { 'r' }

    fn breed_rate(&self) -> f32 { 0.0005 }
    fn population_cap(&self) -> usize { 55 }
    fn habitat(&self) -> &'static [Terrain] {
        &[Terrain::Grass, Terrain::LightForest, Terrain::DenseForest]
    }
    fn seasonal_breed_modifier(&self, season: Season) -> f32 {
        match season {
            Season::Spring => 1.5,
            Season::Summer => 1.0,
            Season::Autumn => 0.5,
            Season::Winter => 0.2,
        }
    }

    fn item_kind(&self) -> ItemKind { ItemKind::RawRat }

    fn flee_speed(&self) -> u32 { 1 }
    fn graze_cadence(&self) -> u64 { 25 }
    fn alert_radius(&self) -> i32 { 4 }
    fn freeze_ticks(&self) -> u64 { 2 }
    fn catch_difficulty(&self) -> f32 { 1.0 }
    fn flee_strategy(&self) -> FleeStrategy { FleeStrategy::SeekCover }
    fn flee_duration(&self) -> u64 { 75 }

    fn den_name(&self) -> &'static str { "rat nest" }
    fn den_capacity(&self) -> u32 { 60 }
    fn den_spawn_rate(&self) -> f32 { 0.012 }
    fn den_habitat(&self) -> &'static [Terrain] { &[Terrain::DenseForest] }
    fn den_raid_drop(&self) -> u32 { 5 }
    fn den_spacing(&self) -> i32 { 10 }
    fn den_density(&self) -> usize { 100 }
}
