pub mod inspect;
pub mod log;
pub mod map;
pub mod status;
pub mod tile_inspect;

use bevy_ecs::entity::Entity;
use ratatui::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::components::physical::Position;
use crate::components::wildlife::WildSpecies;
use crate::resources::food::FoodStores;
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::time::{SimConfig, TimeState};
use crate::resources::weather::WeatherState;

use self::inspect::CatInspectData;
use self::log::ColonyVitals;

// ---------------------------------------------------------------------------
// Focus mode
// ---------------------------------------------------------------------------

/// UI focus state — lives in the main loop, not ECS.
#[derive(Default)]
pub enum FocusMode {
    #[default]
    None,
    Selecting {
        cats: Vec<(Entity, String)>,
        index: usize,
    },
    Inspecting(Entity),
    TileInspect {
        cursor: Position,
    },
    ZoneDesignate {
        cursor: Position,
        kind: crate::components::zone::ZoneKind,
    },
}


// ---------------------------------------------------------------------------
// AppView
// ---------------------------------------------------------------------------

pub struct AppView<'a> {
    pub map: &'a TileMap,
    pub cat_positions: Vec<map::CatDisplay>,
    pub wildlife_positions: Vec<(WildSpecies, Position, map::WildlifeBehavior)>,
    pub ward_positions: Vec<map::WardDisplay>,
    pub herb_positions: Vec<map::HerbDisplay>,
    pub zone_positions: Vec<map::ZoneDisplay>,
    pub narrative: &'a NarrativeLog,
    pub time: &'a TimeState,
    pub config: &'a SimConfig,
    pub weather: &'a WeatherState,
    pub food: &'a FoodStores,
    pub cat_count: usize,
    pub focus: &'a FocusMode,
    pub inspect_data: Option<&'a CatInspectData>,
    pub building_at_cursor: Option<tile_inspect::BuildingInfo>,
    pub coordinator_names: Vec<String>,
    pub priority: Option<crate::resources::colony_priority::PriorityKind>,
    pub vitals: ColonyVitals,
    pub activity_counts: Vec<(crate::ai::Action, usize)>,
}

impl<'a> AppView<'a> {
    pub fn render(&self, frame: &mut Frame) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(frame.area());

        let main_area = vertical[0];
        let bottom_area = vertical[1];

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(main_area);

        let left_area = horizontal[0];
        let right_area = horizontal[1];

        // Render map with optional cursor highlight.
        let cursor = match self.focus {
            FocusMode::TileInspect { cursor } => Some(*cursor),
            FocusMode::ZoneDesignate { cursor, .. } => Some(*cursor),
            _ => None,
        };
        let tick = self.time.tick;
        map::render_map(frame, left_area, self.map, &self.cat_positions, &self.wildlife_positions, &self.ward_positions, &self.herb_positions, &self.zone_positions, cursor, tick);

        // Right panel: inspect view when focused, narrative log otherwise.
        match self.focus {
            FocusMode::Selecting { cats, index } => {
                inspect::render_cat_list(frame, right_area, cats, *index);
            }
            FocusMode::Inspecting(_) => {
                // Split right panel: inspect on top, mini-log at bottom.
                let inspect_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(5)])
                    .split(right_area);
                let inspect_area = inspect_split[0];
                let mini_log_area = inspect_split[1];

                if let Some(data) = self.inspect_data {
                    inspect::render_inspect(frame, inspect_area, data);
                }
                log::render_mini_log(frame, mini_log_area, self.narrative, self.time, self.config);
            }
            FocusMode::TileInspect { cursor } => {
                let cats_at: Vec<&str> = self
                    .cat_positions
                    .iter()
                    .filter(|c| c.pos.x == cursor.x && c.pos.y == cursor.y)
                    .map(|c| c.name.as_str())
                    .collect();
                tile_inspect::render_tile_inspect(
                    frame, right_area, self.map, *cursor, &cats_at,
                    self.building_at_cursor.as_ref(),
                );
            }
            FocusMode::ZoneDesignate { .. } | FocusMode::None => {
                log::render_log(
                    frame,
                    right_area,
                    self.narrative,
                    self.time,
                    self.config,
                    self.weather,
                    self.food,
                    self.cat_count,
                    &self.vitals,
                );
            }
        }

        status::render_status(frame, bottom_area, self.time, self.config, self.focus, &self.coordinator_names, self.priority, &self.activity_counts);
    }
}
