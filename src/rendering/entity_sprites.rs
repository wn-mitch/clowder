use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Text2d;

use crate::components::building::{
    ConstructionSite, CropState, GateState, Structure, StructureType,
};
use crate::components::identity::{Appearance, Name, Species};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{
    FlavorKind, FlavorPlant, GrowthStage, Harvestable, Herb, HerbKind, Ward,
};
use crate::components::physical::{Dead, Position, PreviousPosition, RenderPosition};
use crate::components::prey::{PreyAnimal, PreyConfig, PreyDen, PreyKind};
use crate::components::wildlife::{FoxDen, WildAnimal};
use crate::rendering::sprite_assets::SpriteAssets;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;
use crate::resources::time::{Season, SimConfig, TimeState};

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
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        default(),
    );
    let handle = images.add(image);
    commands.insert_resource(WhitePixel(handle));
}

/// Give stored items a Position matching their container building so the
/// standard sprite-attach + position-sync pipeline can render them.
/// OnGround items already have Position from spawn.
pub fn sync_item_positions(
    mut commands: Commands,
    items_without_pos: Query<(Entity, &Item), Without<Position>>,
    buildings: Query<&Position, With<Structure>>,
) {
    for (entity, item) in &items_without_pos {
        if let ItemLocation::StoredIn(building) = item.location {
            if let Ok(building_pos) = buildings.get(building) {
                commands.entity(entity).insert(*building_pos);
            }
        }
    }
}

/// Visual layout slot for items rendered at a shared grid position.
/// Same-kind items stack vertically (up to 5); different kinds tile into columns.
#[derive(Component, Clone, Copy)]
pub struct ItemDisplaySlot {
    /// Which kind-group column this item belongs to (0, 1, 2...).
    pub kind_column: u8,
    /// Vertical position within the kind-group stack (0 = bottom, max 4).
    pub stack_row: u8,
    /// Total distinct kind-columns at this position, for centering.
    pub total_columns: u8,
}

/// Compute stacking/tiling layout for items sharing a grid position.
/// Runs each frame after `sync_item_positions` assigns positions to stored items.
pub fn compute_item_layout(mut commands: Commands, items: Query<(Entity, &Position, &Item)>) {
    // Group items by grid position.
    let mut by_pos: HashMap<(i32, i32), Vec<(Entity, ItemKind)>> = HashMap::new();
    for (entity, pos, item) in &items {
        by_pos
            .entry((pos.x, pos.y))
            .or_default()
            .push((entity, item.kind));
    }

    for (_pos, mut group) in by_pos {
        // Sort by ItemKind discriminant for stable column ordering, then by entity for
        // stable stack ordering within a kind group.
        group.sort_by(|a, b| {
            (a.1 as usize)
                .cmp(&(b.1 as usize))
                .then(a.0.to_bits().cmp(&b.0.to_bits()))
        });

        // Assign columns per distinct kind.
        let mut current_kind: Option<ItemKind> = None;
        let mut kind_column: u8 = 0;
        let mut stack_row: u8 = 0;
        let mut slots: Vec<(Entity, u8, u8)> = Vec::with_capacity(group.len());

        for (entity, kind) in &group {
            if current_kind != Some(*kind) {
                if current_kind.is_some() {
                    kind_column += 1;
                }
                current_kind = Some(*kind);
                stack_row = 0;
            }
            slots.push((*entity, kind_column, stack_row.min(4)));
            stack_row += 1;
        }

        let total_columns = kind_column + 1;
        for (entity, col, row) in slots {
            commands.entity(entity).insert(ItemDisplaySlot {
                kind_column: col,
                stack_row: row,
                total_columns,
            });
        }
    }
}

/// Attach sprites to entities that have Position but no EntitySpriteMarker.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn attach_entity_sprites(
    mut commands: Commands,
    white_pixel: Res<WhitePixel>,
    sprite_assets: Res<SpriteAssets>,
    map: Res<TileMap>,
    cats: Query<
        (Entity, &Position, &Appearance, &Name),
        (With<Species>, Without<EntitySpriteMarker>, Without<Dead>),
    >,
    dead_cats: Query<(Entity, &Position), (With<Species>, With<Dead>, Without<EntitySpriteMarker>)>,
    wildlife: Query<(Entity, &Position, &WildAnimal), Without<EntitySpriteMarker>>,
    prey: Query<(Entity, &Position, &PreyConfig), (With<PreyAnimal>, Without<EntitySpriteMarker>)>,
    dens: Query<(Entity, &Position, &PreyDen), Without<EntitySpriteMarker>>,
    fox_dens: Query<(Entity, &Position), (With<FoxDen>, Without<EntitySpriteMarker>)>,
    herbs: Query<(Entity, &Position, &Herb), (With<Harvestable>, Without<EntitySpriteMarker>)>,
    flavor_plants: Query<(Entity, &Position, &FlavorPlant), Without<EntitySpriteMarker>>,
    wards: Query<(Entity, &Position, &Ward), Without<EntitySpriteMarker>>,
    items: Query<(Entity, &Position, &Item), Without<EntitySpriteMarker>>,
    carcasses: Query<
        (Entity, &Position, &crate::components::wildlife::Carcass),
        Without<EntitySpriteMarker>,
    >,
    wells: Query<
        (Entity, &Position),
        (
            With<crate::components::building::ColonyWell>,
            Without<EntitySpriteMarker>,
        ),
    >,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;

    if !cats.is_empty() || !wildlife.is_empty() || !prey.is_empty() {
        eprintln!(
            "Attaching sprites: {} cats, {} dead, {} wildlife, {} prey, {} herbs, {} wards, {} fox dens",
            cats.iter().count(),
            dead_cats.iter().count(),
            wildlife.iter().count(),
            prey.iter().count(),
            herbs.iter().count(),
            wards.iter().count(),
            fox_dens.iter().count(),
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
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
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
            PreviousPosition { x: pos.x, y: pos.y },
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
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }

    // Wildlife — species-specific sprite from loaded spritesheets.
    for (entity, pos, animal) in &wildlife {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let label = commands
            .spawn((
                Text2d::new(animal.species.name()),
                TextFont {
                    font_size: 9.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
                Transform::from_xyz(0.0, world_px * 0.45, 1.0),
            ))
            .id();

        let (image, layout, size, frame_count) = wildlife_sprite(&sprite_assets, animal);
        let mut ecmds = commands.entity(entity);
        ecmds.insert((
            Sprite {
                image,
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(size)),
                texture_atlas: Some(layout),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 21.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
        if frame_count > 1 {
            ecmds.insert(crate::rendering::sprite_animation::AnimationTimer::new(
                frame_count,
                std::time::Duration::from_millis(300),
            ));
        }
        ecmds.add_children(&[label]);
    }

    // Prey — species-specific sprite from loaded spritesheets.
    for (entity, pos, config) in &prey {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let entity_hash = entity.to_bits();
        let (image, atlas, color, sprite_size, frame_count) =
            prey_sprite(&sprite_assets, config.kind, world_px, entity_hash);
        let label = commands
            .spawn((
                Text2d::new(config.name),
                TextFont {
                    font_size: 8.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
                Transform::from_xyz(0.0, sprite_size * 0.55 + 2.0, 1.0),
            ))
            .id();
        let mut ecmds = commands.entity(entity);
        ecmds.insert((
            Sprite {
                image,
                color,
                custom_size: Some(Vec2::splat(sprite_size)),
                texture_atlas: atlas,
                ..Default::default()
            },
            Transform::from_xyz(x, y, 18.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
        if frame_count > 1 {
            ecmds.insert(crate::rendering::sprite_animation::AnimationTimer::new(
                frame_count,
                std::time::Duration::from_millis(300),
            ));
        }
        ecmds.add_children(&[label]);
    }

    // Carcasses — dark desaturated prey colors, fading with age.
    for (entity, pos, carcass) in &carcasses {
        use crate::components::prey::PreyKind;
        let base_color = match carcass.prey_kind {
            PreyKind::Mouse => Color::srgba(0.3, 0.25, 0.15, 0.7),
            PreyKind::Rat => Color::srgba(0.25, 0.2, 0.15, 0.7),
            PreyKind::Rabbit => Color::srgba(0.35, 0.25, 0.12, 0.7),
            PreyKind::Fish => Color::srgba(0.2, 0.25, 0.3, 0.7),
            PreyKind::Bird => Color::srgba(0.3, 0.2, 0.25, 0.7),
        };
        let size = prey_sprite_size(carcass.prey_kind, world_px);
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let species_name = match carcass.prey_kind {
            PreyKind::Mouse => "mouse remains",
            PreyKind::Rat => "rat remains",
            PreyKind::Rabbit => "rabbit remains",
            PreyKind::Fish => "fish remains",
            PreyKind::Bird => "bird remains",
        };
        let label = commands
            .spawn((
                Text2d::new(species_name),
                TextFont {
                    font_size: 8.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
                Transform::from_xyz(0.0, size.y * 0.55 + 2.0, 1.0),
            ))
            .id();
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color: base_color,
                custom_size: Some(size),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 16.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
        commands.entity(entity).add_children(&[label]);
    }

    // Prey dens — hot colors, visible.
    for (entity, pos, den) in &dens {
        let color = den_color(den.kind);
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let label = commands
            .spawn((
                Text2d::new(den.den_name),
                TextFont {
                    font_size: 8.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
                Transform::from_xyz(0.0, world_px * 0.4, 1.0),
            ))
            .id();
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color,
                custom_size: Some(Vec2::splat(world_px * 0.6)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 16.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
        commands.entity(entity).add_children(&[label]);
    }

    // Fox dens — earthy brown marker with label.
    for (entity, pos) in &fox_dens {
        let color = Color::srgb(0.6, 0.25, 0.1);
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let label = commands
            .spawn((
                Text2d::new("Fox Den"),
                TextFont {
                    font_size: 8.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.0, 0.0, 0.0)),
                Transform::from_xyz(0.0, world_px * 0.4, 1.0),
            ))
            .id();
        commands.entity(entity).insert((
            Sprite {
                image: white_pixel.0.clone(),
                color,
                custom_size: Some(Vec2::splat(world_px * 0.5)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 16.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
        commands.entity(entity).add_children(&[label]);
    }

    // Herbs — distinct flower/mushroom sprite per kind and growth stage.
    for (entity, pos, herb) in &herbs {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let atlas_index = herb_sprite_index(herb.kind, herb.growth_stage);
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
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }

    // Flavor plants (non-harvestable) — same herbs atlas.
    for (entity, pos, plant) in &flavor_plants {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let atlas_index = flavor_sprite_index(plant.kind, plant.growth_stage);
        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.herbs_texture.clone(),
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(world_px * 0.5)),
                texture_atlas: Some(TextureAtlas {
                    layout: sprite_assets.herbs_layout.clone(),
                    index: atlas_index,
                }),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 16.5),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }

    // Wards — lantern sprite (Lantern_2.png, 16x32) + translucent AOE aura
    // showing the effective repulsion zone. Aura color encodes ward kind,
    // alpha scales with strength so fading wards visibly dim before despawn.
    for (entity, pos, ward) in &wards {
        let sprite_color = if ward.inverted {
            Color::srgb(1.0, 0.3, 0.3)
        } else {
            Color::WHITE
        };
        let w = world_px * 0.5;
        let h = w / 16.0 * 32.0; // preserve 16:32 aspect ratio
        let (x, y) = grid_to_world(pos, map_h, world_px);
        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.ward_texture.clone(),
                color: sprite_color,
                custom_size: Some(Vec2::new(w, h)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 22.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));

        let aura_rgb = if ward.inverted {
            (0.9, 0.2, 0.2)
        } else {
            match ward.kind {
                crate::components::magic::WardKind::Thornward => (0.4, 0.9, 0.5),
                crate::components::magic::WardKind::DurableWard => (0.4, 0.6, 1.0),
            }
        };
        let aura_alpha = 0.18 * ward.strength.clamp(0.0, 1.0);
        // Aura diameter = 2 * repel_radius tiles (Manhattan-scaled); render as a
        // square because the repel logic uses manhattan distance, so the tinted
        // square truthfully represents the actual coverage footprint.
        let diameter = 2.0 * ward.repel_radius() * world_px;
        let aura = commands
            .spawn((
                Sprite {
                    image: sprite_assets.white_pixel.clone(),
                    color: Color::srgba(aura_rgb.0, aura_rgb.1, aura_rgb.2, aura_alpha),
                    custom_size: Some(Vec2::new(diameter, diameter)),
                    ..Default::default()
                },
                // Child transform is relative to parent ward.
                Transform::from_xyz(0.0, 0.0, -20.5),
            ))
            .id();
        commands.entity(entity).add_children(&[aura]);
    }

    // Items — 16x16 sprites from the items spritesheet.
    for (entity, pos, item) in &items {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let atlas_index = item_sprite_index(item.kind);
        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.items_texture.clone(),
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(world_px * 0.4)),
                texture_atlas: Some(TextureAtlas {
                    layout: sprite_assets.items_layout.clone(),
                    index: atlas_index,
                }),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 15.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }

    // Colony well — Fan-tasy Tileset hay well (56x74 source, ~1.2 tiles wide).
    for (entity, pos) in &wells {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let w = 1.2 * world_px;
        let h = w / 56.0 * 74.0;
        commands.entity(entity).insert((
            Sprite {
                image: sprite_assets.well_texture.clone(),
                color: Color::WHITE,
                custom_size: Some(Vec2::new(w, h)),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 14.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }
}

/// Attach sprites to building entities (Structure) that lack EntitySpriteMarker.
///
/// Separated from `attach_entity_sprites` to stay under Bevy's 16-param limit.
/// ConstructionSite buildings render semi-transparent; completed buildings are
/// full opacity.
#[allow(clippy::type_complexity)]
pub fn attach_building_sprites(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    map: Res<TileMap>,
    structures: Query<
        (Entity, &Position, &Structure, Option<&ConstructionSite>),
        (
            Without<EntitySpriteMarker>,
            Without<crate::components::building::ColonyWell>,
        ),
    >,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;

    for (entity, pos, structure, construction) in &structures {
        let (image, size) = building_sprite(&sprite_assets, structure.kind, entity.to_bits());
        let alpha = if construction.is_some() { 0.4 } else { 1.0 };
        let (x, y) = grid_to_world(pos, map_h, world_px);

        commands.entity(entity).insert((
            Sprite {
                image,
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(size),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 13.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }
}

/// Select the texture handle and render size for a building type.
fn building_sprite(
    assets: &SpriteAssets,
    kind: StructureType,
    entity_hash: u64,
) -> (Handle<Image>, Vec2) {
    let world_px = TILE_PX * TILE_SCALE;
    match kind {
        // Den: 3 variants, 89x91 source, 2 tiles wide
        StructureType::Den => {
            let variant = (entity_hash as usize) % assets.den_textures.len();
            let w = 2.0 * world_px;
            let h = w / 89.0 * 91.0;
            (assets.den_textures[variant].clone(), Vec2::new(w, h))
        }
        // Hearth: 128x128 source, 2 tiles wide
        StructureType::Hearth => {
            let w = 2.0 * world_px;
            (assets.hearth_texture.clone(), Vec2::splat(w))
        }
        // Kitchen: reuses the workshop sprite until a dedicated asset exists.
        StructureType::Kitchen => {
            let w = world_px;
            let h = w / 35.0 * 44.0;
            (assets.workshop_texture.clone(), Vec2::new(w, h))
        }
        // Stores: 62x57 source, 2 tiles wide
        StructureType::Stores => {
            let w = 2.0 * world_px;
            let h = w / 62.0 * 57.0;
            (assets.stores_texture.clone(), Vec2::new(w, h))
        }
        // Workshop: 35x44 source, 1 tile wide
        StructureType::Workshop => {
            let w = world_px;
            let h = w / 35.0 * 44.0;
            (assets.workshop_texture.clone(), Vec2::new(w, h))
        }
        // Garden: 32x32 source, 0.7 tiles wide (basket prop)
        StructureType::Garden => {
            let w = 0.7 * world_px;
            (assets.garden_textures[0].clone(), Vec2::splat(w))
        }
        // Watchtower: 68x149 source, 1.5 tiles wide
        StructureType::Watchtower => {
            let w = 1.5 * world_px;
            let h = w / 68.0 * 149.0;
            (assets.watchtower_texture.clone(), Vec2::new(w, h))
        }
        // WardPost: 24x59 source, 0.5 tiles wide
        StructureType::WardPost => {
            let w = 0.5 * world_px;
            let h = w / 24.0 * 59.0;
            (assets.wardpost_texture.clone(), Vec2::new(w, h))
        }
        // Wall: 16x48 source, 1 tile wide
        StructureType::Wall => {
            let w = world_px;
            let h = w / 16.0 * 48.0;
            (assets.wall_texture.clone(), Vec2::new(w, h))
        }
        // Gate: 80x96 source, 2 tiles wide
        StructureType::Gate => {
            let w = 2.0 * world_px;
            let h = w / 80.0 * 96.0;
            (assets.gate_texture.clone(), Vec2::new(w, h))
        }
        // 176: Midden visually reuses the Stores sprite (refuse pile
        // looks "container-shaped"). A future visual-polish ticket
        // can swap in a dedicated midden asset.
        StructureType::Midden => {
            let w = 2.0 * world_px;
            let h = w / 62.0 * 57.0;
            (assets.stores_texture.clone(), Vec2::new(w, h))
        }
    }
}

/// Update gate sprites when GateState changes: open gates fade to low alpha.
#[allow(clippy::type_complexity)]
pub fn update_gate_sprites(
    mut gates: Query<(&GateState, &mut Sprite), (With<Structure>, Changed<GateState>)>,
) {
    for (gate, mut sprite) in &mut gates {
        sprite.color = if gate.open {
            Color::srgba(1.0, 1.0, 1.0, 0.3)
        } else {
            Color::WHITE
        };
    }
}

/// Update garden sprites when CropState changes: swap basket texture by growth stage.
#[allow(clippy::type_complexity)]
pub fn update_crop_sprites(
    sprite_assets: Res<SpriteAssets>,
    mut gardens: Query<(&CropState, &mut Sprite), (With<Structure>, Changed<CropState>)>,
) {
    for (crop, mut sprite) in &mut gardens {
        let idx = if crop.growth < 0.3 {
            0 // Basket_Empty — bare soil
        } else if crop.growth < 0.7 {
            1 // Basket_Cotton — growing
        } else {
            2 // Basket_Vegetables — harvestable
        };
        sprite.image = sprite_assets.garden_textures[idx].clone();
    }
}

/// Swap building sprites between Seasons and Snow variants when winter starts/ends.
pub fn swap_seasonal_building_sprites(
    sprite_assets: Res<SpriteAssets>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut last_season: Local<Option<Season>>,
    mut buildings: Query<(Entity, &Structure, &mut Sprite), With<EntitySpriteMarker>>,
) {
    let current = time.season(&config);
    let prev = *last_season;
    *last_season = Some(current);

    let Some(prev) = prev else { return };
    if prev == current {
        return;
    }

    let entering_winter = current == Season::Winter && prev != Season::Winter;
    let leaving_winter = current != Season::Winter && prev == Season::Winter;
    if !entering_winter && !leaving_winter {
        return;
    }

    for (entity, structure, mut sprite) in &mut buildings {
        let hash = entity.to_bits();
        let (image, _) = if entering_winter {
            building_sprite_snow(&sprite_assets, structure.kind, hash)
        } else {
            building_sprite(&sprite_assets, structure.kind, hash)
        };
        sprite.image = image;
    }
}

/// Select the snow variant texture for a building type (winter).
fn building_sprite_snow(
    assets: &SpriteAssets,
    kind: StructureType,
    entity_hash: u64,
) -> (Handle<Image>, Vec2) {
    let world_px = TILE_PX * TILE_SCALE;
    match kind {
        StructureType::Den => {
            let variant = (entity_hash as usize) % assets.den_snow_textures.len();
            let w = 2.0 * world_px;
            let h = w / 89.0 * 91.0;
            (assets.den_snow_textures[variant].clone(), Vec2::new(w, h))
        }
        StructureType::Hearth => {
            let w = 2.0 * world_px;
            (assets.hearth_snow_texture.clone(), Vec2::splat(w))
        }
        StructureType::Stores => {
            let w = 2.0 * world_px;
            let h = w / 62.0 * 57.0;
            (assets.stores_snow_texture.clone(), Vec2::new(w, h))
        }
        StructureType::Watchtower => {
            let w = 1.5 * world_px;
            let h = w / 68.0 * 149.0;
            (assets.watchtower_snow_texture.clone(), Vec2::new(w, h))
        }
        StructureType::WardPost => {
            let w = 0.5 * world_px;
            let h = w / 24.0 * 59.0;
            (assets.wardpost_snow_texture.clone(), Vec2::new(w, h))
        }
        // Workshop, Garden, Wall, Gate have no snow variants — keep as-is.
        _ => building_sprite(assets, kind, entity_hash),
    }
}

/// Snapshot current Position into PreviousPosition before the simulation tick
/// advances positions. Runs in FixedUpdate before all simulation systems.
pub fn snapshot_previous_positions(mut query: Query<(&Position, &mut PreviousPosition)>) {
    for (pos, mut prev) in &mut query {
        prev.x = pos.x;
        prev.y = pos.y;
    }
}

/// Ticket 129 — refresh `RenderTickProgress` from
/// `Time<Fixed>::overstep_fraction()` once per render frame so every
/// downstream interpolation system reads the same `[0, 1]` parameter
/// without re-pulling `fixed_time` itself. Must run before
/// [`sync_entity_positions`] in the rendering schedule.
pub fn update_render_tick_progress(
    fixed_time: Res<Time<Fixed>>,
    mut progress: ResMut<crate::resources::RenderTickProgress>,
) {
    progress.0 = fixed_time.overstep_fraction().clamp(0.0, 1.0);
}

/// Ticket 129 — backfill `RenderPosition` on any entity that already
/// has `Position` + `PreviousPosition` + sprite marker but is missing
/// the new component (existing spawn sites manually inserted
/// `PreviousPosition` only). Runs in `Update` before
/// `sync_entity_positions` so the interpolation always has a target
/// component to write into.
#[allow(clippy::type_complexity)]
pub fn backfill_render_position(
    mut commands: Commands,
    query: Query<
        Entity,
        (
            With<Position>,
            With<PreviousPosition>,
            With<EntitySpriteMarker>,
            Without<RenderPosition>,
        ),
    >,
) {
    for entity in &query {
        commands.entity(entity).insert(RenderPosition::default());
    }
}

/// Smoothstep ease-in/out — Hermite `3t² − 2t³`, clamped to `[0, 1]`.
/// Inline so the optimizer can fold it into the call site.
#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Sync Position → RenderPosition → Transform for all entities. The
/// per-frame interpolation reads `RenderTickProgress`, applies a
/// smoothstep ease-in/out (ticket 129's curve choice — linear was
/// the pre-129 default), writes the result to `RenderPosition`, and
/// then composes per-entity layout offsets (item-stack columns,
/// non-item hash-deterministic sub-tile jitter) into
/// `Transform.translation`. Tile texture index and z-layer reads
/// elsewhere still use `Position` (containing tile).
#[allow(clippy::type_complexity)]
pub fn sync_entity_positions(
    map: Res<TileMap>,
    progress: Res<crate::resources::RenderTickProgress>,
    mut query: Query<
        (
            Entity,
            &Position,
            &PreviousPosition,
            &mut RenderPosition,
            &mut Transform,
            Option<&ItemDisplaySlot>,
        ),
        With<EntitySpriteMarker>,
    >,
) {
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;
    let smoothed = smoothstep(progress.0);

    for (entity, pos, prev, mut render_pos, mut transform, display_slot) in &mut query {
        let (curr_x, curr_y) = grid_to_world(pos, map_h, world_px);

        // Snap directly for large jumps (spawn, teleport) — skip
        // interpolation. Threshold of 5 grid cells matches pre-129
        // behavior; sub-tile interpolation only makes sense for
        // tick-by-tick step movement.
        let dist = (pos.x - prev.x).unsigned_abs() + (pos.y - prev.y).unsigned_abs();
        let (x, y) = if dist > 5 {
            (curr_x, curr_y)
        } else {
            let prev_x = prev.x as f32 * world_px;
            let prev_y = (map_h - 1.0 - prev.y as f32) * world_px;
            (
                prev_x + (curr_x - prev_x) * smoothed,
                prev_y + (curr_y - prev_y) * smoothed,
            )
        };

        // Tile-center smooth position (no per-entity offsets) — the
        // public render-substrate value. Phase 2 (#131) reads this
        // unchanged when `Position` itself becomes `Vec2<f32>`.
        render_pos.0 = bevy::math::Vec2::new(x, y);

        if let Some(slot) = display_slot {
            // Structured layout for items: columns per kind, stacks per item.
            let col_spacing = world_px * 0.35;
            let row_step = world_px * 0.12;
            let centering = slot.total_columns as f32 * col_spacing * 0.5;
            transform.translation.x =
                x - centering + slot.kind_column as f32 * col_spacing + col_spacing * 0.5;
            transform.translation.y = y + slot.stack_row as f32 * row_step;
            // Upper items render on top.
            transform.translation.z = 15.0 + slot.stack_row as f32 * 0.01;
        } else {
            // Non-item entities: small deterministic sub-tile offset so sprites
            // on the same tile don't stack exactly and name labels stay readable.
            let hash = entity.to_bits() as f32;
            let offset_x = (hash * 7.3).sin() * 0.3 * world_px;
            let offset_y = (hash * 13.7).sin() * 0.15 * world_px;
            transform.translation.x = x + offset_x;
            transform.translation.y = y + offset_y;
        }
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

/// Map each herb kind + growth stage to a sprite index in the Mushrooms, Flowers, Stones atlas.
/// Atlas: 12 cols × 5 rows, 16×16, row-major (index 0 = top-left).
fn herb_sprite_index(kind: HerbKind, stage: GrowthStage) -> usize {
    use GrowthStage::*;
    match kind {
        // Row 0: mushroom cluster (3 sprites, no Bud distinct from Bloom)
        HerbKind::HealingMoss => match stage {
            Sprout => 0,
            Bud | Bloom => 1,
            Blossom => 2,
        },
        // Row 0, cols 3–6: thornbriar stages
        HerbKind::Thornbriar => match stage {
            Sprout => 3,
            Bud => 4,
            Bloom => 5,
            Blossom => 6,
        },
        // Row 4: moonpetal bud (58) / bloom (59) — previously wrong at index 24
        HerbKind::Moonpetal => match stage {
            Sprout | Bud | Bloom => 58,
            Blossom => 59,
        },
        // Row 4: calmroot bloom (56) / blossom (57) — previously wrong at index 36
        HerbKind::Calmroot => match stage {
            Sprout | Bud | Bloom => 56,
            Blossom => 57,
        },
        // Row 4: dreamroot full 4-stage progression — previously wrong at index 27
        HerbKind::Dreamroot => match stage {
            Sprout => 52,
            Bud => 53,
            Bloom => 54,
            Blossom => 55,
        },
        // Row 2, cols 0–3: catnip sprout → bush
        HerbKind::Catnip => match stage {
            Sprout => 24,
            Bud => 25,
            Bloom => 26,
            Blossom => 27,
        },
        // Row 3, cols 4–7: slumbershade
        HerbKind::Slumbershade => match stage {
            Sprout => 40,
            Bud => 41,
            Bloom => 42,
            Blossom => 43,
        },
        // Row 4, cols 0–2: oracle orchid (3 sprites, skip Bud)
        HerbKind::OracleOrchid => match stage {
            Sprout | Bud => 48,
            Bloom => 49,
            Blossom => 50,
        },
    }
}

/// Map each flavor plant kind + growth stage to a sprite index in the Mushrooms, Flowers, Stones atlas.
fn flavor_sprite_index(kind: FlavorKind, stage: GrowthStage) -> usize {
    use GrowthStage::*;
    match kind {
        // Row 3, cols 0–3: sunflower shoot → blossom top
        FlavorKind::Sunflower => match stage {
            Sprout => 36,
            Bud => 37,
            Bloom => 38,
            Blossom => 39,
        },
        // Row 3, col 8: rose (single sprite, all stages same)
        FlavorKind::Rose => 44,
        // Rocks — static sprites, stage irrelevant
        FlavorKind::Pebble => 12,
        FlavorKind::Rock => 13,
        FlavorKind::Stone => 14,
        FlavorKind::StoneChunk => 15,
        FlavorKind::StoneFlat => 16,
        FlavorKind::Boulder => 17,
    }
}

/// Map each item kind to a sprite index in the items spritesheet atlas.
/// The atlas is an 8-col x 15-row grid of 16x16 sprites, row-major.
/// These picks are approximate — tune by running `just run` and eyeballing.
fn item_sprite_index(kind: ItemKind) -> usize {
    match kind {
        // Raw prey — row 0 food sprites
        ItemKind::RawMouse => 0,  // drumstick
        ItemKind::RawRat => 1,    // meat cut
        ItemKind::RawRabbit => 1, // meat cut
        ItemKind::RawFish => 66,  // PEAR (closest fish shape in this pack)
        ItemKind::RawBird => 0,   // drumstick (poultry)
        // Foraged — actual produce sprites from the catalog
        ItemKind::Berries => 82,    // STRAWBERRY
        ItemKind::Nuts => 50,       // APPLE (round food, nut stand-in)
        ItemKind::Roots => 81,      // TURNIP
        ItemKind::WildOnion => 97,  // RADISH (bulb vegetable)
        ItemKind::Mushroom => 7,    // mushroom
        ItemKind::Moss => 5,        // leaf
        ItemKind::DriedGrass => 11, // GRASS
        ItemKind::Feather => 5,     // leaf shape
        // Herbs as bottled items — bottles at cols 4-7: plain, special, large, large-special.
        ItemKind::HerbHealingMoss => 20, // WHITEBOTTLE  (row 3, col 4)
        ItemKind::HerbMoonpetal => 68,   // PINKBOTTLE   (row 9, col 4)
        ItemKind::HerbCalmroot => 84,    // GREENBOTTLE  (row 11, col 4)
        ItemKind::HerbThornbriar => 36,  // BROWNBOTTLE  (row 5, col 4)
        ItemKind::HerbDreamroot => 52,   // PURPLEBOTTLE (row 7, col 4)
        ItemKind::HerbCatnip => 69,      // PINKBOTTLESPECIAL   (row 9, col 5)
        ItemKind::HerbSlumbershade => 37, // BROWNBOTTLESPECIAL  (row 5, col 5)
        ItemKind::HerbOracleOrchid => 53, // PURPLEBOTTLESPECIAL (row 7, col 5)
        // Curiosities
        ItemKind::ShinyPebble => 42,   // STONE
        ItemKind::GlassShard => 43,    // ROCK
        ItemKind::ColorfulShell => 93, // PINKEGG (colorful rounded shape)
        // Shadow materials
        ItemKind::ShadowBone => 43, // ROCK (dark bone-like)
        // Storage upgrades
        ItemKind::Barrel => 30, // LARGEBROWNJAR
        ItemKind::Crate => 35,  // STONEBRICK
        ItemKind::Shelf => 34,  // PLANK
        // Build materials
        ItemKind::Wood => 34,  // PLANK
        ItemKind::Stone => 35, // STONEBRICK
    }
}

/// Select the sprite texture, atlas, render size, and animation frame count for a wildlife species.
fn wildlife_sprite(
    assets: &SpriteAssets,
    animal: &WildAnimal,
) -> (Handle<Image>, TextureAtlas, f32, u8) {
    use crate::components::wildlife::WildSpecies;
    let world_px = TILE_PX * TILE_SCALE;
    match animal.species {
        WildSpecies::Fox => (
            assets.fox_texture.clone(),
            TextureAtlas {
                layout: assets.fox_layout.clone(),
                index: 0,
            },
            world_px * 0.8,
            1, // Minifolks directional, not a simple animation strip
        ),
        WildSpecies::Hawk => (
            assets.hawk_texture.clone(),
            TextureAtlas {
                layout: assets.hawk_layout.clone(),
                index: 0,
            },
            world_px * 0.65,
            4,
        ),
        WildSpecies::Snake => (
            assets.snake_texture.clone(),
            TextureAtlas {
                layout: assets.snake_layout.clone(),
                index: 0,
            },
            world_px * 0.6,
            1, // Directional spritesheet, not a simple strip
        ),
        WildSpecies::ShadowFox => (
            assets.shadow_fox_texture.clone(),
            TextureAtlas {
                layout: assets.shadow_fox_layout.clone(),
                index: 0,
            },
            world_px * 0.85,
            4,
        ),
    }
}

/// Select the sprite texture, optional atlas, tint color, render size, and animation
/// frame count for a prey kind. `entity_hash` provides deterministic per-entity
/// variant selection for species with multiple sprite options.
fn prey_sprite(
    assets: &SpriteAssets,
    kind: PreyKind,
    world_px: f32,
    entity_hash: u64,
) -> (Handle<Image>, Option<TextureAtlas>, Color, f32, u8) {
    match kind {
        PreyKind::Mouse => (
            assets.rat_texture.clone(),
            Some(TextureAtlas {
                layout: assets.rat_layout.clone(),
                index: 0,
            }),
            Color::srgb(0.85, 0.75, 0.6), // lighter brown tint
            world_px * 0.5,
            1, // directional spritesheet, not simple strip
        ),
        PreyKind::Rat => (
            assets.rat_texture.clone(),
            Some(TextureAtlas {
                layout: assets.rat_layout.clone(),
                index: 0,
            }),
            Color::WHITE,
            world_px * 0.55,
            1,
        ),
        PreyKind::Rabbit => (
            assets.rabbit_texture.clone(),
            Some(TextureAtlas {
                layout: assets.rabbit_layout.clone(),
                index: 0,
            }),
            Color::WHITE,
            world_px * 0.6,
            1,
        ),
        PreyKind::Fish => {
            let variant = (entity_hash as usize) % assets.fish_textures.len();
            (
                assets.fish_textures[variant].clone(),
                None,
                Color::WHITE,
                world_px * 0.5,
                1,
            )
        }
        PreyKind::Bird => {
            let variant = (entity_hash as usize) % assets.bird_textures.len();
            (
                assets.bird_textures[variant].clone(),
                Some(TextureAtlas {
                    layout: assets.bird_anim_layout.clone(),
                    index: 0,
                }),
                Color::WHITE,
                world_px * 0.5,
                4,
            )
        }
    }
}

fn den_color(kind: PreyKind) -> Color {
    match kind {
        PreyKind::Mouse => Color::srgb(1.0, 0.3, 0.1), // hot orange
        PreyKind::Rat => Color::srgb(0.9, 0.1, 0.1),   // red
        PreyKind::Rabbit => Color::srgb(1.0, 0.5, 0.0), // amber
        PreyKind::Fish => Color::srgb(0.9, 0.2, 0.6),  // hot pink
        PreyKind::Bird => Color::srgb(1.0, 0.8, 0.0),  // yellow
    }
}

fn prey_sprite_size(kind: PreyKind, world_px: f32) -> Vec2 {
    match kind {
        PreyKind::Mouse => Vec2::splat(world_px * 0.4),
        PreyKind::Rat => Vec2::splat(world_px * 0.45),
        PreyKind::Rabbit => Vec2::splat(world_px * 0.5),
        PreyKind::Fish => Vec2::splat(world_px * 0.4),
        PreyKind::Bird => Vec2::splat(world_px * 0.4),
    }
}
