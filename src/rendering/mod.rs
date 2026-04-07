pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod camera;
pub mod debug_grid;
pub mod entity_sprites;
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
                    (
                        tilemap_sync::create_tilemap,
                        entity_sprites::create_white_pixel,
                    ),
                    debug_grid::setup_grid,
                )
                    .chain()
                    .after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(
                Update,
                (
                    entity_sprites::attach_entity_sprites,
                    entity_sprites::sync_entity_positions,
                    debug_grid::toggle_grid,
                    debug_grid::toggle_overlay_layers,
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
