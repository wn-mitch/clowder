//! Ticket 260 — `CatScentMap` signpost overlay.
//!
//! Renders a per-bucket gold tint over the map so the player can read
//! where cat-territory scent is dense without opening a debug overlay.
//! One `Sprite` entity per `CatScentMap` bucket (default 24 × 18 ≈ 432
//! sprites), pre-spawned at startup and intensity-modulated each tick
//! via `Sprite.color` alpha. F5 toggles the overlay on/off; the
//! per-tick system honours the toggle so hidden signposts incur zero
//! Visibility writes.
//!
//! Z value 3.5 sits between the grass autotile (z=3.0) and the
//! corruption haze (z=4.0).

use bevy::prelude::*;

use crate::rendering::sprite_assets::SpriteAssets;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::CatScentMap;
use crate::resources::map::TileMap;

/// Marker on each signpost sprite. Carries the bucket coordinates so
/// `update_scent_signposts` can index back into `CatScentMap.marks`.
#[derive(Component, Debug, Clone, Copy)]
pub struct ScentSignpostOverlay {
    pub bucket_x: usize,
    pub bucket_y: usize,
}

/// Per-player visibility toggle, distinct from the per-tick
/// intensity-driven Visibility writes. When disabled, the update
/// system skips all entities. Default `false` — opt-in overlay,
/// F5 to flip on.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ScentSignpostsEnabled(pub bool);

/// Z-layer for the signpost overlay (between grass z=3.0 and
/// corruption haze z=4.0).
pub const SCENT_SIGNPOST_Z: f32 = 3.5;
/// Alpha scale applied to `CatScentMap` intensity. Keeps the overlay
/// readable without obscuring tile graphics underneath.
const SIGNPOST_ALPHA_SCALE: f32 = 0.55;
/// Hide a signpost whose intensity-driven alpha would fall below this
/// value — prevents flicker at the bucket-decay tail.
const SIGNPOST_HIDE_THRESHOLD: f32 = 0.05;

/// Startup system: one Sprite per `CatScentMap` bucket, hidden until
/// the update system writes a visible alpha.
pub fn spawn_scent_signposts(
    mut commands: Commands,
    scent_map: Res<CatScentMap>,
    tilemap: Res<TileMap>,
    sprite_assets: Res<SpriteAssets>,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let half_bucket = scent_map.bucket_size as f32 * 0.5;
    let sprite_size = scent_map.bucket_size as f32 * world_px * 0.85;
    let map_h = tilemap.height as f32;

    for by in 0..scent_map.grid_h {
        for bx in 0..scent_map.grid_w {
            // Bucket centroid in tile coordinates.
            let tile_cx = bx as f32 * scent_map.bucket_size as f32 + half_bucket;
            let tile_cy = by as f32 * scent_map.bucket_size as f32 + half_bucket;
            // World-space (y-flipped to match screen orientation, same
            // convention as base terrain at tilemap_sync.rs:104).
            let world_x = tile_cx * world_px;
            let world_y = (map_h - 1.0 - tile_cy) * world_px;

            commands.spawn((
                Sprite {
                    image: sprite_assets.white_pixel.clone(),
                    custom_size: Some(Vec2::splat(sprite_size)),
                    color: Color::srgba(0.85, 0.7, 0.2, 0.0),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, SCENT_SIGNPOST_Z),
                Visibility::Hidden,
                ScentSignpostOverlay {
                    bucket_x: bx,
                    bucket_y: by,
                },
            ));
        }
    }
}

/// Per-tick: modulate `Sprite.color` alpha from `CatScentMap` intensity.
///
/// When `ScentSignpostsEnabled.0` is false, the system early-exits —
/// no Visibility/Sprite writes happen, the user-toggle dominates.
pub fn update_scent_signposts(
    enabled: Res<ScentSignpostsEnabled>,
    scent_map: Res<CatScentMap>,
    mut overlays: Query<(&ScentSignpostOverlay, &mut Sprite, &mut Visibility)>,
) {
    if !enabled.0 {
        return;
    }
    let grid_w = scent_map.grid_w;
    let grid_h = scent_map.grid_h;
    for (overlay, mut sprite, mut visibility) in &mut overlays {
        if overlay.bucket_x >= grid_w || overlay.bucket_y >= grid_h {
            continue;
        }
        let idx = overlay.bucket_y * grid_w + overlay.bucket_x;
        let intensity = scent_map.marks[idx];
        let alpha = (intensity * SIGNPOST_ALPHA_SCALE).clamp(0.0, SIGNPOST_ALPHA_SCALE);
        let new_vis = if alpha >= SIGNPOST_HIDE_THRESHOLD {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != new_vis {
            *visibility = new_vis;
        }
        sprite.color = Color::srgba(0.85, 0.7, 0.2, alpha);
    }
}

/// F5 toggles the signpost overlay. When toggled off, all signposts
/// snap to Hidden (the update system stops writing Visibility while
/// disabled).
pub fn toggle_scent_signposts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut enabled: ResMut<ScentSignpostsEnabled>,
    mut overlays: Query<&mut Visibility, With<ScentSignpostOverlay>>,
) {
    if !keyboard.just_pressed(KeyCode::F5) {
        return;
    }
    enabled.0 = !enabled.0;
    eprintln!(
        "Toggle: Cat-scent signposts ({})",
        if enabled.0 { "ON" } else { "OFF" }
    );
    if !enabled.0 {
        for mut v in &mut overlays {
            *v = Visibility::Hidden;
        }
    }
}
