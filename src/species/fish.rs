use super::PreyProfile;
use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyKind};
use crate::resources::map::Terrain;
use crate::resources::time::Season;

pub struct Fish;

impl PreyProfile for Fish {
    fn kind(&self) -> PreyKind {
        PreyKind::Fish
    }
    fn name(&self) -> &'static str {
        "fish"
    }
    fn symbol(&self) -> char {
        '~'
    }

    fn breed_rate(&self) -> f32 {
        0.0002
    }
    fn population_cap(&self) -> usize {
        35
    }
    fn habitat(&self) -> &'static [Terrain] {
        &[Terrain::Water]
    }
    fn seasonal_breed_modifier(&self, season: Season) -> f32 {
        match season {
            Season::Spring => 2.0,
            Season::Summer => 0.5,
            Season::Autumn => 0.3,
            Season::Winter => 0.1,
        }
    }

    fn item_kind(&self) -> ItemKind {
        ItemKind::RawFish
    }

    fn flee_speed(&self) -> u32 {
        0
    }
    fn graze_cadence(&self) -> u64 {
        50
    }
    fn alert_radius(&self) -> i32 {
        2
    }
    fn freeze_ticks(&self) -> u64 {
        0
    }
    fn catch_difficulty(&self) -> f32 {
        0.6
    }
    fn flee_strategy(&self) -> FleeStrategy {
        FleeStrategy::Stationary
    }
    fn flee_duration(&self) -> u64 {
        0
    }

    fn den_name(&self) -> &'static str {
        "spawning pool"
    }
    fn den_capacity(&self) -> u32 {
        50
    }
    fn den_spawn_rate(&self) -> f32 {
        0.006
    }
    fn den_habitat(&self) -> &'static [Terrain] {
        &[Terrain::Water]
    }
    fn den_raid_drop(&self) -> u32 {
        3
    }
    fn den_spacing(&self) -> i32 {
        20
    }
    fn den_density(&self) -> usize {
        250
    }
}
