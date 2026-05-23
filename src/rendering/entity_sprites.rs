use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Text2d;

use crate::components::building::{ConstructionSite, CropState, GateState, Structure};
use crate::components::identity::{Appearance, Name, Species};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{FlavorPlant, Harvestable, Herb, Ward};
use crate::components::physical::{Dead, Position, PreviousPosition, RenderPosition};
use crate::components::prey::{PreyAnimal, PreyConfig, PreyDen, PreyKind};
use crate::components::wildlife::{FoxDen, WildAnimal};
use crate::rendering::sprite_assets::SpriteAssets;
use crate::rendering::sprite_bindings::SpriteBindings;
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

/// Rendering data sources bundled to keep `attach_entity_sprites` under
/// the Bevy 16-param tuple limit (CLAUDE.md ECS rules). Add new sprite
/// data resources here rather than as siblings of this SystemParam.
#[derive(SystemParam)]
pub struct RenderingData<'w> {
    pub white_pixel: Res<'w, WhitePixel>,
    pub sprite_assets: Res<'w, SpriteAssets>,
    pub bindings: Res<'w, SpriteBindings>,
}

/// Attach sprites to entities that have Position but no EntitySpriteMarker.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn attach_entity_sprites(
    mut commands: Commands,
    rendering: RenderingData,
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
                image: rendering.sprite_assets.character_texture.clone(),
                color,
                custom_size: Some(Vec2::splat(world_px)),
                texture_atlas: Some(TextureAtlas {
                    layout: rendering.sprite_assets.character_layout.clone(),
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
                image: rendering.white_pixel.0.clone(),
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

        let (image, layout, size, frame_count) = wildlife_sprite(&rendering.sprite_assets, animal);
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
            prey_sprite(&rendering.sprite_assets, config.kind, world_px, entity_hash);
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
                image: rendering.white_pixel.0.clone(),
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
                image: rendering.white_pixel.0.clone(),
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
                image: rendering.white_pixel.0.clone(),
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

    // Herbs — bound to whichever atlas the manifest declares for each
    // species. `herb_sprite()` resolves to (texture, layout, index).
    for (entity, pos, herb) in &herbs {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let s = rendering.bindings.herb_sprite(herb.kind, herb.growth_stage);
        let color = if herb.twisted {
            Color::srgb(0.6, 0.15, 0.4) // corrupted: dark magenta tint
        } else {
            Color::WHITE
        };
        commands.entity(entity).insert((
            Sprite {
                image: s.texture,
                color,
                custom_size: Some(Vec2::splat(world_px * 0.5)),
                texture_atlas: Some(TextureAtlas {
                    layout: s.layout,
                    index: s.index,
                }),
                ..Default::default()
            },
            Transform::from_xyz(x, y, 17.0),
            PreviousPosition { x: pos.x, y: pos.y },
            EntitySpriteMarker,
        ));
    }

    // Flavor plants (non-harvestable).
    for (entity, pos, plant) in &flavor_plants {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let s = rendering
            .bindings
            .flavor_sprite(plant.kind, plant.growth_stage);
        commands.entity(entity).insert((
            Sprite {
                image: s.texture,
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(world_px * 0.5)),
                texture_atlas: Some(TextureAtlas {
                    layout: s.layout,
                    index: s.index,
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
                image: rendering.sprite_assets.ward_texture.clone(),
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
                    image: rendering.sprite_assets.white_pixel.clone(),
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

    // Items — bound to whichever atlas the manifest declares per item,
    // OR to a single-file texture path (Fan-tasy props). The renderer
    // branches on the binding form: atlas items carry a `TextureAtlas`
    // component, texture items render the full PNG with no atlas.
    for (entity, pos, item) in &items {
        let (x, y) = grid_to_world(pos, map_h, world_px);
        let (image, atlas) = match rendering.bindings.item_sprite(item.kind) {
            crate::rendering::sprite_bindings::ItemSprite::Atlas(s) => (
                s.texture,
                Some(TextureAtlas {
                    layout: s.layout,
                    index: s.index,
                }),
            ),
            crate::rendering::sprite_bindings::ItemSprite::Texture(handle) => (handle, None),
        };
        commands.entity(entity).insert((
            Sprite {
                image,
                color: Color::WHITE,
                custom_size: Some(Vec2::splat(world_px * 0.4)),
                texture_atlas: atlas,
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
                image: rendering.sprite_assets.well_texture.clone(),
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
    bindings: Res<SpriteBindings>,
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
        let (image, size) = bindings.building_sprite(structure.kind, entity.to_bits(), world_px);
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
    bindings: Res<SpriteBindings>,
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

    let world_px = TILE_PX * TILE_SCALE;
    for (entity, structure, mut sprite) in &mut buildings {
        let hash = entity.to_bits();
        let (image, _) = if entering_winter {
            bindings.building_sprite_winter(structure.kind, hash, world_px)
        } else {
            bindings.building_sprite(structure.kind, hash, world_px)
        };
        sprite.image = image;
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
