use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::rendering::terrain_sprites::{
    base_tile_index, blob_bitmask, grass_overlay_atlas_index_with_variant, OVERLAY_LAYERS,
};
use crate::resources::map::{Terrain, TileMap};

/// Marker component for the base terrain layer.
#[derive(Component)]
pub struct BaseTerrainLayer;

/// Marker component for a blob autotile overlay layer.
#[derive(Component)]
pub struct BlobOverlayLayer;

/// Tile scale factor: 16px sprites rendered at this multiplier.
pub const TILE_SCALE: f32 = 3.0;
/// Pixel size of each tile in the sprite sheet.
pub const TILE_PX: f32 = 16.0;

/// Startup system: creates the multi-layer tilemap from the TileMap resource.
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
            let tile_pos = TilePos { x, y: map_height - 1 - y };
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

    // --- Blob autotile overlay layers ---
    // Collect unique (atlas_path, z) combinations, then build each layer fully
    // before inserting the TilemapBundle (avoids TileStorage replacement issues).
    struct PendingLayer {
        atlas_path: &'static str,
        z: f32,
        groups: Vec<crate::rendering::terrain_sprites::TerrainGroup>,
    }

    let mut pending: Vec<PendingLayer> = Vec::new();
    for overlay in OVERLAY_LAYERS {
        if let Some(layer) = pending.iter_mut().find(|l| {
            l.atlas_path == overlay.atlas_path && (l.z - overlay.z).abs() < f32::EPSILON
        }) {
            layer.groups.push(overlay.group);
        } else {
            pending.push(PendingLayer {
                atlas_path: overlay.atlas_path,
                z: overlay.z,
                groups: vec![overlay.group],
            });
        }
    }

    for layer in &pending {
        let texture: Handle<Image> = asset_server.load(layer.atlas_path);
        let tilemap_entity = commands.spawn_empty().id();
        let mut storage = TileStorage::empty(map_size);

        for y in 0..map_height {
            for x in 0..map_width {
                let terrain = &map.get(x as i32, y as i32).terrain;
                if !layer.groups.contains(&terrain.group()) {
                    continue;
                }

                let tile_pos = TilePos { x, y: map_height - 1 - y };
                let bitmask = blob_bitmask(&map, x as i32, y as i32, terrain.group());
                let atlas_index = grass_overlay_atlas_index_with_variant(
                    bitmask, x as i32, y as i32,
                );

                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        texture_index: TileTextureIndex(atlas_index),
                        tilemap_id: TilemapId(tilemap_entity),
                        ..Default::default()
                    })
                    .id();
                storage.set(&tile_pos, tile_entity);
            }
        }

        commands.entity(tilemap_entity).insert((
            TilemapBundle {
                grid_size,
                map_type: TilemapType::Square,
                size: map_size,
                spacing: TilemapSpacing::zero(),
                storage,
                texture: TilemapTexture::Single(texture),
                tile_size,
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, layer.z),
                    scale: Vec3::splat(TILE_SCALE),
                    ..Default::default()
                },
                ..Default::default()
            },
            BlobOverlayLayer,
        ));
    }

    dump_terrain_debug(&map);
}

fn dump_terrain_debug(map: &TileMap) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create("/tmp/clowder_terrain.txt") else { return };
    for y in 0..map.height {
        for x in 0..map.width {
            let t = &map.get(x, y).terrain;
            let ch = match t {
                Terrain::Grass => '.',
                Terrain::LightForest => 't',
                Terrain::DenseForest => 'T',
                Terrain::Water => '~',
                Terrain::Rock => '#',
                Terrain::Mud => ',',
                Terrain::Sand => ':',
                Terrain::Den => 'D',
                Terrain::Hearth => 'H',
                Terrain::Stores => 'S',
                Terrain::Workshop => 'W',
                Terrain::Garden => 'G',
                _ => '?',
            };
            let _ = write!(f, "{ch}");
        }
        let _ = writeln!(f);
    }
    eprintln!("Terrain dump → /tmp/clowder_terrain.txt");
}
