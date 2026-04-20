use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

// ---------------------------------------------------------------------------
// Tree sprite pool — varied individual PNGs from the Fan-tasy tileset
// ---------------------------------------------------------------------------

/// A single tree sprite with rendering metadata.
pub struct TreeSpriteEntry {
    pub image: Handle<Image>,
    /// Native pixel width of the source PNG.
    pub native_w: f32,
    /// Native pixel height of the source PNG.
    pub native_h: f32,
    /// Target render height as a fraction of world_px.
    pub height_scale: f32,
}

impl TreeSpriteEntry {
    /// Compute render size preserving native aspect ratio.
    pub fn render_size(&self, world_px: f32) -> Vec2 {
        let h = self.height_scale * world_px;
        let w = h * (self.native_w / self.native_h);
        Vec2::new(w, h)
    }
}

/// A ground scatter prop sprite with rendering metadata.
pub struct ScatterEntry {
    pub image: Handle<Image>,
    pub native_w: f32,
    pub native_h: f32,
    /// Target render height as a fraction of world_px.
    pub height_scale: f32,
}

impl ScatterEntry {
    pub fn render_size(&self, world_px: f32) -> Vec2 {
        let h = self.height_scale * world_px;
        let w = h * (self.native_w / self.native_h);
        Vec2::new(w, h)
    }
}

/// A color-coherent group of tree sprites (e.g. all "Dark" variants).
pub struct ForestPalette {
    pub entries: Vec<TreeSpriteEntry>,
}

/// Pre-loaded tree sprites for varied forest rendering, grouped by color
/// palette so that nearby tiles draw from the same color family.
#[derive(Resource)]
pub struct TreeSpritePool {
    /// Palettes for LightForest tiles, one per color (Dark, Emerald, Light).
    pub light_forest: Vec<ForestPalette>,
    /// Palettes for DenseForest tiles, one per color (Dark, Emerald, Light).
    pub dense_forest: Vec<ForestPalette>,
    /// Shadow sprite (round oval, rendered semi-transparent under each tree).
    pub shadow: Handle<Image>,
    /// Small decorative props for ground scatter.
    pub scatter: Vec<ScatterEntry>,
}

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

    /// Colony well — Fan-tasy Tileset hay well, 56x74.
    pub well_texture: Handle<Image>,

    // -- Wildlife sprites --
    /// Minifolks fox — 192x192, 32x32 frames (6 cols x 6 rows).
    pub fox_texture: Handle<Image>,
    pub fox_layout: Handle<TextureAtlasLayout>,

    /// Bald Eagle — 64x16, 16x16 frames (4 cols x 1 row). 4-frame idle animation.
    pub hawk_texture: Handle<Image>,
    pub hawk_layout: Handle<TextureAtlasLayout>,

    /// Snake — 160x320, 16x16 frames (10 cols x 20 rows). Directional animations.
    pub snake_texture: Handle<Image>,
    pub snake_layout: Handle<TextureAtlasLayout>,

    /// Arctic Wolf — 64x16, 16x16 frames (4 cols x 1 row). 4-frame idle animation.
    pub shadow_fox_texture: Handle<Image>,
    pub shadow_fox_layout: Handle<TextureAtlasLayout>,

    // -- Prey sprites --
    /// Rat/Mouse — 160x256, 16x16 frames (10 cols x 16 rows). Mouse reuses with lighter tint.
    pub rat_texture: Handle<Image>,
    pub rat_layout: Handle<TextureAtlasLayout>,

    /// Minifolks rabbit — 128x128, 32x32 frames (4 cols x 4 rows).
    pub rabbit_texture: Handle<Image>,
    pub rabbit_layout: Handle<TextureAtlasLayout>,

    /// Bird variants for visual variety. Each is 64x16, 16x16 frames (4 cols x 1 row).
    /// Randomly selected per entity at spawn time.
    pub bird_textures: Vec<Handle<Image>>,
    pub bird_anim_layout: Handle<TextureAtlasLayout>,

    /// Fish — 16x16 single-frame sprites. Two color variants (orange, yellow).
    pub fish_textures: Vec<Handle<Image>>,

    /// Ancient-ruin rune pair — 144x32, 16x16 frames (9 cols x 2 rows).
    /// Row 0 = left half, row 1 = right half. Animation order is authored;
    /// see `RUNE_ANIMATION_STEPS` in `tilemap_sync`.
    pub ruin_rune_texture: Handle<Image>,
    pub ruin_rune_layout: Handle<TextureAtlasLayout>,

    // -- Building sprites (Fan-tasy Tileset) --
    /// Den — small hay houses, 3 variants for per-entity variety. 89x91 each.
    pub den_textures: [Handle<Image>; 3],
    /// Hearth — colored medium house. 128x128.
    pub hearth_texture: Handle<Image>,
    /// Stores — market stand. 62x57.
    pub stores_texture: Handle<Image>,
    /// Workshop — tool stand prop. 35x44.
    pub workshop_texture: Handle<Image>,
    /// Garden — basket props for CropState stages (empty, growing, harvestable).
    pub garden_textures: [Handle<Image>; 3],
    /// Watchtower — tall hay watchtower. 68x149.
    pub watchtower_texture: Handle<Image>,
    /// WardPost — banner on stick. 24x59.
    pub wardpost_texture: Handle<Image>,
    /// Ward entity — lantern on stick prop. 16x32.
    pub ward_texture: Handle<Image>,
    /// Wall — single city wall segment. 16x48. Directional autotile deferred.
    pub wall_texture: Handle<Image>,
    /// Gate — city wall gate. 80x96.
    pub gate_texture: Handle<Image>,

    // -- Snow building variants (winter seasonal swap) --
    pub den_snow_textures: [Handle<Image>; 3],
    pub hearth_snow_texture: Handle<Image>,
    pub stores_snow_texture: Handle<Image>,
    pub watchtower_snow_texture: Handle<Image>,
    pub wardpost_snow_texture: Handle<Image>,
    pub well_snow_texture: Handle<Image>,

    // -- Weather VFX spritesheets (Pixel Art Atmospheric) --
    // Each is 15360x180 (48 frames of 320x180). Shared atlas layout.
    pub weather_rain_texture: Handle<Image>,
    pub weather_snow_texture: Handle<Image>,
    pub weather_wind_texture: Handle<Image>,
    pub weather_autumn_leaves_texture: Handle<Image>,
    pub weather_fireflies_texture: Handle<Image>,
    pub weather_god_rays_texture: Handle<Image>,
    pub weather_fire_embers_texture: Handle<Image>,
    pub weather_sakura_texture: Handle<Image>,
    pub weather_meteors_texture: Handle<Image>,
    pub weather_tornado_texture: Handle<Image>,
    /// Shared atlas layout for all weather spritesheets: 48 cols x 1 row, 320x180.
    pub weather_layout: Handle<TextureAtlasLayout>,
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

    let well_texture = asset_server.load(
        "new_sprites/The Fan-tasy Tileset - Turning of the Seasons/Art/Buildings/Well_Hay_1.png",
    );

    // Wildlife sprites
    let fox_texture = asset_server.load("sprites/wildlife/fox.png");
    let fox_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(32),
        6,
        6,
        None,
        None,
    ));

    // Hawk → Bald Eagle (4-frame 16x16 animation strip)
    let hawk_texture = asset_server
        .load("new_sprites/Premium Asset Pack/Premium Animal Animations/Bald Eagle/BaldEagle.png");
    let hawk_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        4,
        1,
        None,
        None,
    ));

    // Snake → new_sprites version (better directional art)
    let snake_texture = asset_server.load("new_sprites/Snake_Sprites.png");
    let snake_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        10,
        20,
        None,
        None,
    ));

    // ShadowFox → Arctic Wolf (4-frame 16x16 animation strip)
    let shadow_fox_texture = asset_server.load(
        "new_sprites/Supporter Asset Pack/Supporter Animal Animations/Arctic Wolf/ArcticWolf.png",
    );
    let shadow_fox_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        4,
        1,
        None,
        None,
    ));

    // Prey sprites

    // Rat/Mouse → new_sprites version (better art, same 10x16 grid)
    let rat_texture = asset_server.load("new_sprites/Rat_Sprites.png");
    let rat_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        10,
        16,
        None,
        None,
    ));

    let rabbit_texture =
        asset_server.load("new_sprites/MinifolksForestAnimals/Without outline/MiniBunny.png");
    let rabbit_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(32),
        4,
        4,
        None,
        None,
    ));

    // Bird varieties — randomly selected per entity for forest visual diversity
    let bird_anim_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        4,
        1,
        None,
        None,
    ));
    let bird_textures = vec![
        asset_server.load(
            "new_sprites/Supporter Asset Pack/Supporter Animal Animations/Chirping Bird/ChirpingBird.png",
        ),
        asset_server.load(
            "new_sprites/Supporter Asset Pack/Supporter Animal Animations/Blue Jay/BlueJay.png",
        ),
        asset_server.load(
            "new_sprites/Premium Asset Pack/Premium Animal Animations/Wise Owl/WiseOwl.png",
        ),
    ];

    // Fish — two single-frame color variants
    let fish_textures = vec![
        asset_server.load("new_sprites/Fish 0021.png"),
        asset_server.load("new_sprites/Fish Png1.png"),
    ];

    // Ancient-ruin rune atlas — built by tools/build_rune_atlas.py from the
    // Fan-tasy Tileset animation sheet. 9 unique frames x 2 halves.
    let ruin_rune_texture = asset_server.load("sprites/rune_rock_gray_atlas.png");
    let ruin_rune_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(16),
        9,
        2,
        None,
        None,
    ));

    // -----------------------------------------------------------------------
    // Building sprites — Fan-tasy Tileset
    // -----------------------------------------------------------------------

    let seasons = "new_sprites/The Fan-tasy Tileset - Turning of the Seasons/Art";
    let premium = "new_sprites/The Fan-tasy Tileset (Premium)/Art";
    let snow_pack = "new_sprites/The Fan-tasy Tileset - Snow Adventures/Art";

    // Den — 3 small hay house variants (89x91 each)
    let den_textures = [
        asset_server.load(format!("{seasons}/Buildings/House_Hay_1.png")),
        asset_server.load(format!("{seasons}/Buildings/House_Hay_2.png")),
        asset_server.load(format!("{seasons}/Buildings/House_Hay_3.png")),
    ];
    let hearth_texture = asset_server.load(format!("{seasons}/Buildings/House_Hay_4_Red.png"));
    let stores_texture = asset_server.load(format!("{seasons}/Buildings/MarketStand_1_Red.png"));
    let workshop_texture = asset_server.load(format!("{premium}/Props/ToolsStand_1.png"));
    let garden_textures = [
        asset_server.load(format!("{premium}/Props/Basket_Empty.png")),
        asset_server.load(format!("{premium}/Props/Basket_Cotton.png")),
        asset_server.load(format!("{premium}/Props/Basket_Vegetables.png")),
    ];
    let watchtower_texture =
        asset_server.load(format!("{seasons}/Buildings/Watchtower_1_Hay_Red.png"));
    let wardpost_texture = asset_server.load(format!("{seasons}/Props/Banner_Stick_1_Red.png"));
    let ward_texture = asset_server.load(format!("{premium}/Props/Lantern_2.png"));
    let wall_texture = asset_server.load(format!("{premium}/Fences and Walls/CityWall_Up_1.png"));
    let gate_texture = asset_server.load(format!("{premium}/Fences and Walls/CityWall_Gate_1.png"));

    // Snow variants for winter seasonal swap
    let den_snow_textures = [
        asset_server.load(format!("{snow_pack}/Buildings/House_Snow_1_1.png")),
        asset_server.load(format!("{snow_pack}/Buildings/House_Snow_1_2.png")),
        asset_server.load(format!("{snow_pack}/Buildings/House_Snow_1_3.png")),
    ];
    let hearth_snow_texture =
        asset_server.load(format!("{snow_pack}/Buildings/House_Snow_5_1_Red.png"));
    let stores_snow_texture =
        asset_server.load(format!("{snow_pack}/Buildings/MarketStand_1_Red.png"));
    let watchtower_snow_texture =
        asset_server.load(format!("{snow_pack}/Buildings/Watchtower_1_Snow_Red.png"));
    let wardpost_snow_texture =
        asset_server.load(format!("{snow_pack}/Props/Banner_Stick_1_Red_Snow.png"));
    let well_snow_texture = asset_server.load(format!("{snow_pack}/Buildings/Well_Snow_1.png"));

    // -----------------------------------------------------------------------
    // Weather VFX — Pixel Art Atmospheric spritesheets
    // -----------------------------------------------------------------------

    let atmo = "new_sprites/Pixel Art Atmospheric/Pixel Art Atmospheric/SpriteSheet";

    let weather_rain_texture = asset_server.load(format!("{atmo}/clima_lluvia_estetica.png"));
    let weather_snow_texture = asset_server.load(format!("{atmo}/clima_nieve_cozy.png"));
    let weather_wind_texture = asset_server.load(format!("{atmo}/clima_viento_estetico.png"));
    let weather_autumn_leaves_texture = asset_server.load(format!("{atmo}/clima_hojas_autumn.png"));
    let weather_fireflies_texture = asset_server.load(format!("{atmo}/clima_luciernagas_cozy.png"));
    let weather_god_rays_texture = asset_server.load(format!("{atmo}/clima_godrays.png"));
    let weather_fire_embers_texture = asset_server.load(format!("{atmo}/clima_chispas_fuego.png"));
    let weather_sakura_texture = asset_server.load(format!("{atmo}/clima_sakura.png"));
    let weather_meteors_texture = asset_server.load(format!("{atmo}/clima_meteoritos.png"));
    let weather_tornado_texture = asset_server.load(format!("{atmo}/clima_tornado_epico.png"));
    let weather_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(320, 180),
        48,
        1,
        None,
        None,
    ));

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
        bird_textures,
        bird_anim_layout,
        fish_textures,
        ruin_rune_texture,
        ruin_rune_layout,
        // Buildings
        den_textures,
        hearth_texture,
        stores_texture,
        workshop_texture,
        garden_textures,
        watchtower_texture,
        wardpost_texture,
        ward_texture,
        wall_texture,
        gate_texture,
        // Snow variants
        den_snow_textures,
        hearth_snow_texture,
        stores_snow_texture,
        watchtower_snow_texture,
        wardpost_snow_texture,
        well_snow_texture,
        // Weather VFX
        weather_rain_texture,
        weather_snow_texture,
        weather_wind_texture,
        weather_autumn_leaves_texture,
        weather_fireflies_texture,
        weather_god_rays_texture,
        weather_fire_embers_texture,
        weather_sakura_texture,
        weather_meteors_texture,
        weather_tornado_texture,
        weather_layout,
    });
}

// ---------------------------------------------------------------------------
// Tree sprite pool loader
// ---------------------------------------------------------------------------

const FANTASY_TREES: &str = "new_sprites/The Fan-tasy Tileset (Premium)/Art/Trees and Bushes";
const FANTASY_SHADOWS: &str = "new_sprites/The Fan-tasy Tileset (Premium)/Art/Shadows";
const FANTASY_PROPS: &str = "new_sprites/The Fan-tasy Tileset (Premium)/Art/Props";

/// Startup system: pre-load varied tree, shadow, and scatter sprites.
///
/// Sprites are grouped by color palette (Dark, Emerald, Light) so that
/// the renderer can pick one palette per spatial region, keeping forests
/// visually coherent while still varying tree shapes within each region.
pub fn load_tree_sprite_pool(mut commands: Commands, asset_server: Res<AssetServer>) {
    let colors = ["Dark", "Emerald", "Light"];

    let mut light_forest: Vec<ForestPalette> = colors
        .iter()
        .map(|_| ForestPalette {
            entries: Vec::new(),
        })
        .collect();
    let mut dense_forest: Vec<ForestPalette> = colors
        .iter()
        .map(|_| ForestPalette {
            entries: Vec::new(),
        })
        .collect();

    // --- Birch (both pools) — tall, narrow deciduous ---
    let birch: &[(u32, f32, f32)] = &[
        (1, 52.0, 92.0),
        (2, 48.0, 93.0),
        (3, 46.0, 63.0),
        (4, 38.0, 77.0),
    ];
    for (ci, color) in colors.iter().enumerate() {
        for &(v, w, h) in birch {
            let image = asset_server.load(format!("{FANTASY_TREES}/Birch_{color}_{v}.png"));
            light_forest[ci].entries.push(TreeSpriteEntry {
                image: image.clone(),
                native_w: w,
                native_h: h,
                height_scale: 1.0,
            });
            dense_forest[ci].entries.push(TreeSpriteEntry {
                image,
                native_w: w,
                native_h: h,
                height_scale: 1.0,
            });
        }
    }

    // --- Bush (light forest only) — short, wide undergrowth ---
    let bush: &[(u32, f32, f32)] = &[
        (1, 40.0, 29.0),
        (3, 28.0, 28.0),
        (8, 26.0, 26.0),
        (9, 39.0, 41.0),
    ];
    for (ci, color) in colors.iter().enumerate() {
        for &(v, w, h) in bush {
            light_forest[ci].entries.push(TreeSpriteEntry {
                image: asset_server.load(format!("{FANTASY_TREES}/Bush_{color}_{v}.png")),
                native_w: w,
                native_h: h,
                height_scale: 0.7,
            });
        }
    }

    // --- LeavyBush (both pools) — low leafy cover ---
    let leavy: &[(u32, f32, f32)] = &[(2, 32.0, 23.0), (3, 31.0, 28.0)];
    for (ci, color) in colors.iter().enumerate() {
        for &(v, w, h) in leavy {
            let image = asset_server.load(format!("{FANTASY_TREES}/LeavyBush_{color}_{v}.png"));
            light_forest[ci].entries.push(TreeSpriteEntry {
                image: image.clone(),
                native_w: w,
                native_h: h,
                height_scale: 0.5,
            });
            dense_forest[ci].entries.push(TreeSpriteEntry {
                image,
                native_w: w,
                native_h: h,
                height_scale: 0.5,
            });
        }
    }

    // --- Tree / oak (dense forest only) — large canopy ---
    let oak: &[(u32, f32, f32)] = &[
        (1, 64.0, 63.0),
        (2, 46.0, 63.0),
        (3, 52.0, 92.0),
        (5, 97.0, 124.0),
        (6, 80.0, 110.0),
    ];
    for (ci, color) in colors.iter().enumerate() {
        for &(v, w, h) in oak {
            dense_forest[ci].entries.push(TreeSpriteEntry {
                image: asset_server.load(format!("{FANTASY_TREES}/Tree_{color}_{v}.png")),
                native_w: w,
                native_h: h,
                height_scale: 1.2,
            });
        }
    }

    // --- Shadow sprite ---
    let shadow = asset_server.load(format!(
        "{FANTASY_SHADOWS}/Shadow_Round_24x24_Medium_Black.png"
    ));

    // --- Ground scatter props ---
    let scatter_defs: &[(&str, f32, f32, f32)] = &[
        ("Plant_Mushroom_1.png", 16.0, 12.0, 0.35),
        ("Plant_Mushroom_2.png", 10.0, 13.0, 0.35),
        ("Plant_Mushroom_3.png", 12.0, 13.0, 0.35),
        ("Plant_Mushroom_Chanterelle_1.png", 14.0, 11.0, 0.3),
        ("FloatingGrass_1_Green.png", 7.0, 9.0, 0.25),
        ("FloatingGrass_2_Green.png", 7.0, 9.0, 0.25),
        ("FloatingGrass_3_Green.png", 7.0, 9.0, 0.25),
        ("FloatingGrass_4_Green.png", 7.0, 9.0, 0.25),
        ("Plant_1.png", 16.0, 25.0, 0.4),
        ("Plant_2.png", 15.0, 11.0, 0.3),
    ];
    let scatter = scatter_defs
        .iter()
        .map(|&(name, w, h, scale)| ScatterEntry {
            image: asset_server.load(format!("{FANTASY_PROPS}/{name}")),
            native_w: w,
            native_h: h,
            height_scale: scale,
        })
        .collect();

    commands.insert_resource(TreeSpritePool {
        light_forest,
        dense_forest,
        shadow,
        scatter,
    });
}
