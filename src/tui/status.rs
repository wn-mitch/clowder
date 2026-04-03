use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::resources::time::{SimConfig, TimeState};

pub fn render_status(frame: &mut Frame, area: Rect, time: &TimeState, _config: &SimConfig) {
    let paused_indicator = if time.paused { " [PAUSED]" } else { "" };
    let text = format!(
        " [S]peed: {} [P]ause{} [Q]uit",
        time.speed.label(),
        paused_indicator,
    );

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Black).bg(Color::White));
    frame.render_widget(paragraph, area);
}
