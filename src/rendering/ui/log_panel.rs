use bevy::prelude::*;

use crate::rendering::ui::{TEXT_COLOR, TEXT_DANGER, TEXT_DIM, TEXT_HIGHLIGHT, TEXT_NATURE, UiRoot};
use crate::resources::{NarrativeLog, NarrativeTier, SimConfig, TimeState};

/// Marker for the log panel container.
#[derive(Component)]
pub struct LogPanel;

/// Marker for the log text content area.
#[derive(Component)]
pub struct LogContent;

/// Marker for the log header text.
#[derive(Component)]
pub struct LogHeader;

/// Tracks how many log entries we've rendered.
#[derive(Component)]
pub struct LogState {
    pub rendered_count: u64,
}

pub fn setup_log_panel(
    mut commands: Commands,
    root_query: Query<Entity, With<UiRoot>>,
) {
    let Ok(root) = root_query.single() else { return };

    // Log panel: right side, top area
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(8.0),
                width: Val::Px(380.0),
                height: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::clip_y(),
                ..Default::default()
            },
            BackgroundColor(crate::rendering::ui::PANEL_BG),
            BorderColor::from(crate::rendering::ui::PANEL_BORDER),
            LogPanel,
        ))
        .id();

    // Header
    let header = commands
        .spawn((
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..Default::default()
            },
            Text::new("Day 1 — Spring — Dawn"),
            TextFont {
                font_size: 14.0,
                ..Default::default()
            },
            TextColor(TEXT_HIGHLIGHT),
            LogHeader,
        ))
        .id();

    // Scrollable log content
    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                flex_grow: 1.0,
                ..Default::default()
            },
            ScrollPosition::default(),
            LogContent,
            LogState { rendered_count: 0 },
        ))
        .id();

    commands.entity(panel).add_children(&[header, content]);
    commands.entity(root).add_children(&[panel]);
}

pub fn update_log_panel(
    mut commands: Commands,
    narrative: Res<NarrativeLog>,
    time_state: Res<TimeState>,
    config: Res<SimConfig>,
    mut header_query: Query<&mut Text, With<LogHeader>>,
    mut content_query: Query<(Entity, &mut LogState, &mut ScrollPosition), With<LogContent>>,
) {
    // Update header with current time info.
    if let Ok(mut header_text) = header_query.single_mut() {
        let day = TimeState::day_number(time_state.tick, &config);
        let season = time_state.season(&config);
        let phase = crate::resources::time::DayPhase::from_tick(time_state.tick, &config);
        **header_text = format!(
            "Day {} — {} — {}",
            day,
            season.label(),
            phase.label(),
        );
    }

    // Add new log entries.
    let Ok((content_entity, mut log_state, mut scroll)) = content_query.single_mut() else {
        return;
    };

    let new_count = narrative.total_pushed.saturating_sub(log_state.rendered_count);
    if new_count == 0 {
        return;
    }

    let start = narrative.entries.len().saturating_sub(new_count as usize);
    for entry in narrative.entries.range(start..) {
        let color = match entry.tier {
            NarrativeTier::Micro => TEXT_DIM,
            NarrativeTier::Action => TEXT_COLOR,
            NarrativeTier::Significant => TEXT_HIGHLIGHT,
            NarrativeTier::Danger => TEXT_DANGER,
            NarrativeTier::Nature => TEXT_NATURE,
        };

        let day = TimeState::day_number(entry.tick, &config);
        let phase = crate::resources::time::DayPhase::from_tick(entry.tick, &config);
        let prefix = format!("D{} {}: ", day, phase.label());

        let entry_node = commands
            .spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(3.0)),
                    ..Default::default()
                },
                Text::new(format!("{}{}", prefix, entry.text)),
                TextFont {
                    font_size: 11.0,
                    ..Default::default()
                },
                TextColor(color),
            ))
            .id();

        commands.entity(content_entity).add_children(&[entry_node]);
    }

    log_state.rendered_count = narrative.total_pushed;

    // Auto-scroll to bottom.
    scroll.0.y = f32::MAX;
}
