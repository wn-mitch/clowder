use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::identity::Species;
use crate::components::physical::{Dead, Position};
use crate::rendering::camera::GameCamera;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;
use crate::ui_data::{InspectionMode, InspectionState};

const WORLD_PX: f32 = TILE_PX * TILE_SCALE;

/// Convert world coordinates back to grid coordinates.
///
/// Computes the TilePos row via floor first, then inverts to data-space y.
/// Applying floor *after* the subtraction (`floor(a - b)`) gives the wrong
/// result when `b` has a fractional part, because `floor(a - b) ≠ a - floor(b)`.
fn world_to_grid(world: Vec2, map_height: f32) -> (i32, i32) {
    let gx = (world.x / WORLD_PX).floor() as i32;
    let tile_y = (world.y / WORLD_PX).floor() as i32;
    let gy = map_height as i32 - 1 - tile_y;
    (gx, gy)
}

/// Track cursor world and grid position every frame.
pub fn track_cursor_position(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    map: Res<TileMap>,
    mut inspection: ResMut<InspectionState>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok((camera, global_transform)) = camera_q.single() else { return };

    let Some(cursor_pos) = window.cursor_position() else {
        inspection.cursor_world_pos = None;
        inspection.cursor_grid_pos = None;
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(global_transform, cursor_pos) else {
        return;
    };

    inspection.cursor_world_pos = Some(world_pos);
    let (gx, gy) = world_to_grid(world_pos, map.height as f32);
    inspection.cursor_grid_pos = Some((gx, gy));
}

/// Handle mouse clicks: left-click selects cat or tile, right-click dismisses.
pub fn handle_world_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut inspection: ResMut<InspectionState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cats: Query<(Entity, &Position), (With<Species>, Without<Dead>)>,
    map: Res<TileMap>,
) {
    if mouse.just_pressed(MouseButton::Right) {
        inspection.mode = InspectionMode::None;
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((gx, gy)) = inspection.cursor_grid_pos else { return };

    // Ignore clicks outside the map entirely.
    if !map.in_bounds(gx, gy) {
        return;
    }

    // Store screen position for tile popup placement.
    if let Ok(window) = windows.single() {
        inspection.click_screen_pos = window.cursor_position().map(|p| Vec2::new(p.x, p.y));
    }

    // Find nearest cat within 1.5-tile radius of click position.
    let mut nearest: Option<(Entity, f32)> = None;
    for (entity, pos) in &cats {
        let dx = (pos.x - gx) as f32;
        let dy = (pos.y - gy) as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 1.5 && nearest.is_none_or(|(_, d)| dist < d) {
            nearest = Some((entity, dist));
        }
    }

    if let Some((entity, _)) = nearest {
        inspection.mode = InspectionMode::CatInspect(entity);
        inspection.last_selected_cat = Some(entity);
    } else if map.in_bounds(gx, gy) {
        inspection.mode = InspectionMode::TileInspect { x: gx, y: gy };
    }
}

/// Handle keyboard selection: Tab cycles cats, T inspects tile under cursor.
pub fn handle_keyboard_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inspection: ResMut<InspectionState>,
    cats: Query<(Entity, &Name), (With<Species>, Without<Dead>)>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        let mut cat_list: Vec<(Entity, &Name)> = cats.iter().collect();
        if cat_list.is_empty() {
            return;
        }
        // Sort by entity for stable ordering.
        cat_list.sort_by_key(|(e, _)| *e);

        let next_idx = match inspection.mode {
            InspectionMode::CatInspect(current) => {
                let current_idx = cat_list.iter().position(|(e, _)| *e == current);
                match current_idx {
                    Some(i) => (i + 1) % cat_list.len(),
                    None => 0,
                }
            }
            _ => 0,
        };

        let (entity, _) = cat_list[next_idx];
        inspection.mode = InspectionMode::CatInspect(entity);
        inspection.last_selected_cat = Some(entity);
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        if let Some((x, y)) = inspection.cursor_grid_pos {
            inspection.mode = InspectionMode::TileInspect { x, y };
        }
    }
}
