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

    // -- Wildlife sprites --

    /// Minifolks fox — 192x192, 32x32 frames (6 cols x 6 rows).
    pub fox_texture: Handle<Image>,
    pub fox_layout: Handle<TextureAtlasLayout>,

    /// Bald eagle / hawk — 64x16, 16x16 frames (4 cols x 1 row). Idle animation.
    pub hawk_texture: Handle<Image>,
    pub hawk_layout: Handle<TextureAtlasLayout>,

    /// Snake — 160x320, 16x16 frames (10 cols x 20 rows). Directional animations.
    pub snake_texture: Handle<Image>,
    pub snake_layout: Handle<TextureAtlasLayout>,

    /// Recolored MiniWolf (purple/pink) — 224x256, 32x32 frames (7 cols x 8 rows).
    pub shadow_fox_texture: Handle<Image>,
    pub shadow_fox_layout: Handle<TextureAtlasLayout>,

    // -- Prey sprites --

    /// Rat — 160x256, 16x16 frames (10 cols x 16 rows). Also reused for mouse with tint.
    pub rat_texture: Handle<Image>,
    pub rat_layout: Handle<TextureAtlasLayout>,

    /// Minifolks rabbit — 128x128, 32x32 frames (4 cols x 4 rows).
    pub rabbit_texture: Handle<Image>,
    pub rabbit_layout: Handle<TextureAtlasLayout>,

    /// Minifolks bird — 64x48, 16x16 frames (4 cols x 3 rows).
    pub bird_texture: Handle<Image>,
    pub bird_layout: Handle<TextureAtlasLayout>,

    /// Fish — 16x16 single static frame.
    pub fish_texture: Handle<Image>,
}

pub fn load_sprite_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
) {
    let white_pixel = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
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

    let items_texture = asset_server
        .load("sprites/Sprout Lands - Sprites - premium pack/Objects/Items/items-spritesheet.png");
    let items_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        8,
        15,
        None,
        None,
    ));

    let well_texture =
        asset_server.load("sprites/Sprout Lands - Sprites - premium pack/Objects/Water well.png");

    // Wildlife sprites
    let fox_texture = asset_server.load("sprites/wildlife/fox.png");
    let fox_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(32),
        6,
        6,
        None,
        None,
    ));

    let hawk_texture = asset_server.load("sprites/wildlife/hawk.png");
    let hawk_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(16, 16),
        4,
        1,
        None,
        None,
    ));

    let snake_texture = asset_server.load("sprites/wildlife/snake.png");
    let snake_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        10,
        20,
        None,
        None,
    ));

    let shadow_fox_texture = asset_server.load("sprites/wildlife/shadow_fox.png");
    let shadow_fox_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(32),
        7,
        8,
        None,
        None,
    ));

    // Prey sprites
    let rat_texture = asset_server.load("sprites/prey/rat.png");
    let rat_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        10,
        16,
        None,
        None,
    ));

    let rabbit_texture = asset_server.load("sprites/prey/rabbit.png");
    let rabbit_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(32),
        4,
        4,
        None,
        None,
    ));

    let bird_texture = asset_server.load("sprites/prey/bird.png");
    let bird_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        4,
        3,
        None,
        None,
    ));

    let fish_texture = asset_server.load("sprites/prey/fish.png");

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
        fox_texture,
        fox_layout,
        hawk_texture,
        hawk_layout,
        snake_texture,
        snake_layout,
        shadow_fox_texture,
        shadow_fox_layout,
        rat_texture,
        rat_layout,
        rabbit_texture,
        rabbit_layout,
        bird_texture,
        bird_layout,
        fish_texture,
    });
}
