use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::ai::Action;
use crate::resources::time::{SimConfig, TimeState};
use crate::tui::FocusMode;

/// Short label for action display in the status bar.
fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Eat => "Eat",
        Action::Sleep => "Sleep",
        Action::Hunt => "Hunt",
        Action::Forage => "Forage",
        Action::Wander => "Wander",
        Action::Idle => "Idle",
        Action::Socialize => "Social",
        Action::Groom => "Groom",
        Action::Explore => "Explore",
        Action::Flee => "Flee",
        Action::Fight => "Fight",
        Action::Patrol => "Patrol",
        Action::Build => "Build",
        Action::Farm => "Farm",
        Action::Herbcraft => "Herb",
        Action::PracticeMagic => "Magic",
        Action::Coordinate => "Coord",
        Action::Mentor => "Mentor",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    time: &TimeState,
    _config: &SimConfig,
    focus: &FocusMode,
    coordinator_names: &[String],
    priority: Option<crate::resources::colony_priority::PriorityKind>,
    activity_counts: &[(Action, usize)],
) {
    let paused_indicator = if time.paused { " [PAUSED]" } else { "" };
    let focus_hint = match focus {
        FocusMode::None => " [F]ocus [I]nspect tile [Shift+P]riority",
        FocusMode::Selecting { .. } => " [↑↓]Select [Enter]Inspect [Esc]Back",
        FocusMode::Inspecting(_) => " [F/Esc]Back",
        FocusMode::TileInspect { .. } => " [↑↓←→]Move [I/Esc]Back",
        FocusMode::ZoneDesignate { kind, .. } => match kind {
            crate::components::zone::ZoneKind::BuildHere => " [↑↓←→]Move [Tab]Type:Build [Enter]Place [Esc]Cancel",
            crate::components::zone::ZoneKind::FarmHere => " [↑↓←→]Move [Tab]Type:Farm [Enter]Place [Esc]Cancel",
            crate::components::zone::ZoneKind::Avoid => " [↑↓←→]Move [Tab]Type:Avoid [Enter]Place [Esc]Cancel",
        },
    };
    let coord_str = if coordinator_names.is_empty() {
        String::new()
    } else {
        format!(" Coord: {}", coordinator_names.join(", "))
    };
    let priority_str = match priority {
        Some(p) => format!(" Priority: {}", p.label()),
        None => String::new(),
    };

    // Line 1: controls
    let line1 = format!(
        " [S]peed: {} [P]ause{}{coord_str}{priority_str} [Q]uit{focus_hint}",
        time.speed.label(),
        paused_indicator,
    );

    // Line 2: activity summary
    let activity_parts: Vec<String> = activity_counts
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(action, count)| format!("{}:{}", action_label(action), count))
        .collect();
    let line2 = if activity_parts.is_empty() {
        String::new()
    } else {
        format!(" {}", activity_parts.join(" "))
    };

    let text = Text::from(vec![
        Line::from(line1),
        Line::from(line2),
    ]);

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Black).bg(Color::White));
    frame.render_widget(paragraph, area);
}
