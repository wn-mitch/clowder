use bevy::prelude::*;

use crate::rendering::sprite_assets::SpriteAssets;
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

/// Marker for decorative tree sprites placed on forest terrain.
#[derive(Component)]
pub struct TreeDecoration;

/// Marker for corruption haze overlay sprites.
#[derive(Component)]
pub struct CorruptionOverlay;

/// Tile scale factor: 16px sprites rendered at this multiplier.
pub const TILE_SCALE: f32 = 3.0;
/// Pixel size of each tile in the sprite sheet.
pub const TILE_PX: f32 = 16.0;

/// Startup system: creates the multi-layer tilemap from the TileMap resource.
///
/// The base terrain layer uses plain Bevy Sprites (one per tile) because
/// bevy_ecs_tilemap's texture array pipeline doesn't reliably render
/// different TileTextureIndex values on macOS Metal. Overlay layers still
/// use bevy_ecs_tilemap since they each use a single atlas texture (index 0
/// works fine).
pub fn create_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<TileMap>,
    sprite_assets: Res<SpriteAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let world_px = TILE_PX * TILE_SCALE;

    // --- Base terrain layer (z=0.0) — plain Bevy Sprites ---
    let base_tile_images: Vec<Handle<Image>> = [
        "sprites/tiles/grass.png",
        "sprites/tiles/water.png",
        "sprites/tiles/dirt.png",
        "sprites/tiles/sand.png",
        "sprites/tiles/rock.png",
        "sprites/tiles/stone.png",
        "sprites/tiles/building.png",
    ]
    .iter()
    .map(|p| asset_server.load(*p))
    .collect();

    for y in 0..map.height {
        for x in 0..map.width {
            let terrain = &map.get(x, y).terrain;
            let idx = base_tile_index(terrain) as usize;
            let world_x = x as f32 * world_px;
            let world_y = (map.height as f32 - 1.0 - y as f32) * world_px;
            commands.spawn((
                Sprite {
                    image: base_tile_images[idx].clone(),
                    custom_size: Some(Vec2::splat(world_px)),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 0.0),
                BaseTerrainLayer,
            ));
        }
    }

    // --- Blob autotile overlay layers — plain Bevy Sprites with TextureAtlas ---
    // (bevy_ecs_tilemap's texture_2d_array pipeline is broken on macOS Metal,
    //  so we use Bevy's built-in sprite atlas system instead.)

    // Build atlas layouts (one per unique atlas image). All overlay atlases
    // share the same 8×8 grid of 16×16 tiles.
    let overlay_cols = 8;
    let overlay_rows = 8;
    let overlay_layout = TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_PX as u32),
        overlay_cols,
        overlay_rows,
        None,
        None,
    );
    let overlay_layout_handle = texture_atlas_layouts.add(overlay_layout);

    struct PendingLayer {
        atlas_path: &'static str,
        z: f32,
        groups: Vec<crate::rendering::terrain_sprites::TerrainGroup>,
    }

    let mut pending: Vec<PendingLayer> = Vec::new();
    for overlay in OVERLAY_LAYERS {
        if let Some(layer) = pending
            .iter_mut()
            .find(|l| l.atlas_path == overlay.atlas_path && (l.z - overlay.z).abs() < f32::EPSILON)
        {
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
        let atlas_image: Handle<Image> = asset_server.load(layer.atlas_path);

        for y in 0..map.height {
            for x in 0..map.width {
                let terrain = &map.get(x, y).terrain;
                if !layer.groups.contains(&terrain.group()) {
                    continue;
                }

                let bitmask = blob_bitmask(&map, x, y, terrain.group());
                let atlas_index = grass_overlay_atlas_index_with_variant(bitmask, x, y);

                let world_x = x as f32 * world_px;
                let world_y = (map.height as f32 - 1.0 - y as f32) * world_px;

                commands.spawn((
                    Sprite {
                        image: atlas_image.clone(),
                        custom_size: Some(Vec2::splat(world_px)),
                        texture_atlas: Some(TextureAtlas {
                            layout: overlay_layout_handle.clone(),
                            index: atlas_index as usize,
                        }),
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, layer.z),
                    BlobOverlayLayer,
                ));
            }
        }
    }

    // --- Tree decorations on forest terrain (z=5.0) ---
    // tree_sprites.png is 12 cols x 4 rows of 48x48 frames.
    // Row 2 (indices 24-35): medium trees for LightForest.
    // Row 3 (indices 36-47): full-grown trees for DenseForest.
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get(x, y);
            let base_index = match tile.terrain {
                Terrain::LightForest => 24, // medium trees (row 2)
                Terrain::DenseForest => 36, // full trees (row 3)
                _ => continue,
            };
            // Deterministic per-tile variation: pick from 3 frames.
            let variant = ((x.wrapping_mul(7) ^ y.wrapping_mul(13)) % 3) as usize;
            let atlas_index = base_index + variant;

            let world_x = x as f32 * world_px;
            let world_y = (map.height as f32 - 1.0 - y as f32) * world_px;

            commands.spawn((
                Sprite {
                    image: sprite_assets.trees_texture.clone(),
                    custom_size: Some(Vec2::splat(world_px)),
                    texture_atlas: Some(TextureAtlas {
                        layout: sprite_assets.trees_layout.clone(),
                        index: atlas_index,
                    }),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 5.0),
                TreeDecoration,
            ));
        }
    }

    // --- Corruption haze overlay (z=4.0) ---
    // Semi-transparent dark magenta on tiles with corruption > 0.
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get(x, y);
            if tile.corruption <= 0.0 {
                continue;
            }
            let world_x = x as f32 * world_px;
            let world_y = (map.height as f32 - 1.0 - y as f32) * world_px;
            commands.spawn((
                Sprite {
                    image: sprite_assets.white_pixel.clone(),
                    color: Color::srgba(0.4, 0.0, 0.25, tile.corruption * 0.35),
                    custom_size: Some(Vec2::splat(world_px)),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 4.0),
                CorruptionOverlay,
            ));
        }
    }

    dump_terrain_debug(&map);
}

fn dump_terrain_debug(map: &TileMap) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create("/tmp/clowder_terrain.txt") else {
        return;
    };
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
