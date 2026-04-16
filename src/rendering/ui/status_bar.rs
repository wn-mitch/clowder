use bevy::prelude::*;

use crate::rendering::ui::{UiRoot, TEXT_COLOR};
use crate::resources::time::DayPhase;
use crate::resources::weather::WeatherState;
use crate::resources::{SimConfig, TimeState};

/// Marker for the status bar.
#[derive(Component)]
pub struct StatusBar;

/// Marker for the status text.
#[derive(Component)]
pub struct StatusText;

pub fn setup_status_bar(mut commands: Commands, root_query: Query<Entity, With<UiRoot>>) {
    let Ok(root) = root_query.single() else {
        return;
    };

    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                border: UiRect::top(Val::Px(1.0)),
                ..Default::default()
            },
            BackgroundColor(crate::rendering::ui::PANEL_BG),
            BorderColor::from(crate::rendering::ui::PANEL_BORDER),
            StatusBar,
        ))
        .id();

    let text = commands
        .spawn((
            Text::new(""),
            TextFont {
                font_size: 12.0,
                ..Default::default()
            },
            TextColor(TEXT_COLOR),
            StatusText,
        ))
        .id();

    commands.entity(bar).add_children(&[text]);
    commands.entity(root).add_children(&[bar]);
}

pub fn update_status_bar(
    time_state: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    mut text_query: Query<&mut Text, With<StatusText>>,
) {
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let day = TimeState::day_number(time_state.tick, &config);
    let season = time_state.season(&config);
    let phase = DayPhase::from_tick(time_state.tick, &config);
    let speed_label = time_state.speed.label();

    let pause_str = if time_state.paused { " PAUSED" } else { "" };

    **text = format!(
        "Day {} | {} {} | {} {} | Speed: {}{} | [P]ause []] Speed [L]og [R]oster [I]nventory [Tab] Inspect",
        day,
        season.label(),
        phase.label(),
        weather.current.symbol(),
        weather.current.label(),
        speed_label,
        pause_str,
    );
}
