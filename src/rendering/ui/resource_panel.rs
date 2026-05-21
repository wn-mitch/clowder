use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::components::items::{Item, ItemCategory};
use crate::components::prey::{PreyAnimal, PreyConfig, PreyKind};
use crate::rendering::ui::{UiRoot, PANEL_BG, PANEL_BORDER, TEXT_COLOR, TEXT_DIM, TEXT_HIGHLIGHT};
use crate::resources::food::FoodStores;

#[derive(Component)]
pub struct ResourcePanel;

#[derive(Component)]
pub struct ResourcePanelContent;

const FONT_SIZE: f32 = 11.0;
const HEADER_FONT_SIZE: f32 = 13.0;
const BAR_WIDTH: f32 = 100.0;
const BAR_HEIGHT: f32 = 10.0;
const BAR_GREEN: Color = Color::srgb(0.3, 0.8, 0.3);
const BAR_YELLOW: Color = Color::srgb(0.9, 0.75, 0.2);
const BAR_RED: Color = Color::srgb(0.9, 0.25, 0.2);
const BAR_BG: Color = Color::srgba(0.2, 0.2, 0.2, 0.6);

pub fn setup_resource_panel(mut commands: Commands, root_query: Query<Entity, With<UiRoot>>) {
    let Ok(root) = root_query.single() else {
        return;
    };

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                bottom: Val::Px(36.0),
                width: Val::Px(300.0),
                max_height: Val::Percent(25.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::from(PANEL_BORDER),
            ResourcePanel,
        ))
        .id();

    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            ResourcePanelContent,
        ))
        .id();

    commands.entity(panel).add_children(&[content]);
    commands.entity(root).add_children(&[panel]);
}

pub fn update_resource_panel(
    mut commands: Commands,
    panel_vis: Res<crate::rendering::ui::PanelVisibility>,
    mut panel_query: Query<&mut Visibility, With<ResourcePanel>>,
    content_query: Query<Entity, With<ResourcePanelContent>>,
    food: Res<FoodStores>,
    items: Query<&Item>,
    prey: Query<&PreyConfig, With<PreyAnimal>>,
) {
    for mut vis in &mut panel_query {
        *vis = if panel_vis.resource_panel {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if !panel_vis.resource_panel {
        return;
    }

    let Ok(content_entity) = content_query.single() else {
        return;
    };

    // Rebuild every frame (cheap — just counting).
    commands
        .entity(content_entity)
        .despawn_related::<Children>();

    let mut children: Vec<Entity> = Vec::new();

    // --- Header ---
    children.push(spawn_text(
        &mut commands,
        "Resources",
        HEADER_FONT_SIZE,
        TEXT_HIGHLIGHT,
    ));

    // --- Food stores bar (Stores fill — the canonical chronic-full signal) ---
    children.push(spawn_food_bar(&mut commands, food.current, food.capacity));

    // --- Food breakdown by source (190) ---
    // Honest labels: in_stores/held are GOAP-accessible today; in_dens/in_workshops
    // are tracked-but-dark until the Den/Workshop retrieval substrate lands.
    let breakdown = format!(
        "  Total {} food  ({} stores · {} dens · {} workshops · {} held)",
        food.total_accessible(),
        food.in_stores,
        food.in_dens,
        food.in_workshops,
        food.held,
    );
    children.push(spawn_text(&mut commands, &breakdown, FONT_SIZE, TEXT_DIM));

    // --- Item counts by category (enum-driven; hides zero-count categories) ---
    let counts = count_items_by_category(&items);
    children.push(spawn_spacer(&mut commands));
    children.push(spawn_text(
        &mut commands,
        "Stockpile",
        FONT_SIZE + 1.0,
        TEXT_COLOR,
    ));
    if counts.is_empty() {
        children.push(spawn_text(&mut commands, "  (empty)", FONT_SIZE, TEXT_DIM));
    } else {
        for (category, count) in counts {
            children.push(spawn_text(
                &mut commands,
                &format!("  {}: {}", category.label(), count),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
    }

    // --- Prey populations ---
    let (mice, rats, rabbits, fish, birds) = count_prey(&prey);
    let total_prey = mice + rats + rabbits + fish + birds;
    if total_prey > 0 {
        children.push(spawn_spacer(&mut commands));
        children.push(spawn_text(
            &mut commands,
            "Wildlife",
            FONT_SIZE + 1.0,
            TEXT_COLOR,
        ));
        if mice > 0 {
            children.push(spawn_text(
                &mut commands,
                &format!("  Mice: {mice}"),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
        if rats > 0 {
            children.push(spawn_text(
                &mut commands,
                &format!("  Rats: {rats}"),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
        if rabbits > 0 {
            children.push(spawn_text(
                &mut commands,
                &format!("  Rabbits: {rabbits}"),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
        if fish > 0 {
            children.push(spawn_text(
                &mut commands,
                &format!("  Fish: {fish}"),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
        if birds > 0 {
            children.push(spawn_text(
                &mut commands,
                &format!("  Birds: {birds}"),
                FONT_SIZE,
                TEXT_DIM,
            ));
        }
    }

    commands.entity(content_entity).add_children(&children);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bucket every world `Item` by `ItemCategory`, returning categories in
/// stable display order. Empty categories are omitted so the panel only
/// renders rows for items the colony actually has.
fn count_items_by_category(items: &Query<&Item>) -> Vec<(ItemCategory, u32)> {
    let mut by_category: BTreeMap<u8, (ItemCategory, u32)> = BTreeMap::new();
    for item in items.iter() {
        let cat = item.kind.category();
        let entry = by_category.entry(cat.sort_key()).or_insert((cat, 0));
        entry.1 += 1;
    }
    by_category.into_values().collect()
}

fn count_prey(prey: &Query<&PreyConfig, With<PreyAnimal>>) -> (usize, usize, usize, usize, usize) {
    let (mut mice, mut rats, mut rabbits, mut fish, mut birds) = (0, 0, 0, 0, 0);
    for p in prey.iter() {
        match p.kind {
            PreyKind::Mouse => mice += 1,
            PreyKind::Rat => rats += 1,
            PreyKind::Rabbit => rabbits += 1,
            PreyKind::Fish => fish += 1,
            PreyKind::Bird => birds += 1,
        }
    }
    (mice, rats, rabbits, fish, birds)
}

fn spawn_text(commands: &mut Commands, content: &str, size: f32, color: Color) -> Entity {
    commands
        .spawn((
            Node {
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            Text::new(content.to_string()),
            TextFont {
                font_size: size,
                ..default()
            },
            TextColor(color),
        ))
        .id()
}

fn spawn_spacer(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(4.0),
            ..default()
        })
        .id()
}

fn bar_color(fraction: f32) -> Color {
    if fraction < 0.2 {
        BAR_RED
    } else if fraction < 0.5 {
        BAR_YELLOW
    } else {
        BAR_GREEN
    }
}

fn spawn_food_bar(commands: &mut Commands, current: f32, capacity: f32) -> Entity {
    let fraction = if capacity > 0.0 {
        (current / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color = bar_color(fraction);

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::vertical(Val::Px(2.0)),
            ..default()
        })
        .id();

    let label = commands
        .spawn((
            Node {
                width: Val::Px(50.0),
                ..default()
            },
            Text::new("  Food"),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    let bar_container = commands
        .spawn(Node {
            width: Val::Px(BAR_WIDTH),
            height: Val::Px(BAR_HEIGHT),
            ..default()
        })
        .id();

    let filled = commands
        .spawn((
            Node {
                width: Val::Percent(fraction * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();

    let empty = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(BAR_BG),
        ))
        .id();

    commands
        .entity(bar_container)
        .add_children(&[filled, empty]);

    let value_text = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Px(4.0)),
                ..default()
            },
            Text::new(format!("{:.0}/{:.0}", current, capacity)),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands
        .entity(row)
        .add_children(&[label, bar_container, value_text]);
    row
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::{ItemKind, ItemLocation};
    use bevy_ecs::system::SystemState;

    fn count_in_world(world: &mut World) -> Vec<(ItemCategory, u32)> {
        let mut state: SystemState<Query<&Item>> = SystemState::new(world);
        let items = state.get(world);
        count_items_by_category(&items)
    }

    #[test]
    fn count_items_by_category_groups_and_orders() {
        let mut world = World::new();
        world.spawn(Item::new(ItemKind::RawMouse, 1.0, ItemLocation::OnGround));
        world.spawn(Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround));
        world.spawn(Item::new(ItemKind::HerbCatnip, 1.0, ItemLocation::OnGround));
        world.spawn(Item::new(ItemKind::Wood, 1.0, ItemLocation::OnGround));
        world.spawn(Item::new(ItemKind::Barrel, 1.0, ItemLocation::OnGround));
        world.spawn(Item::new(
            ItemKind::ShinyPebble,
            1.0,
            ItemLocation::OnGround,
        ));

        let counts = count_in_world(&mut world);

        // Expect display order: RawFood, Herb, Material, StorageUpgrade, Curiosity.
        let cats: Vec<ItemCategory> = counts.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            cats,
            vec![
                ItemCategory::RawFood,
                ItemCategory::Herb,
                ItemCategory::Material,
                ItemCategory::StorageUpgrade,
                ItemCategory::Curiosity,
            ]
        );

        assert_eq!(counts[0], (ItemCategory::RawFood, 2));
        assert_eq!(counts[1], (ItemCategory::Herb, 1));
        assert_eq!(counts[2], (ItemCategory::Material, 1));
        assert_eq!(counts[3], (ItemCategory::StorageUpgrade, 1));
        assert_eq!(counts[4], (ItemCategory::Curiosity, 1));
    }

    #[test]
    fn count_items_by_category_omits_empty_categories() {
        let mut world = World::new();
        world.spawn(Item::new(
            ItemKind::HerbCalmroot,
            1.0,
            ItemLocation::OnGround,
        ));

        let counts = count_in_world(&mut world);

        assert_eq!(counts.len(), 1, "only one non-empty category");
        assert_eq!(counts[0], (ItemCategory::Herb, 1));
    }

    #[test]
    fn count_items_by_category_empty_world() {
        let mut world = World::new();
        let counts = count_in_world(&mut world);
        assert!(counts.is_empty());
    }
}
