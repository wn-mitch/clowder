use bevy::prelude::*;

use crate::components::physical::Position;
use crate::components::prey::{PreyAnimal, PreyConfig, PreyState};
use crate::components::wildlife::{BehaviorType, WildAnimal};
use crate::rendering::ui::{PANEL_BG, PANEL_BORDER, TEXT_COLOR, TEXT_DIM, TEXT_HIGHLIGHT, UiRoot};
use crate::ui_data::{InspectionMode, InspectionState};

const FONT_SIZE: f32 = 11.0;
const HEADER_FONT_SIZE: f32 = 14.0;

#[derive(Component)]
pub struct WildlifeInspectPopup;

#[derive(Component)]
pub struct WildlifeInspectContent;

pub fn setup_wildlife_inspect(
    mut commands: Commands,
    root_query: Query<Entity, With<UiRoot>>,
) {
    let Ok(root) = root_query.single() else { return };

    let popup = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(100.0),
                top: Val::Px(100.0),
                width: Val::Px(220.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..Default::default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::from(PANEL_BORDER),
            Visibility::Hidden,
            WildlifeInspectPopup,
        ))
        .id();

    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            WildlifeInspectContent,
        ))
        .id();

    commands.entity(popup).add_children(&[content]);
    commands.entity(root).add_children(&[popup]);
}

pub fn update_wildlife_inspect(
    mut commands: Commands,
    inspection: Res<InspectionState>,
    mut popup_query: Query<(&mut Visibility, &mut Node), With<WildlifeInspectPopup>>,
    content_query: Query<Entity, With<WildlifeInspectContent>>,
    wildlife: Query<(&Position, &WildAnimal)>,
    prey: Query<(&Position, &PreyConfig, &PreyState), With<PreyAnimal>>,
    windows: Query<&Window>,
    mut last_entity: Local<Option<Entity>>,
) {
    let should_show = matches!(inspection.mode, InspectionMode::WildlifeInspect(_));

    for (mut vis, mut node) in &mut popup_query {
        if should_show {
            *vis = Visibility::Inherited;

            if let Some(click_pos) = inspection.click_screen_pos {
                let window_width = windows.single().map(|w| w.width()).unwrap_or(1280.0);
                let window_height = windows.single().map(|w| w.height()).unwrap_or(720.0);
                let popup_w = 220.0;
                let popup_h = 200.0;
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
        *last_entity = None;
        return;
    }

    let InspectionMode::WildlifeInspect(entity) = inspection.mode else {
        return;
    };

    if *last_entity == Some(entity) {
        return;
    }
    *last_entity = Some(entity);

    let Ok(content_entity) = content_query.single() else { return };
    commands.entity(content_entity).despawn_related::<Children>();

    let mut children: Vec<Entity> = Vec::new();

    // Try wild animal first, then prey.
    if let Ok((pos, animal)) = wildlife.get(entity) {
        children.push(spawn_text(
            &mut commands,
            &capitalize(animal.species.name()),
            HEADER_FONT_SIZE,
            TEXT_HIGHLIGHT,
        ));
        children.push(spawn_prop(
            &mut commands,
            "Behavior",
            behavior_label(animal.behavior),
        ));
        children.push(spawn_prop(
            &mut commands,
            "Threat",
            &format!("{:.0}%", animal.threat_power * 100.0),
        ));
        children.push(spawn_prop(
            &mut commands,
            "Defense",
            &format!("{:.0}%", animal.defense * 100.0),
        ));
        children.push(spawn_prop(
            &mut commands,
            "Position",
            &format!("({}, {})", pos.x, pos.y),
        ));
    } else if let Ok((pos, config, state)) = prey.get(entity) {
        children.push(spawn_text(
            &mut commands,
            &capitalize(config.name),
            HEADER_FONT_SIZE,
            TEXT_HIGHLIGHT,
        ));
        children.push(spawn_prop(
            &mut commands,
            "Hunger",
            &format!("{:.0}%", state.hunger * 100.0),
        ));
        children.push(spawn_prop(
            &mut commands,
            "Alertness",
            &format!("{:.0}%", state.alertness * 100.0),
        ));
        children.push(spawn_prop(
            &mut commands,
            "State",
            ai_state_label(&state.ai_state),
        ));
        children.push(spawn_prop(
            &mut commands,
            "Position",
            &format!("({}, {})", pos.x, pos.y),
        ));
    } else {
        children.push(spawn_text(&mut commands, "Entity gone", FONT_SIZE, TEXT_DIM));
    }

    commands.entity(content_entity).add_children(&children);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ai_state_label(state: &crate::components::prey::PreyAiState) -> &'static str {
    use crate::components::prey::PreyAiState;
    match state {
        PreyAiState::Idle => "Idle",
        PreyAiState::Grazing { .. } => "Grazing",
        PreyAiState::Alert { .. } => "Alert",
        PreyAiState::Fleeing { .. } => "Fleeing",
    }
}

fn behavior_label(behavior: BehaviorType) -> &'static str {
    match behavior {
        BehaviorType::Patrol => "Patrol",
        BehaviorType::Circle => "Circle",
        BehaviorType::Ambush => "Ambush",
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

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
            Node { width: Val::Px(80.0), ..default() },
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
