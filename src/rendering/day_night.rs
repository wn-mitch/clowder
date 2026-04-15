use bevy::prelude::*;

use crate::rendering::sprite_assets::SpriteAssets;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;
use crate::resources::time::{SimConfig, TimeState};

/// Marker for the day/night tint overlay sprite.
#[derive(Component)]
pub struct DayNightOverlay;

/// Spawn a world-space sprite covering the map, used as the day/night tint.
///
/// A world-space sprite (rather than a UI node) ensures the tint only covers
/// the map area, leaving UI panels untinted.
pub fn setup_day_night_overlay(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    map: Res<TileMap>,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let center_x = map.width as f32 * world_px / 2.0;
    let center_y = map.height as f32 * world_px / 2.0;
    // Half-tile margin on each side prevents sub-pixel gaps at map edges.
    let size = Vec2::new(
        (map.width as f32 + 1.0) * world_px,
        (map.height as f32 + 1.0) * world_px,
    );

    commands.spawn((
        Sprite {
            image: sprite_assets.white_pixel.clone(),
            color: Color::NONE,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(center_x, center_y, 50.0),
        DayNightOverlay,
    ));
}

/// Each frame, smoothly update the overlay color based on day cycle progress.
///
/// Instead of snapping to phase colors, we compute a continuous position in
/// the day cycle and lerp between the four phase tints. This produces gentle
/// sunrise/sunset transitions.
pub fn update_day_night_overlay(
    time_state: Res<TimeState>,
    config: Res<SimConfig>,
    mut query: Query<&mut Sprite, With<DayNightOverlay>>,
) {
    let Ok(mut sprite) = query.single_mut() else { return };

    let ticks_per_day = config.ticks_per_day_phase * 4;
    let tick_in_day = (time_state.tick % ticks_per_day) as f32;
    let phase_len = config.ticks_per_day_phase as f32;

    // Continuous progress through the day [0.0, 4.0).
    // 0.0-1.0 = Dawn, 1.0-2.0 = Day, 2.0-3.0 = Dusk, 3.0-4.0 = Night.
    let progress = tick_in_day / phase_len;

    // Phase center colors (what the tint looks like at the midpoint of each phase).
    let dawn = LinearRgba::new(1.0, 0.85, 0.5, 0.08);
    let day = LinearRgba::new(0.0, 0.0, 0.0, 0.0); // clear
    let dusk = LinearRgba::new(0.9, 0.5, 0.2, 0.12);
    let night = LinearRgba::new(0.1, 0.1, 0.35, 0.25);

    // Lerp between adjacent phases based on fractional progress.
    let color = match progress {
        p if p < 1.0 => lerp_rgba(dawn, day, p),           // Dawn → Day
        p if p < 2.0 => lerp_rgba(day, dusk, p - 1.0),     // Day → Dusk
        p if p < 3.0 => lerp_rgba(dusk, night, p - 2.0),   // Dusk → Night
        p => lerp_rgba(night, dawn, p - 3.0),               // Night → Dawn
    };

    sprite.color = Color::LinearRgba(color);
}

fn lerp_rgba(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}
