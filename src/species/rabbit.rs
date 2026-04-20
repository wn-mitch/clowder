use super::PreyProfile;
use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyKind};
use crate::resources::map::Terrain;
use crate::resources::time::Season;

pub struct Rabbit;

impl PreyProfile for Rabbit {
    fn kind(&self) -> PreyKind {
        PreyKind::Rabbit
    }
    fn name(&self) -> &'static str {
        "rabbit"
    }
    fn plural_name(&self) -> &'static str {
        "rabbits"
    }
    fn symbol(&self) -> char {
        'R'
    }

    fn breed_rate(&self) -> f32 {
        0.0004
    }
    fn population_cap(&self) -> usize {
        45
    }
    fn habitat(&self) -> &'static [Terrain] {
        &[Terrain::Grass]
    }
    fn seasonal_breed_modifier(&self, season: Season) -> f32 {
        match season {
            Season::Spring => 2.0,
            Season::Summer => 1.0,
            _ => 0.0,
        }
    }

    fn item_kind(&self) -> ItemKind {
        ItemKind::RawRabbit
    }

    fn flee_speed(&self) -> u32 {
        1
    }
    fn graze_cadence(&self) -> u64 {
        20
    }
    fn alert_radius(&self) -> i32 {
        6
    }
    fn freeze_ticks(&self) -> u64 {
        10
    }
    fn catch_difficulty(&self) -> f32 {
        0.85
    }
    fn flee_strategy(&self) -> FleeStrategy {
        FleeStrategy::Standard
    }
    fn flee_duration(&self) -> u64 {
        60
    }

    fn den_name(&self) -> &'static str {
        "rabbit warren"
    }
    fn den_capacity(&self) -> u32 {
        60
    }
    fn den_spawn_rate(&self) -> f32 {
        0.01
    }
    fn den_habitat(&self) -> &'static [Terrain] {
        &[Terrain::Grass]
    }
    fn den_raid_drop(&self) -> u32 {
        4
    }
    fn den_spacing(&self) -> i32 {
        20
    }
    fn den_density(&self) -> usize {
        250
    }
}
