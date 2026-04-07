mod cat_inspect;
mod log_panel;
pub mod panel;
mod selection;
mod status_bar;
mod tile_inspect;

use bevy::prelude::*;

use crate::ui_data::InspectionState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectionState>()
            .init_resource::<PanelVisibility>()
            .add_systems(
                Startup,
                (
                    panel::setup_ui_assets,
                    setup_ui_root,
                    log_panel::setup_log_panel,
                    status_bar::setup_status_bar,
                    cat_inspect::setup_cat_inspect_panel,
                    tile_inspect::setup_tile_inspect,
                )
                    .chain()
                    .after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(
                Update,
                (
                    selection::track_cursor_position,
                    selection::handle_world_click,
                    selection::handle_keyboard_selection,
                    toggle_panel_visibility,
                    log_panel::update_log_panel,
                    status_bar::update_status_bar,
                    cat_inspect::update_cat_inspect_panel,
                    tile_inspect::update_tile_inspect,
                )
                    .chain(),
            );
    }
}

/// Marker for the root UI container.
#[derive(Component)]
pub struct UiRoot;

/// Controls which panels are visible.
#[derive(Resource)]
pub struct PanelVisibility {
    pub log: bool,
    pub status_bar: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            log: true,
            status_bar: true,
        }
    }
}

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

fn toggle_panel_visibility(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel_vis: ResMut<PanelVisibility>,
    mut log_query: Query<&mut Visibility, With<log_panel::LogPanel>>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        panel_vis.log = !panel_vis.log;
        for mut vis in &mut log_query {
            *vis = if panel_vis.log {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}
