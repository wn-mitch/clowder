use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::resources::food::FoodStores;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::time::{DayPhase, SimConfig, TimeState};
use crate::resources::weather::WeatherState;

// ---------------------------------------------------------------------------
// Colony vitals snapshot (computed at render time, not an ECS resource)
// ---------------------------------------------------------------------------

/// Aggregate colony metrics for the header display.
#[derive(Debug, Default)]
pub struct ColonyVitals {
    /// Average mood valence mapped from [-1,1] to [0,1].
    pub avg_mood: f32,
    /// Average health across living cats (0.0-1.0).
    pub avg_health: f32,
    /// Average safety need across living cats (0.0-1.0).
    pub avg_safety: f32,
    /// Average building condition (0.0-1.0), `None` if no buildings.
    pub avg_bldg_condition: Option<f32>,
    /// Average building cleanliness (0.0-1.0), `None` if no buildings.
    pub avg_bldg_cleanliness: Option<f32>,
    /// Average cat corruption (0.0-1.0), `None` if all zero.
    pub avg_corruption: Option<f32>,
}

/// Build a visual bar like `[####------] 40%`.
fn bar(frac: f32, width: usize) -> String {
    let clamped = frac.clamp(0.0, 1.0);
    let filled = (clamped * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}] {:>3.0}%",
        "#".repeat(filled),
        "-".repeat(empty),
        clamped * 100.0
    )
}

/// Build a day-progress bar using block characters.
fn day_bar(frac: f32, width: usize) -> String {
    let clamped = frac.clamp(0.0, 1.0);
    let filled = (clamped * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Format a tick as a day/phase timestamp for log entries.
fn timestamp(tick: u64, config: &SimConfig) -> String {
    let day = TimeState::day_number(tick, config);
    let phase = DayPhase::from_tick(tick, config);
    format!("D{} {}", day, phase.label())
}

/// Simple word-wrap: splits `text` into lines of at most `max_width` chars,
/// breaking at word boundaries when possible.
fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            // First word on this line — accept even if it exceeds width.
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push(std::mem::take(&mut current_line));
            current_line.push_str(word);
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

#[allow(clippy::too_many_arguments)]
pub fn render_log(
    frame: &mut Frame,
    area: Rect,
    log: &NarrativeLog,
    time: &TimeState,
    config: &SimConfig,
    weather: &WeatherState,
    food: &FoodStores,
    cat_count: usize,
    vitals: &ColonyVitals,
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

    // Count dynamic vitals rows (food is always shown; others conditional).
    // Row 1: Food + Mood (always shown)
    // Row 2: Health + Safety (always shown)
    // Row 3: Bldgs + Corrupt (only if relevant)
    let has_bldg_row = vitals.avg_bldg_condition.is_some();
    let has_corrupt_row = vitals.avg_corruption.is_some();
    let extra_vitals_rows: u16 = if has_bldg_row || has_corrupt_row { 1 } else { 0 };

    // 2 clock lines + 2 vitals rows + optional row + separator
    let reserved: u16 = 2 + 2 + extra_vitals_rows + 1;
    let entry_lines = if inner.height > reserved {
        (inner.height - reserved) as usize
    } else {
        0
    };

    // Collect entries from newest, accumulating wrapped line counts until
    // we fill the available display lines. This handles entries that wrap
    // across multiple lines.
    let ts_sample = timestamp(time.tick, config);
    let ts_prefix_width = ts_sample.len() + 2; // "D1 Dawn: "
    let text_width = (inner.width as usize).saturating_sub(ts_prefix_width);
    let indent_width = ts_prefix_width; // continuation lines align with text start

    let mut selected_entries = Vec::new();
    let mut lines_used: usize = 0;
    for entry in log.entries.iter().rev() {
        let wrapped = word_wrap(&entry.text, text_width);
        let line_count = wrapped.len();
        if lines_used + line_count > entry_lines && !selected_entries.is_empty() {
            break;
        }
        lines_used += line_count;
        selected_entries.push(entry);
        if lines_used >= entry_lines {
            break;
        }
    }
    selected_entries.reverse();

    let mut lines: Vec<Line> = Vec::with_capacity(inner.height as usize);

    // Header line 1: Day — Season, Week
    let week = TimeState::week_number(time.tick, config);
    lines.push(Line::from(vec![Span::styled(
        format!(" Day {} \u{2014} {}, Week {}", day, season, week),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]));

    // Header line 2: phase icon + day progress bar + weather + cat count
    let day_phase = time.day_phase(config);
    let (phase_icon, phase_color) = match day_phase {
        DayPhase::Dawn => ("*", Color::Yellow),
        DayPhase::Day => ("o", Color::LightYellow),
        DayPhase::Dusk => ("*", Color::Red),
        DayPhase::Night => (".", Color::DarkGray),
    };
    let progress = TimeState::day_progress(time.tick, config);
    let progress_bar = day_bar(progress, 8);
    lines.push(Line::from(vec![
        Span::styled(format!(" {} ", phase_icon), Style::default().fg(phase_color)),
        Span::styled(format!("{} ", phase), Style::default().fg(phase_color)),
        Span::styled(format!("{progress_bar} "), Style::default().fg(Color::White)),
        Span::styled(
            format!("\u{2014} {} \u{2014} {} cats", weather_label, cat_count),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Vitals row 1: Food + Mood
    let food_str = bar(food.fraction(), 10);
    let mood_str = bar(vitals.avg_mood, 10);
    let food_color = if food.fraction() < 0.2 { Color::Red } else { Color::Green };
    let mood_color = if vitals.avg_mood < 0.3 { Color::Red } else { Color::Yellow };
    lines.push(Line::from(vec![
        Span::styled(format!(" Food: {food_str}"), Style::default().fg(food_color)),
        Span::styled(format!("  Mood: {mood_str}"), Style::default().fg(mood_color)),
    ]));

    // Vitals row 2: Health + Safety
    let health_str = bar(vitals.avg_health, 10);
    let safety_str = bar(vitals.avg_safety, 10);
    let health_color = if vitals.avg_health < 0.3 { Color::Red } else { Color::Green };
    let safety_color = if vitals.avg_safety < 0.3 { Color::Red } else { Color::Cyan };
    lines.push(Line::from(vec![
        Span::styled(format!(" Hlth: {health_str}"), Style::default().fg(health_color)),
        Span::styled(format!("  Safe: {safety_str}"), Style::default().fg(safety_color)),
    ]));

    // Vitals row 3 (optional): Buildings + Corruption
    if has_bldg_row || has_corrupt_row {
        let mut spans = Vec::new();
        if let Some(cond) = vitals.avg_bldg_condition {
            let bldg_str = bar(cond, 10);
            let bldg_color = if cond < 0.3 { Color::Red } else { Color::White };
            spans.push(Span::styled(format!(" Bldg: {bldg_str}"), Style::default().fg(bldg_color)));
        }
        if let Some(corr) = vitals.avg_corruption {
            let corr_str = bar(corr, 10);
            let corr_color = if corr > 0.5 { Color::Red } else { Color::Magenta };
            let prefix = if spans.is_empty() { " " } else { "  " };
            spans.push(Span::styled(format!("{prefix}Crpt: {corr_str}"), Style::default().fg(corr_color)));
        }
        lines.push(Line::from(spans));
    }

    // Separator
    lines.push(Line::from(Span::raw(separator)));

    // Log entries with timestamps and word wrapping
    for entry in &selected_entries {
        let ts = timestamp(entry.tick, config);
        let style = match entry.tier {
            NarrativeTier::Micro => Style::default().fg(Color::DarkGray),
            NarrativeTier::Action => Style::default().fg(Color::White),
            NarrativeTier::Significant => Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        };
        let ts_style = Style::default().fg(Color::DarkGray);
        let wrapped = word_wrap(&entry.text, text_width);
        for (i, line_text) in wrapped.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(format!("{ts}: "), ts_style),
                    Span::styled(line_text.clone(), style),
                ]));
            } else {
                // Continuation: indent to align with text start
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent_width)),
                    Span::styled(line_text.clone(), style),
                ]));
            }
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render a compact mini-log showing only the most recent Significant-tier
/// entries. Shown at the bottom of the right panel during inspect mode so the
/// player doesn't lose visibility of major events.
pub fn render_mini_log(
    frame: &mut Frame,
    area: Rect,
    log: &NarrativeLog,
    _time: &TimeState,
    config: &SimConfig,
) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let max_entries = inner.height as usize;

    // Filter to Significant and Action tier, take most recent N.
    let entries: Vec<_> = log
        .entries
        .iter()
        .rev()
        .filter(|e| matches!(e.tier, NarrativeTier::Significant | NarrativeTier::Action))
        .take(max_entries)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut lines: Vec<Line> = Vec::with_capacity(max_entries);
    for entry in entries {
        let ts = timestamp(entry.tick, config);
        let style = match entry.tier {
            NarrativeTier::Significant => Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::White),
        };
        let ts_style = Style::default().fg(Color::DarkGray);
        lines.push(Line::from(vec![
            Span::styled(format!("{ts}: "), ts_style),
            Span::styled(entry.text.clone(), style),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
