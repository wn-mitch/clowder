use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Text2d;

use crate::components::identity::{Appearance, Name, Species};
use crate::components::magic::{Herb, HerbKind, Harvestable, Ward};
use crate::components::physical::{Dead, Position};
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::WildAnimal;
use crate::rendering::sprite_assets::SpriteAssets;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;

/// Marker: this entity has had rendering components attached.
#[derive(Component)]
pub struct EntitySpriteMarker;

/// Shared white pixel texture for colored rectangle sprites.
#[derive(Resource)]
pub struct WhitePixel(pub Handle<Image>);

/// Startup: create the 1x1 white pixel texture used for all entity sprites.
pub fn create_white_pixel(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let format = TextureFormat::Rgba8UnormSrgb;
    let data = vec![255u8, 255, 255, 255]; // RGBA white pixel
    let image = Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        format,
        default(),
    );
    let handle = images.add(image);
    commands.insert_resource(WhitePixel(handle));
}

/// Attach sprites to entities that have Position but no EntitySpriteMarker.
pub fn attach_entity_sprites(
    mut commands: Commands,
    white_pixel: Res<WhitePixel>,
    sprite_assets: Res<SpriteAssets>,
    map: Res<TileMap>,
    cats: Query<
        (Entity, &Position, &Appearance, &Name),
        (With<Species>, Without<EntitySpriteMarker>, Without<Dead>),
    >,
    dead_cats: Query<
        (Entity, &Position),
        (With<Species>, With<Dead>, Without<EntitySpriteMarker>),
    >,
    wildlife: Query<
        (Entity, &Position, &WildAnimal),
        Without<EntitySpriteMarker>,
    >,
    prey: Query<
        (Entity, &Position, &PreyAnimal),
        Without<EntitySpriteMarker>,
    >,
    herbs: Query<
        (Entity, &Position, &Herb),
        (With<Harvestable>, Without<EntitySpriteMarker>),
    >,
    wards: Query<
        (Entity, &Position, &Ward),
        Without<EntitySpriteMarker>,
    >,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;

    if !cats.is_empty() || !wildlife.is_empty() || !prey.is_empty() {
        eprintln!(
            "Attaching sprites: {} cats, {} dead, {} wildlife, {} prey, {} herbs, {} wards",
            cats.iter().count(), dead_cats.iter().count(),
            wildlife.iter().count(), prey.iter().count(),
            herbs.iter().count(), wards.iter().count(),
        );
    }

    // Living cats — character sprite tinted by fur color, with name label.
    for (entity, pos, appearance, name) in &cats {
        let color = fur_color_to_bevy(&appearance.fur_color);
        let (x, y) = grid_to_world(pos, map_h, world_px);

        // Name label as a child entity, offset above the sprite.
        let label = commands
            .spawn((
                Text2d::new(&name.0),
                TextFont {
                    font_size: 10.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.85)),
                Transform::from_xyz(0.0, world_px * 0.55, 1.0),
            ))
            .id();

        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.character_texture.clone(),
                color,
                custom_size: Some(Vec2::splat(world_px)),
                texture_atlas: Some(TextureAtlas {
                    layout: sprite_assets.character_layout.clone(),
                    index: 0, // front-facing idle
                }),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 20.0),
            EntitySpriteMarker,
        ));
        commands.entity(entity).add_children(&[label]);
    }

    // Dead cats — gray.
    for (entity, pos) in &dead_cats {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color: Color::srgba(0.4, 0.4, 0.4, 0.5),
                custom_size: Some(Vec2::new(world_px * 0.5, world_px * 0.5)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 19.0),
            EntitySpriteMarker,
        ));
    }

    // Wildlife — colored by species.
    for (entity, pos, animal) in &wildlife {
        let color = wildlife_color(animal);
        let (x, y) = grid_to_world(pos, map_h, world_px);
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color,
                custom_size: Some(Vec2::new(world_px * 0.7, world_px * 0.7)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 21.0),
            EntitySpriteMarker,
        ));
    }

    // Prey — small, colored by species.
    for (entity, pos, prey_animal) in &prey {
        let color = prey_color(prey_animal);
        let (x, y) = grid_to_world(pos, map_h, world_px);
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color,
                custom_size: Some(Vec2::new(world_px * 0.4, world_px * 0.4)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 18.0),
            EntitySpriteMarker,
        ));
    }

    // Herbs — distinct flower/mushroom sprite per kind.
    for (entity, pos, herb) in &herbs {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let atlas_index = herb_sprite_index(herb.kind);
        let color = if herb.twisted {
            Color::srgb(0.6, 0.15, 0.4) // corrupted: dark magenta tint
        } else {
            Color::WHITE
        };
        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.herbs_texture.clone(),
                color,
                custom_size: Some(Vec2::splat(world_px * 0.5)),
                texture_atlas: Some(TextureAtlas {
                    layout: sprite_assets.herbs_layout.clone(),
                    index: atlas_index,
                }),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 17.0),
            EntitySpriteMarker,
        ));
    }

    // Wards — cyan or red marker.
    for (entity, pos, ward) in &wards {
        let color = if ward.inverted {
            Color::srgb(0.9, 0.2, 0.2)
        } else {
            Color::srgb(0.2, 0.9, 0.9)
        };
        let (x, y) = grid_to_world(pos, map_h, world_px);
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color,
                custom_size: Some(Vec2::new(world_px * 0.3, world_px * 0.3)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 22.0),
            EntitySpriteMarker,
        ));
    }
}

/// Sync Position → Transform for all entities with both components.
pub fn sync_entity_positions(
    map: Res<TileMap>,
    mut query: Query<(Entity, &Position, &mut Transform), With<EntitySpriteMarker>>,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;

    for (entity, pos, mut transform) in &mut query {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        // Small deterministic sub-tile offset so sprites on the same tile
        // don't stack exactly and name labels stay readable.
        let hash = entity.to_bits() as f32;
        let offset_x = (hash * 7.3).sin() * 0.3 * world_px;
        let offset_y = (hash * 13.7).sin() * 0.15 * world_px;
        transform.translation.x = x + offset_x;
        transform.translation.y = y + offset_y;
    }
}

fn grid_to_world(pos: &Position, map_height: f32, world_px: f32) -> (f32, f32) {
    let x = pos.x as f32 * world_px;
    let y = (map_height - 1.0 - pos.y as f32) * world_px;
    (x, y)
}

fn fur_color_to_bevy(fur: &str) -> Color {
    match fur {
        "ginger" => Color::srgb(0.9, 0.55, 0.2),
        "black" => Color::srgb(0.15, 0.15, 0.15),
        "white" => Color::srgb(0.95, 0.95, 0.92),
        "gray" => Color::srgb(0.5, 0.5, 0.52),
        "tabby brown" => Color::srgb(0.6, 0.4, 0.2),
        "calico" => Color::srgb(0.85, 0.6, 0.3),
        "tortoiseshell" => Color::srgb(0.55, 0.3, 0.15),
        "cream" => Color::srgb(0.95, 0.88, 0.7),
        "silver" => Color::srgb(0.75, 0.78, 0.8),
        "russet" => Color::srgb(0.7, 0.3, 0.15),
        _ => Color::srgb(0.7, 0.5, 0.3), // fallback brown
    }
}

/// Map each herb kind to a sprite index in the Mushrooms, Flowers, Stones atlas.
/// The atlas is a 12-col x 5-row grid of 16x16 sprites. Indices are row-major
/// (0 = top-left, 11 = top-right, 12 = second-row-left, etc.).
/// These picks are approximate — tune by running `just run` and eyeballing.
fn herb_sprite_index(kind: HerbKind) -> usize {
    match kind {
        HerbKind::HealingMoss => 0,       // mushroom (top-left)
        HerbKind::Moonpetal => 24,         // flower (row 2, col 0 area)
        HerbKind::Calmroot => 36,          // small green plant (row 3)
        HerbKind::Thornbriar => 6,         // darker mushroom/thorny (row 0, col 6)
        HerbKind::Dreamroot => 27,         // colorful flower (row 2, col 3)
    }
}

fn wildlife_color(animal: &WildAnimal) -> Color {
    use crate::components::wildlife::WildSpecies;
    match animal.species {
        WildSpecies::Fox => Color::srgb(0.85, 0.35, 0.1),
        WildSpecies::Hawk => Color::srgb(0.8, 0.75, 0.3),
        WildSpecies::Snake => Color::srgb(0.3, 0.7, 0.2),
        WildSpecies::ShadowFox => Color::srgb(0.6, 0.15, 0.7),
    }
}

fn prey_color(prey: &PreyAnimal) -> Color {
    use crate::components::prey::PreySpecies;
    match prey.species {
        PreySpecies::Mouse => Color::srgb(0.6, 0.5, 0.35),
        PreySpecies::Rat => Color::srgb(0.45, 0.45, 0.45),
        PreySpecies::Fish => Color::srgb(0.4, 0.6, 0.85),
        PreySpecies::Bird => Color::srgb(0.85, 0.85, 0.8),
    }
}
