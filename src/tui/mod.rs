pub mod log;
pub mod map;
pub mod status;

use ratatui::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::time::{SimConfig, TimeState};
use crate::resources::weather::WeatherState;

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
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(frame.area());

        let main_area = vertical[0];
        let bottom_area = vertical[1];

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(main_area);

        let left_area = horizontal[0];
        let right_area = horizontal[1];

        map::render_map(frame, left_area, self.map, &self.cat_positions);
        log::render_log(
            frame,
            right_area,
            self.narrative,
            self.time,
            self.config,
            self.weather,
            self.cat_count,
        );
        status::render_status(frame, bottom_area, self.time, self.config);
    }
}
