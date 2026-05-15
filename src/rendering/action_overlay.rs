use bevy::prelude::*;
use bevy::sprite::Text2d;

use crate::ai::CurrentAction;
use crate::rendering::entity_sprites::EntitySpriteMarker;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};

/// Whether the per-cat action debug overlay is visible (F9 toggle).
#[derive(Resource, Default)]
pub struct ActionOverlayEnabled(pub bool);

/// Marker on the Text2d child entity that shows a cat's current action.
#[derive(Component)]
pub struct ActionOverlayLabel;

/// Stored on a cat entity: points to its action overlay label child.
#[derive(Component)]
pub struct ActionOverlayLabelEntity(pub Entity);

const OVERLAY_LABEL_COLOR: Color = Color::srgba(1.0, 0.95, 0.2, 0.9);

/// Spawn a hidden action label child for every cat that has a sprite but no label yet.
#[allow(clippy::type_complexity)]
pub fn spawn_action_overlay_labels(
    mut commands: Commands,
    cats: Query<
        (Entity, &CurrentAction),
        (With<EntitySpriteMarker>, Without<ActionOverlayLabelEntity>),
    >,
) {
    let world_px = TILE_PX * TILE_SCALE;
    for (entity, current_action) in &cats {
        let label = commands
            .spawn((
                Text2d::new(format!("{:?}", current_action.action)),
                TextFont {
                    font_size: 8.0,
                    ..Default::default()
                },
                TextColor(OVERLAY_LABEL_COLOR),
                // Position above the name label (name label is at world_px * 0.55).
                Transform::from_xyz(0.0, world_px * 0.9, 1.0),
                Visibility::Hidden,
                ActionOverlayLabel,
            ))
            .id();
        commands.entity(entity).add_children(&[label]);
        commands.entity(entity).insert(ActionOverlayLabelEntity(label));
    }
}

/// F9 toggles the action overlay on/off, updating visibility of all labels.
pub fn toggle_action_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<ActionOverlayEnabled>,
    mut labels: Query<&mut Visibility, With<ActionOverlayLabel>>,
) {
    if !keyboard.just_pressed(KeyCode::F9) {
        return;
    }
    overlay.0 = !overlay.0;
    let vis = if overlay.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut labels {
        *v = vis;
    }
}

/// When the overlay is on, update label text whenever a cat's action changes.
pub fn update_action_overlay_labels(
    overlay: Res<ActionOverlayEnabled>,
    cats: Query<(&CurrentAction, &ActionOverlayLabelEntity), Changed<CurrentAction>>,
    mut labels: Query<&mut Text2d, With<ActionOverlayLabel>>,
) {
    if !overlay.0 {
        return;
    }
    for (current_action, label_entity) in &cats {
        if let Ok(mut text) = labels.get_mut(label_entity.0) {
            **text = format!("{:?}", current_action.action);
        }
    }
}
