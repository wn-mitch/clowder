pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod camera;
pub mod day_night;
pub mod debug_grid;
pub mod entity_sprites;
pub mod sprite_assets;
pub mod ui;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .add_systems(
                Startup,
                (
                    // load_sprite_assets must run before create_tilemap (trees, corruption)
                    (
                        entity_sprites::create_white_pixel,
                        sprite_assets::load_sprite_assets,
                    ),
                    tilemap_sync::create_tilemap,
                    debug_grid::setup_grid,
                )
                    .chain()
                    .after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(
                Startup,
                day_night::setup_day_night_overlay,
            )
            .add_systems(
                Update,
                (
                    entity_sprites::sync_item_positions,
                    entity_sprites::compute_item_layout,
                    entity_sprites::attach_entity_sprites,
                    entity_sprites::sync_entity_positions,
                    debug_grid::toggle_grid,
                    debug_grid::toggle_overlay_layers,
                    day_night::update_day_night_overlay,
                )
                    .chain(),
            );
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<camera::AutoScreenshot>()
            .add_systems(
                Startup,
                camera::setup_camera
                    .after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(Update, (camera::camera_update, camera::auto_screenshot));
    }
}
