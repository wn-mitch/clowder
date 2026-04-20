use bevy::prelude::*;

use crate::rendering::sprite_assets::{SpriteAssets, TreeSpritePool};
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

/// Marker for the animated rune sprite rendered on AncientRuin footprints.
#[derive(Component)]
pub struct RuinRune;

/// 18-step glow-pulse sequence authored in Animation_Rock_Brown_EmeraldGrass.tsx
/// for tile ID 41 (left half). Entries are column indices 0..9 into
/// `ruin_rune_layout` row 0. The right half uses the same sequence against
/// row 1 of the atlas.
const RUNE_ANIMATION_STEPS: [u8; 18] = [0, 1, 2, 3, 4, 5, 0, 6, 0, 7, 8, 7, 0, 6, 0, 7, 8, 7];
const RUNE_FRAME_DURATION_MS: u64 = 250;
/// Row 1 of the 9x2 atlas holds the right-half frames; its base atlas index
/// is one row-width away from row 0.
const RUNE_RIGHT_ROW_BASE: u16 = 9;

/// Marker for decorative tree sprites placed on forest terrain.
#[derive(Component)]
pub struct TreeDecoration;

/// Marker for shadow sprites cast by trees.
#[derive(Component)]
pub struct TreeShadow;

/// Marker for small decorative ground scatter props.
#[derive(Component)]
pub struct GroundScatter;

/// Marker for corruption haze overlay sprites.
#[derive(Component)]
pub struct CorruptionOverlay;

/// Tile scale factor: 16px sprites rendered at this multiplier.
pub const TILE_SCALE: f32 = 3.0;
/// Pixel size of each tile in the sprite sheet.
pub const TILE_PX: f32 = 16.0;
/// Coarse region size (in tiles) for tree color palette coherence.
/// Each region of this size picks a single color palette (Dark/Emerald/Light),
/// while tree shape varies per-tile within that palette.
const TREE_COLOR_REGION: i32 = 8;

/// Deterministic hash for stable per-tile variation. Different `salt` values
/// produce independent sequences so tree selection, scatter placement, and
/// position jitter don't correlate.
fn tile_hash(x: i32, y: i32, salt: u32) -> u32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(salt.wrapping_mul(2654435761));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

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
    tree_pool: Res<TreeSpritePool>,
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

    // --- Ancient-ruin rune pair (z=0.5) ---
    // For each AncientRuin footprint (2x2), detect the top-left anchor and
    // spawn two animated sprites on the bottom row: tile 41 (left half) and
    // tile 42 (right half). Both cycle through the same 18-step pulse at
    // 250ms/frame using the authored TSX sequence. Z sits above the base
    // terrain but below all blob overlays so grass scalloping renders over
    // the rune's edges.
    let is_ruin = |x: i32, y: i32| -> bool {
        map.in_bounds(x, y) && map.get(x, y).terrain == Terrain::AncientRuin
    };
    for y in 0..map.height {
        for x in 0..map.width {
            if !is_ruin(x, y) {
                continue;
            }
            // Anchor = top-left tile of a 2x2 cluster.
            if is_ruin(x - 1, y) || is_ruin(x, y - 1) {
                continue;
            }
            // Bottom row world coordinates. TileMap is Y-down; visual south is y+1.
            let by = y + 1;
            for (half_idx, base_atlas_index) in [(0i32, 0u16), (1i32, RUNE_RIGHT_ROW_BASE)] {
                let tile_x = x + half_idx;
                let world_x = tile_x as f32 * world_px;
                let world_y = (map.height as f32 - 1.0 - by as f32) * world_px;
                commands.spawn((
                    Sprite {
                        image: sprite_assets.ruin_rune_texture.clone(),
                        custom_size: Some(Vec2::splat(world_px)),
                        texture_atlas: Some(TextureAtlas {
                            layout: sprite_assets.ruin_rune_layout.clone(),
                            index: base_atlas_index as usize,
                        }),
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 0.5),
                    crate::rendering::sprite_animation::AnimationTimer::new(
                        RUNE_ANIMATION_STEPS.len() as u8,
                        std::time::Duration::from_millis(RUNE_FRAME_DURATION_MS),
                    ),
                    crate::rendering::sprite_animation::AnimationSequence {
                        base: base_atlas_index,
                        steps: &RUNE_ANIMATION_STEPS,
                        cursor: 0,
                    },
                    RuinRune,
                ));
            }
        }
    }

    // --- Ground scatter layer (z=4.5) ---
    // Small decorative props (mushrooms, grass tufts) on grass/forest tiles.
    // Density varies by terrain: dense forest > light forest > open grass.
    if !tree_pool.scatter.is_empty() {
        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map.get(x, y);
                let threshold = match tile.terrain {
                    Terrain::DenseForest => 25,
                    Terrain::LightForest => 15,
                    Terrain::Grass => 10,
                    _ => continue,
                };
                if tile_hash(x, y, 1) % 100 >= threshold {
                    continue;
                }
                let idx = tile_hash(x, y, 2) as usize % tree_pool.scatter.len();
                let scatter = &tree_pool.scatter[idx];
                let size = scatter.render_size(world_px);

                // Jitter position within the tile so scatter doesn't look grid-aligned.
                let jx = ((tile_hash(x, y, 3) % 20) as f32 - 10.0) / 10.0 * world_px * 0.3;
                let jy = ((tile_hash(x, y, 4) % 20) as f32 - 10.0) / 10.0 * world_px * 0.3;
                let world_x = x as f32 * world_px + jx;
                let world_y = (map.height as f32 - 1.0 - y as f32) * world_px + jy;

                commands.spawn((
                    Sprite {
                        image: scatter.image.clone(),
                        custom_size: Some(size),
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 4.5),
                    GroundScatter,
                ));
            }
        }
    }

    // --- Tree decorations with variety (z=5.0) + shadow companions (z=4.8) ---
    // Each forest tile gets a deterministically-selected tree sprite. A coarse
    // spatial hash picks a color palette per region so nearby tiles share the
    // same color family; a fine per-tile hash picks the shape variant within
    // that palette.
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get(x, y);
            let palettes = match tile.terrain {
                Terrain::LightForest => &tree_pool.light_forest,
                Terrain::DenseForest => &tree_pool.dense_forest,
                _ => continue,
            };
            if palettes.is_empty() {
                continue;
            }

            let color_idx = tile_hash(x / TREE_COLOR_REGION, y / TREE_COLOR_REGION, 7) as usize
                % palettes.len();
            let palette = &palettes[color_idx];
            if palette.entries.is_empty() {
                continue;
            }
            let idx = tile_hash(x, y, 0) as usize % palette.entries.len();
            let entry = &palette.entries[idx];
            let size = entry.render_size(world_px);

            // Small jitter so trees don't sit on a perfect grid.
            let jx = ((tile_hash(x, y, 5) % 10) as f32 - 5.0) / 5.0 * world_px * 0.15;
            let jy = ((tile_hash(x, y, 6) % 10) as f32 - 5.0) / 5.0 * world_px * 0.15;
            let world_x = x as f32 * world_px + jx;
            let world_y = (map.height as f32 - 1.0 - y as f32) * world_px + jy;

            commands.spawn((
                Sprite {
                    image: entry.image.clone(),
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 5.0),
                TreeDecoration,
            ));

            // Shadow companion — offset south-east, sized relative to tree width.
            let shadow_w = size.x * 0.7;
            let shadow_h = shadow_w * 0.6;
            commands.spawn((
                Sprite {
                    image: tree_pool.shadow.clone(),
                    custom_size: Some(Vec2::new(shadow_w, shadow_h)),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.25),
                    ..default()
                },
                Transform::from_xyz(world_x + 0.3 * world_px, world_y - 0.2 * world_px, 4.8),
                TreeShadow,
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
                Terrain::Kitchen => 'K',
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
