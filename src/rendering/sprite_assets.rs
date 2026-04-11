use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Centralized sprite sheet handles loaded at startup.
///
/// Each field pair is (texture image, atlas layout) for a single sprite sheet.
/// Systems reference these via `Res<SpriteAssets>` to build `Sprite` +
/// `TextureAtlas` components without loading assets per-entity.
#[derive(Resource)]
pub struct SpriteAssets {
    /// 1x1 white pixel for tinted overlays (corruption, etc.)
    pub white_pixel: Handle<Image>,

    /// Premium character spritesheet — 384x1152, 48x48 frames (8 cols x 24 rows).
    /// Front-facing idle at index 0.
    pub character_texture: Handle<Image>,
    pub character_layout: Handle<TextureAtlasLayout>,

    /// Mushrooms, Flowers, Stones — 192x80, 16x16 frames (12 cols x 5 rows).
    pub herbs_texture: Handle<Image>,
    pub herbs_layout: Handle<TextureAtlasLayout>,

    /// Tree animation sprites — 576x192, 48x48 frames (12 cols x 4 rows).
    /// Row 0-1: saplings/small, Row 2: medium, Row 3: full-grown.
    pub trees_texture: Handle<Image>,
    pub trees_layout: Handle<TextureAtlasLayout>,

    /// Items spritesheet — 128x240, 16x16 frames (8 cols x 15 rows).
    /// Food, foraged goods, herbs-as-bottles, curiosities.
    pub items_texture: Handle<Image>,
    pub items_layout: Handle<TextureAtlasLayout>,

    /// Colony well — 32x32 single image (decorative landmark at colony center).
    pub well_texture: Handle<Image>,
}

pub fn load_sprite_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
) {
    let white_pixel = images.add(Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![255u8, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ));
    let character_texture = asset_server.load(
        "sprites/Sprout Lands - Sprites - premium pack/Characters/Premium Charakter Spritesheet.png",
    );
    let character_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(48),
        8,
        24,
        None,
        None,
    ));

    let herbs_texture = asset_server.load(
        "sprites/Sprout Lands - Sprites - premium pack/Objects/Mushrooms, Flowers, Stones.png",
    );
    let herbs_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        12,
        5,
        None,
        None,
    ));

    let trees_texture = asset_server.load(
        "sprites/Sprout Lands - Sprites - premium pack/Objects/Tree animations/tree_sprites.png",
    );
    let trees_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(48),
        12,
        4,
        None,
        None,
    ));

    let items_texture = asset_server.load(
        "sprites/Sprout Lands - Sprites - premium pack/Objects/Items/items-spritesheet.png",
    );
    let items_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        8,
        15,
        None,
        None,
    ));

    let well_texture = asset_server.load(
        "sprites/Sprout Lands - Sprites - premium pack/Objects/Water well.png",
    );

    commands.insert_resource(SpriteAssets {
        white_pixel,
        character_texture,
        character_layout,
        herbs_texture,
        herbs_layout,
        trees_texture,
        trees_layout,
        items_texture,
        items_layout,
        well_texture,
    });
}
