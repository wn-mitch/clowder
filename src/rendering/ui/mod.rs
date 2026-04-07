mod log_panel;
mod status_bar;

use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (setup_ui_root, log_panel::setup_log_panel, status_bar::setup_status_bar)
                .chain()
                .after(crate::plugins::setup::setup_world_exclusive),
        )
        .add_systems(
            Update,
            (log_panel::update_log_panel, status_bar::update_status_bar),
        );
    }
}

/// Marker for the root UI container.
#[derive(Component)]
pub struct UiRoot;

/// Semi-transparent dark background color for panels.
pub const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.1, 0.85);
pub const PANEL_BORDER: Color = Color::srgba(0.4, 0.35, 0.25, 0.9);
pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.88, 0.82);
pub const TEXT_DIM: Color = Color::srgb(0.5, 0.5, 0.48);
pub const TEXT_HIGHLIGHT: Color = Color::srgb(0.95, 0.85, 0.4);

fn setup_ui_root(mut commands: Commands) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            ..Default::default()
        },
        // Don't block mouse events on the map
        Pickable::IGNORE,
        UiRoot,
    ));
}
