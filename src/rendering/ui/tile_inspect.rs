use bevy::prelude::*;

use crate::components::building::{ConstructionSite, CropState, GateState, Structure};
use crate::components::identity::Species;
use crate::components::physical::{Dead, Position};
use crate::rendering::ui::{TEXT_COLOR, TEXT_DIM, TEXT_HIGHLIGHT, UiRoot};
use crate::resources::map::TileMap;
use crate::ui_data::{terrain_label, InspectionMode, InspectionState};

const FONT_SIZE: f32 = 11.0;
const HEADER_FONT_SIZE: f32 = 14.0;

#[derive(Component)]
pub struct TileInspectPopup;

#[derive(Component)]
pub struct TileInspectContent;

pub fn setup_tile_inspect(
    mut commands: Commands,
    root_query: Query<Entity, With<UiRoot>>,
) {
    let Ok(root) = root_query.single() else { return };

    let popup = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Dynamic positioning set per-frame in update system.
                left: Val::Px(100.0),
                top: Val::Px(100.0),
                width: Val::Px(260.0),
                // Auto height: grow to fit content.
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..Default::default()
            },
            BackgroundColor(crate::rendering::ui::PANEL_BG),
            BorderColor::from(crate::rendering::ui::PANEL_BORDER),
            Visibility::Hidden,
            TileInspectPopup,
        ))
        .id();

    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            TileInspectContent,
        ))
        .id();

    commands.entity(popup).add_children(&[content]);
    commands.entity(root).add_children(&[popup]);
}

pub fn update_tile_inspect(
    mut commands: Commands,
    inspection: Res<InspectionState>,
    mut popup_query: Query<(&mut Visibility, &mut Node), With<TileInspectPopup>>,
    content_query: Query<Entity, With<TileInspectContent>>,
    map: Res<TileMap>,
    cats: Query<(&Position, &Name), (With<Species>, Without<Dead>)>,
    buildings: Query<(
        &Position,
        &Structure,
        Option<&ConstructionSite>,
        Option<&CropState>,
        Option<&GateState>,
    )>,
    windows: Query<&Window>,
    mut last_tile: Local<Option<(i32, i32)>>,
) {
    let should_show = matches!(inspection.mode, InspectionMode::TileInspect { .. });

    for (mut vis, mut node) in &mut popup_query {
        if should_show {
            *vis = Visibility::Inherited;

            // Position near click point, clamped to screen edges.
            if let Some(click_pos) = inspection.click_screen_pos {
                let window_width = windows
                    .single()
                    .map(|w| w.width())
                    .unwrap_or(1280.0);
                let window_height = windows
                    .single()
                    .map(|w| w.height())
                    .unwrap_or(720.0);

                let popup_w = 260.0;
                let popup_h = 300.0; // estimated max height

                let x = (click_pos.x + 20.0).min(window_width - popup_w - 8.0).max(8.0);
                let y = (click_pos.y + 20.0).min(window_height - popup_h - 8.0).max(8.0);

                node.left = Val::Px(x);
                node.top = Val::Px(y);
            }
        } else {
            *vis = Visibility::Hidden;
        }
    }

    if !should_show {
        *last_tile = None;
        return;
    }

    let InspectionMode::TileInspect { x, y } = inspection.mode else {
        return;
    };

    // Only rebuild when tile changes.
    if *last_tile == Some((x, y)) {
        return;
    }
    *last_tile = Some((x, y));

    let Ok(content_entity) = content_query.single() else { return };
    commands.entity(content_entity).despawn_related::<Children>();

    let mut children: Vec<Entity> = Vec::new();

    // Title
    children.push(spawn_text(
        &mut commands,
        &format!("Tile ({x}, {y})"),
        HEADER_FONT_SIZE,
        TEXT_HIGHLIGHT,
    ));

    if !map.in_bounds(x, y) {
        children.push(spawn_text(&mut commands, "Out of bounds", FONT_SIZE, TEXT_DIM));
        commands.entity(content_entity).add_children(&children);
        return;
    }

    let tile = map.get(x, y);

    // Terrain type
    let name = terrain_label(tile.terrain);
    children.push(spawn_text(
        &mut commands,
        &format!("{} {name}", tile.terrain.symbol()),
        FONT_SIZE + 1.0,
        TEXT_COLOR,
    ));

    children.push(spawn_spacer(&mut commands));

    // Properties
    children.push(spawn_text(&mut commands, "Properties", FONT_SIZE + 1.0, TEXT_COLOR));

    let cost = tile.terrain.movement_cost();
    let cost_str = if cost == u32::MAX {
        "impassable".to_string()
    } else {
        format!("{cost}")
    };
    children.push(spawn_prop(&mut commands, "Move cost", &cost_str));
    children.push(spawn_prop(
        &mut commands,
        "Shelter",
        &format!("{:.0}%", tile.terrain.shelter_value() * 100.0),
    ));
    children.push(spawn_prop(
        &mut commands,
        "Forage yield",
        &format!("{:.1}", tile.terrain.foraging_yield()),
    ));
    children.push(spawn_prop(
        &mut commands,
        "Passable",
        if tile.terrain.is_passable() { "yes" } else { "no" },
    ));

    // Corruption / mystery
    if tile.corruption > 0.0 {
        children.push(spawn_spacer(&mut commands));
        children.push(spawn_text(
            &mut commands,
            &format!("Corruption: {:.2}", tile.corruption),
            FONT_SIZE,
            Color::srgb(0.9, 0.25, 0.2),
        ));
    }
    if tile.mystery > 0.0 {
        if tile.corruption == 0.0 {
            children.push(spawn_spacer(&mut commands));
        }
        children.push(spawn_text(
            &mut commands,
            &format!("Mystery: {:.2}", tile.mystery),
            FONT_SIZE,
            Color::srgb(0.7, 0.3, 0.8),
        ));
    }

    // Building info
    for (bpos, structure, construction, crop, gate) in &buildings {
        if bpos.x == x && bpos.y == y {
            children.push(spawn_spacer(&mut commands));
            children.push(spawn_text(&mut commands, "Building", FONT_SIZE + 1.0, TEXT_COLOR));
            children.push(spawn_prop(
                &mut commands,
                "Condition",
                &format!("{:.0}%", structure.condition * 100.0),
            ));
            if let Some(site) = construction {
                children.push(spawn_prop(
                    &mut commands,
                    "Progress",
                    &format!("{:.0}%", site.progress * 100.0),
                ));
                children.push(spawn_prop(
                    &mut commands,
                    "Materials",
                    if site.materials_complete() { "complete" } else { "needed" },
                ));
            }
            if let Some(c) = crop {
                children.push(spawn_prop(
                    &mut commands,
                    "Crop growth",
                    &format!("{:.0}%", c.growth * 100.0),
                ));
            }
            if let Some(g) = gate {
                children.push(spawn_prop(
                    &mut commands,
                    "Gate",
                    if g.open { "open" } else { "closed" },
                ));
            }
            break; // one building per tile
        }
    }

    // Occupants
    let occupants: Vec<&str> = cats
        .iter()
        .filter(|(pos, _)| pos.x == x && pos.y == y)
        .map(|(_, name)| name.as_str())
        .collect();
    if !occupants.is_empty() {
        children.push(spawn_spacer(&mut commands));
        children.push(spawn_text(&mut commands, "Occupants", FONT_SIZE + 1.0, TEXT_COLOR));
        for name in &occupants {
            children.push(spawn_text(
                &mut commands,
                &format!("  {name}"),
                FONT_SIZE,
                TEXT_HIGHLIGHT,
            ));
        }
    }

    commands.entity(content_entity).add_children(&children);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn spawn_text(commands: &mut Commands, content: &str, size: f32, color: Color) -> Entity {
    commands
        .spawn((
            Node {
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            Text::new(content.to_string()),
            TextFont { font_size: size, ..default() },
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

fn spawn_prop(commands: &mut Commands, label: &str, value: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .id();

    let label_node = commands
        .spawn((
            Node { width: Val::Px(100.0), ..default() },
            Text::new(format!("  {label}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    let value_node = commands
        .spawn((
            Text::new(value.to_string()),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    commands.entity(row).add_children(&[label_node, value_node]);
    row
}
