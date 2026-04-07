use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;

/// Marker for the main game camera.
#[derive(Component)]
pub struct GameCamera;

/// Startup system: spawns a 2D camera centered on the colony.
pub fn setup_camera(mut commands: Commands, map: Res<TileMap>) {
    let world_px = TILE_PX * TILE_SCALE;
    let center_x = (map.width as f32 / 2.0) * world_px;
    let center_y = (map.height as f32 / 2.0) * world_px;

    commands.spawn((
        Camera2d,
        Transform::from_xyz(center_x, center_y, 999.0),
        GameCamera,
    ));
}

/// Update system: scroll wheel zooms, arrow keys / WASD pans.
pub fn camera_controls(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut query: Query<(&mut Transform, &mut Projection), With<GameCamera>>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut projection)) = query.single_mut() else {
        return;
    };

    // Get current scale from the orthographic projection.
    let current_scale = match &*projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };

    // Zoom with scroll wheel.
    let mut new_scale = current_scale;
    for event in scroll_events.read() {
        let zoom_delta = -event.y * 0.1;
        new_scale = (new_scale + zoom_delta).clamp(0.2, 5.0);
    }
    if new_scale != current_scale {
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scale = new_scale;
        }
    }

    // Pan with arrow keys or WASD.
    let pan_speed = 500.0 * new_scale * time.delta_secs();
    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }

    if direction != Vec2::ZERO {
        direction = direction.normalize();
        transform.translation.x += direction.x * pan_speed;
        transform.translation.y += direction.y * pan_speed;
    }

    // Screenshot with F12.
    if keyboard.just_pressed(KeyCode::F12) {
        commands.spawn(Screenshot::primary_window())
            .observe(save_to_disk("/tmp/clowder_screenshot.png"));
    }
}
