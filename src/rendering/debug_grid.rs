use bevy::prelude::*;

use crate::rendering::entity_sprites::WhitePixel;
use crate::rendering::tilemap_sync::{BlobOverlayLayer, TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;

/// Whether the debug grid is visible.
#[derive(Resource)]
pub struct DebugGrid {
    pub visible: bool,
}

/// Marker for grid line sprites.
#[derive(Component)]
pub struct GridLine;

/// Marker for coordinate label text.
#[derive(Component)]
pub struct GridLabel;

const GRID_Z: f32 = 10.0;
const LINE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.15);
const LABEL_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.6);

/// Startup: spawn grid lines and coordinate labels, initially hidden.
pub fn setup_grid(mut commands: Commands, map: Res<TileMap>, white_pixel: Res<WhitePixel>) {
    commands.insert_resource(DebugGrid { visible: false });

    let world_px = TILE_PX * TILE_SCALE;
    let map_w = map.width as f32;
    let map_h = map.height as f32;
    let total_w = map_w * world_px;
    let total_h = map_h * world_px;

    // Vertical lines (at each tile column boundary).
    for col in 0..=map.width {
        let x = col as f32 * world_px;
        commands.spawn((
            Sprite {
                image: white_pixel.0.clone(),
                color: LINE_COLOR,
                custom_size: Some(Vec2::new(1.0, total_h)),
                ..default()
            },
            Transform::from_xyz(x, total_h / 2.0, GRID_Z),
            Visibility::Hidden,
            GridLine,
        ));
    }

    // Horizontal lines (at each tile row boundary).
    for row in 0..=map.height {
        let y = row as f32 * world_px;
        commands.spawn((
            Sprite {
                image: white_pixel.0.clone(),
                color: LINE_COLOR,
                custom_size: Some(Vec2::new(total_w, 1.0)),
                ..default()
            },
            Transform::from_xyz(total_w / 2.0, y, GRID_Z),
            Visibility::Hidden,
            GridLine,
        ));
    }

    // Coordinate labels every 10 tiles along the left edge.
    // Grid y=0 is at the TOP of the map (y-down), which maps to the
    // highest screen y (y-up). Labels show grid coordinates.
    for gy in (0..map.height).step_by(10) {
        let screen_y = (map_h - gy as f32 - 0.5) * world_px;
        commands.spawn((
            Text2d::new(format!("y={gy}")),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(LABEL_COLOR),
            Transform::from_xyz(-world_px * 0.5, screen_y, GRID_Z),
            Visibility::Hidden,
            GridLabel,
            GridLine, // reuse GridLine for toggle
        ));
    }

    // Coordinate labels every 10 tiles along the top edge.
    for gx in (0..map.width).step_by(10) {
        let screen_x = (gx as f32 + 0.5) * world_px;
        let screen_y = total_h + world_px * 0.3;
        commands.spawn((
            Text2d::new(format!("x={gx}")),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(LABEL_COLOR),
            Transform::from_xyz(screen_x, screen_y, GRID_Z),
            Visibility::Hidden,
            GridLabel,
            GridLine, // reuse GridLine for toggle
        ));
    }
}

/// F4 toggles grid visibility.
pub fn toggle_grid(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut grid: ResMut<DebugGrid>,
    mut query: Query<&mut Visibility, With<GridLine>>,
) {
    if !keyboard.just_pressed(KeyCode::F4) {
        return;
    }

    grid.visible = !grid.visible;
    let vis = if grid.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for mut v in &mut query {
        *v = vis;
    }
}

/// F6/F7/F8 toggle individual overlay layers by z-value.
/// F6 = Soil (z=1), F7 = Stone (z=2), F8 = Grass (z=3).
pub fn toggle_overlay_layers(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tilemaps: Query<(&mut Visibility, &Transform), With<BlobOverlayLayer>>,
) {
    let toggle_z = if keyboard.just_pressed(KeyCode::F6) {
        eprintln!("Toggle: Soil overlay (z=1)");
        Some(1.0)
    } else if keyboard.just_pressed(KeyCode::F7) {
        eprintln!("Toggle: Stone overlay (z=2)");
        Some(2.0)
    } else if keyboard.just_pressed(KeyCode::F8) {
        eprintln!("Toggle: Grass overlay (z=3)");
        Some(3.0)
    } else {
        None
    };

    if let Some(z) = toggle_z {
        for (mut vis, transform) in &mut tilemaps {
            if (transform.translation.z - z).abs() < 0.5 {
                *vis = match *vis {
                    Visibility::Hidden => Visibility::Inherited,
                    _ => Visibility::Hidden,
                };
            }
        }
    }
}
