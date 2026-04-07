pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod camera;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .add_systems(Startup, tilemap_sync::create_tilemap);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, camera::setup_camera)
            .add_systems(Update, camera::camera_controls);
    }
}
