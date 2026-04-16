use super::PreyProfile;
use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyKind};
use crate::resources::map::Terrain;
use crate::resources::time::Season;

pub struct Bird;

impl PreyProfile for Bird {
    fn kind(&self) -> PreyKind {
        PreyKind::Bird
    }
    fn name(&self) -> &'static str {
        "bird"
    }
    fn symbol(&self) -> char {
        'b'
    }

    fn breed_rate(&self) -> f32 {
        0.0001
    }
    fn population_cap(&self) -> usize {
        30
    }
    fn habitat(&self) -> &'static [Terrain] {
        &[Terrain::Grass, Terrain::LightForest]
    }
    fn seasonal_breed_modifier(&self, season: Season) -> f32 {
        match season {
            Season::Spring => 1.5,
            Season::Summer => 1.0,
            _ => 0.0,
        }
    }

    fn item_kind(&self) -> ItemKind {
        ItemKind::RawBird
    }

    fn flee_speed(&self) -> u32 {
        3
    }
    fn graze_cadence(&self) -> u64 {
        35
    }
    fn alert_radius(&self) -> i32 {
        8
    }
    fn freeze_ticks(&self) -> u64 {
        1
    }
    fn catch_difficulty(&self) -> f32 {
        0.5
    }
    fn flee_strategy(&self) -> FleeStrategy {
        FleeStrategy::Teleport
    }
    fn flee_duration(&self) -> u64 {
        30
    }

    fn den_name(&self) -> &'static str {
        "bird nest"
    }
    fn den_capacity(&self) -> u32 {
        40
    }
    fn den_spawn_rate(&self) -> f32 {
        0.004
    }
    fn den_habitat(&self) -> &'static [Terrain] {
        &[Terrain::LightForest]
    }
    fn den_raid_drop(&self) -> u32 {
        3
    }
    fn den_spacing(&self) -> i32 {
        15
    }
    fn den_density(&self) -> usize {
        250
    }
}
