use bevy::prelude::*;

use crate::components::goap_plan::GoapPlan;
use crate::components::identity::{Appearance, Name, Species};
use crate::components::mental::Mood;
use crate::components::physical::{Dead, Needs};
use crate::rendering::ui::{UiRoot, TEXT_DIM, TEXT_HIGHLIGHT};
use crate::resources::TimeState;
use crate::ui_data::{InspectionMode, InspectionState};

/// Marker for the roster panel container.
#[derive(Component)]
pub struct CatRoster;

/// Marker for the scrollable content area.
#[derive(Component)]
pub struct CatRosterContent;

/// Marker on each row, storing the cat entity for click detection.
#[derive(Component)]
pub struct RosterRow(pub Entity);

/// Tracks last tick to debounce rebuilds.
#[derive(Component)]
pub struct RosterState {
    pub last_tick: u64,
    pub last_cat_count: usize,
}

/// Marker for the roster header text.
#[derive(Component)]
pub struct RosterHeader;

const FONT_SIZE: f32 = 11.0;
const MINI_BAR_HEIGHT: f32 = 4.0;
const MINI_BAR_WIDTH: f32 = 50.0;
const BAR_RED: Color = Color::srgb(0.9, 0.25, 0.2);
const BAR_YELLOW: Color = Color::srgb(0.9, 0.75, 0.2);
const BAR_GREEN: Color = Color::srgb(0.3, 0.8, 0.3);
const BAR_BG: Color = Color::srgba(0.2, 0.2, 0.2, 0.6);
const SELECTED_BG: Color = Color::srgba(0.15, 0.15, 0.2, 0.95);

pub fn setup_cat_roster(mut commands: Commands, root_query: Query<Entity, With<UiRoot>>) {
    let Ok(root) = root_query.single() else {
        return;
    };

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                width: Val::Px(220.0),
                height: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::clip_y(),
                ..Default::default()
            },
            BackgroundColor(crate::rendering::ui::PANEL_BG),
            BorderColor::from(crate::rendering::ui::PANEL_BORDER),
            CatRoster,
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..Default::default()
            },
            Text::new("Colony (0)"),
            TextFont {
                font_size: 14.0,
                ..Default::default()
            },
            TextColor(TEXT_HIGHLIGHT),
            RosterHeader,
        ))
        .id();

    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                flex_grow: 1.0,
                ..Default::default()
            },
            ScrollPosition::default(),
            CatRosterContent,
            RosterState {
                last_tick: u64::MAX,
                last_cat_count: 0,
            },
        ))
        .id();

    commands.entity(panel).add_children(&[header, content]);
    commands.entity(root).add_children(&[panel]);
}

#[allow(clippy::too_many_arguments)]
pub fn update_cat_roster(
    mut commands: Commands,
    time_state: Res<TimeState>,
    inspection: Res<InspectionState>,
    mut header_query: Query<&mut Text, With<RosterHeader>>,
    mut content_query: Query<(Entity, &mut RosterState), With<CatRosterContent>>,
    cats: Query<
        (Entity, &Name, &Appearance, &Needs, &Mood, Option<&GoapPlan>),
        (With<Species>, Without<Dead>),
    >,
) {
    let Ok((content_entity, mut state)) = content_query.single_mut() else {
        return;
    };

    let cat_count = cats.iter().count();

    if let Ok(mut header_text) = header_query.single_mut() {
        **header_text = format!("Colony ({cat_count})");
    }

    // Only rebuild when tick advances or cat count changes.
    if state.last_tick == time_state.tick && state.last_cat_count == cat_count {
        return;
    }
    state.last_tick = time_state.tick;
    state.last_cat_count = cat_count;

    commands
        .entity(content_entity)
        .despawn_related::<Children>();

    let mut cat_list: Vec<_> = cats.iter().collect();
    cat_list.sort_by_key(|(e, ..)| *e);

    let selected = match inspection.mode {
        InspectionMode::CatInspect(e) => Some(e),
        _ => None,
    };

    for (entity, name, appearance, needs, mood, disposition) in &cat_list {
        let row = spawn_roster_row(
            &mut commands,
            *entity,
            name,
            appearance,
            needs,
            mood,
            disposition.as_deref(),
            selected == Some(*entity),
        );
        commands.entity(content_entity).add_children(&[row]);
    }
}

pub fn handle_roster_clicks(
    interaction_query: Query<(&Interaction, &RosterRow), Changed<Interaction>>,
    mut inspection: ResMut<InspectionState>,
) {
    for (interaction, roster_row) in &interaction_query {
        if *interaction == Interaction::Pressed {
            inspection.mode = InspectionMode::CatInspect(roster_row.0);
            inspection.last_selected_cat = Some(roster_row.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Row building
// ---------------------------------------------------------------------------

fn spawn_roster_row(
    commands: &mut Commands,
    entity: Entity,
    name: &Name,
    appearance: &Appearance,
    needs: &Needs,
    mood: &Mood,
    disposition: Option<&GoapPlan>,
    selected: bool,
) -> Entity {
    let bg = if selected { SELECTED_BG } else { Color::NONE };

    let row = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                margin: UiRect::bottom(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::from(if selected {
                crate::rendering::ui::PANEL_BORDER
            } else {
                Color::NONE
            }),
            RosterRow(entity),
        ))
        .id();

    // --- Line 1: mood dot + name + disposition ---
    let header_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();

    let mood_dot = commands
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                margin: UiRect::right(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(mood_indicator_color(mood.valence)),
        ))
        .id();

    let name_node = commands
        .spawn((
            Text::new(name.0.clone()),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(fur_color_to_ui(&appearance.fur_color)),
        ))
        .id();

    let left_group = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    commands
        .entity(left_group)
        .add_children(&[mood_dot, name_node]);

    let disp_label = disposition.map(|d| d.kind.label()).unwrap_or("Idle");
    let disp_node = commands
        .spawn((
            Text::new(disp_label.to_string()),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands
        .entity(header_row)
        .add_children(&[left_group, disp_node]);

    // --- Line 2: mini need bars ---
    let bars_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();

    let hunger_bar = spawn_mini_bar(commands, "H", needs.hunger);
    let energy_bar = spawn_mini_bar(commands, "E", needs.energy);
    let safety_bar = spawn_mini_bar(commands, "S", needs.safety);
    commands
        .entity(bars_row)
        .add_children(&[hunger_bar, energy_bar, safety_bar]);

    commands.entity(row).add_children(&[header_row, bars_row]);
    row
}

fn spawn_mini_bar(commands: &mut Commands, label: &str, value: f32) -> Entity {
    let pct = (value * 100.0).round() as u32;

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();

    let label_node = commands
        .spawn((
            Node {
                width: Val::Px(12.0),
                ..default()
            },
            Text::new(label.to_string()),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    let bar_container = commands
        .spawn(Node {
            width: Val::Px(MINI_BAR_WIDTH),
            height: Val::Px(MINI_BAR_HEIGHT),
            ..default()
        })
        .id();

    let filled = commands
        .spawn((
            Node {
                width: Val::Percent(value.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(bar_color(value)),
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

    let pct_node = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            Text::new(format!("{pct}%")),
            TextFont {
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands
        .entity(row)
        .add_children(&[label_node, bar_container, pct_node]);
    row
}

fn bar_color(value: f32) -> Color {
    if value < 0.2 {
        BAR_RED
    } else if value < 0.5 {
        BAR_YELLOW
    } else {
        BAR_GREEN
    }
}

fn mood_indicator_color(valence: f32) -> Color {
    if valence > 0.3 {
        BAR_GREEN
    } else if valence > -0.3 {
        BAR_YELLOW
    } else {
        BAR_RED
    }
}

/// Fur colors brightened slightly for readability as text on dark backgrounds.
fn fur_color_to_ui(fur: &str) -> Color {
    match fur {
        "ginger" => Color::srgb(1.0, 0.65, 0.3),
        "black" => Color::srgb(0.5, 0.5, 0.55),
        "white" => Color::srgb(0.95, 0.95, 0.92),
        "gray" => Color::srgb(0.65, 0.65, 0.68),
        "tabby brown" => Color::srgb(0.75, 0.55, 0.3),
        "calico" => Color::srgb(0.95, 0.7, 0.4),
        "tortoiseshell" => Color::srgb(0.7, 0.45, 0.25),
        "cream" => Color::srgb(0.95, 0.88, 0.7),
        "silver" => Color::srgb(0.8, 0.83, 0.85),
        "russet" => Color::srgb(0.85, 0.4, 0.2),
        _ => Color::srgb(0.8, 0.6, 0.4),
    }
}
