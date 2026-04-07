use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::rendering::terrain_sprites::{
    base_tile_index, cardinal_bitmask, grass_overlay_atlas_index, has_grass_overlay,
};
use crate::resources::map::TileMap;

/// Marker component for the base terrain layer.
#[derive(Component)]
pub struct BaseTerrainLayer;

/// Marker component for the grass overlay layer.
#[derive(Component)]
pub struct GrassOverlayLayer;

/// Tile scale factor: 16px sprites rendered at this multiplier.
pub const TILE_SCALE: f32 = 3.0;
/// Pixel size of each tile in the sprite sheet.
pub const TILE_PX: f32 = 16.0;

/// Startup system: creates the two-layer tilemap from the TileMap resource.
pub fn create_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<TileMap>,
) {
    let map_width = map.width as u32;
    let map_height = map.height as u32;
    let map_size = TilemapSize { x: map_width, y: map_height };
    let tile_size = TilemapTileSize { x: TILE_PX, y: TILE_PX };
    let grid_size = TilemapGridSize { x: TILE_PX, y: TILE_PX };

    // --- Base terrain layer (z=0.0) ---
    let base_texture: Handle<Image> =
        asset_server.load("sprites/base_terrain_atlas.png");

    let base_tilemap_entity = commands.spawn_empty().id();
    let mut base_storage = TileStorage::empty(map_size);

    for y in 0..map_height {
        for x in 0..map_width {
            let tile_pos = TilePos { x, y };
            let terrain = &map.get(x as i32, y as i32).terrain;
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(base_tile_index(terrain)),
                    tilemap_id: TilemapId(base_tilemap_entity),
                    ..Default::default()
                })
                .id();
            base_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(base_tilemap_entity).insert((
        TilemapBundle {
            grid_size,
            map_type: TilemapType::Square,
            size: map_size,
            spacing: TilemapSpacing::zero(),
            storage: base_storage,
            texture: TilemapTexture::Single(base_texture),
            tile_size,
            transform: Transform::from_scale(Vec3::splat(TILE_SCALE)),
            ..Default::default()
        },
        BaseTerrainLayer,
    ));

    // --- Grass overlay layer (z=1.0) ---
    let grass_texture: Handle<Image> =
        asset_server.load("sprites/grass_autotile_atlas.png");

    let grass_tilemap_entity = commands.spawn_empty().id();
    let mut grass_storage = TileStorage::empty(map_size);

    for y in 0..map_height {
        for x in 0..map_width {
            let tile_pos = TilePos { x, y };
            let terrain = &map.get(x as i32, y as i32).terrain;

            if !has_grass_overlay(terrain) {
                continue;
            }

            let bitmask = cardinal_bitmask(&map, x as i32, y as i32, terrain.group());
            let atlas_index = grass_overlay_atlas_index(bitmask);

            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(atlas_index),
                    tilemap_id: TilemapId(grass_tilemap_entity),
                    ..Default::default()
                })
                .id();
            grass_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(grass_tilemap_entity).insert((
        TilemapBundle {
            grid_size,
            map_type: TilemapType::Square,
            size: map_size,
            spacing: TilemapSpacing::zero(),
            storage: grass_storage,
            texture: TilemapTexture::Single(grass_texture),
            tile_size,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 1.0),
                scale: Vec3::splat(TILE_SCALE),
                ..Default::default()
            },
            ..Default::default()
        },
        GrassOverlayLayer,
    ));
}
