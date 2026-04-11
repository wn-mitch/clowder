use crate::components::items::ItemKind;
use crate::components::prey::{FleeStrategy, PreyKind};
use crate::resources::map::Terrain;
use crate::resources::time::Season;
use super::PreyProfile;

pub struct Mouse;

impl PreyProfile for Mouse {
    fn kind(&self) -> PreyKind { PreyKind::Mouse }
    fn name(&self) -> &'static str { "mouse" }
    fn symbol(&self) -> char { 'm' }

    fn breed_rate(&self) -> f32 { 0.0003 }
    fn population_cap(&self) -> usize { 80 }
    fn habitat(&self) -> &'static [Terrain] { &[Terrain::Grass, Terrain::LightForest] }
    fn seasonal_breed_modifier(&self, season: Season) -> f32 {
        match season {
            Season::Spring => 1.5,
            Season::Summer => 1.0,
            Season::Autumn => 0.5,
            Season::Winter => 0.1,
        }
    }

    fn item_kind(&self) -> ItemKind { ItemKind::RawMouse }

    fn flee_speed(&self) -> u32 { 1 }
    fn graze_cadence(&self) -> u64 { 40 }
    fn alert_radius(&self) -> i32 { 3 }
    fn freeze_ticks(&self) -> u64 { 1 }
    fn catch_difficulty(&self) -> f32 { 0.9 }
    fn flee_strategy(&self) -> FleeStrategy { FleeStrategy::SeekCover }
    fn flee_duration(&self) -> u64 { 50 }

    fn den_name(&self) -> &'static str { "mouse nest" }
    fn den_capacity(&self) -> u32 { 80 }
    fn den_spawn_rate(&self) -> f32 { 0.01 }
    fn den_habitat(&self) -> &'static [Terrain] { &[Terrain::LightForest] }
    fn den_raid_drop(&self) -> u32 { 6 }
    fn den_spacing(&self) -> i32 { 10 }
    fn den_density(&self) -> usize { 100 }
}
