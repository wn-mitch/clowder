pub mod camera;
pub mod day_night;
pub mod debug_grid;
pub mod entity_sprites;
pub mod sprite_animation;
pub mod sprite_assets;
pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod ui;
pub mod weather_vfx;

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
                        sprite_assets::load_tree_sprite_pool,
                    ),
                    tilemap_sync::create_tilemap,
                    debug_grid::setup_grid,
                    day_night::setup_day_night_overlay,
                    weather_vfx::setup_weather_overlay_state,
                )
                    .chain()
                    .after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(
                Update,
                (
                    entity_sprites::sync_item_positions,
                    entity_sprites::compute_item_layout,
                    entity_sprites::attach_entity_sprites,
                    entity_sprites::attach_building_sprites,
                    entity_sprites::update_gate_sprites,
                    entity_sprites::update_crop_sprites,
                    entity_sprites::swap_seasonal_building_sprites,
                    entity_sprites::sync_entity_positions,
                    sprite_animation::tick_sprite_animations,
                    debug_grid::toggle_grid,
                    debug_grid::toggle_overlay_layers,
                    day_night::update_day_night_overlay,
                    weather_vfx::update_weather_overlays,
                    weather_vfx::apply_weather_alpha,
                    weather_vfx::sync_weather_overlay_positions,
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
                camera::setup_camera.after(crate::plugins::setup::setup_world_exclusive),
            )
            .add_systems(
                Update,
                (camera::camera_update, camera::auto_screenshot)
                    .after(entity_sprites::sync_entity_positions),
            );
    }
}
