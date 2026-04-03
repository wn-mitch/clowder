use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::time::{SimConfig, TimeState};
use crate::resources::weather::WeatherState;

pub fn render_log(
    frame: &mut Frame,
    area: Rect,
    log: &NarrativeLog,
    time: &TimeState,
    config: &SimConfig,
    weather: &WeatherState,
    cat_count: usize,
) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let day = TimeState::day_number(time.tick, config);
    let season = time.season(config).label();
    let phase = time.day_phase(config).label();
    let weather_label = weather.current.label();

    let separator = "─".repeat(inner.width as usize);

    // Reserve 2 lines for header + separator; remaining for log entries.
    let reserved: u16 = 2;
    let entry_lines = if inner.height > reserved {
        (inner.height - reserved) as usize
    } else {
        0
    };

    // Collect entries newest-first, then reverse so newest appears at bottom.
    let total = log.entries.len();
    let take = entry_lines.min(total);
    let entries: Vec<_> = log
        .entries
        .iter()
        .rev()
        .take(take)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut lines: Vec<Line> = Vec::with_capacity(inner.height as usize);

    // Header
    lines.push(Line::from(vec![Span::styled(
        format!(
            " Day {} — {} — {} — {} — {} cats",
            day, season, phase, weather_label, cat_count
        ),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]));

    // Separator
    lines.push(Line::from(Span::raw(separator)));

    // Log entries
    for entry in entries {
        let style = match entry.tier {
            NarrativeTier::Micro => Style::default().fg(Color::DarkGray),
            NarrativeTier::Action => Style::default().fg(Color::White),
            NarrativeTier::Significant => Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        };
        lines.push(Line::from(Span::styled(entry.text.clone(), style)));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
