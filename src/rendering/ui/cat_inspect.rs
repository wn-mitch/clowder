use bevy::prelude::*;

use crate::ai::CurrentAction;
use crate::components::aspirations::{Aspirations, MilestoneCondition, Preferences, Preference};
use crate::components::disposition::{ActionHistory, Disposition};
use crate::components::coordination::{ActiveDirective, Coordinator};
use crate::components::fate::{FatedLove, FatedRival};
use crate::components::identity::{Age, LifeStage, Species};
use crate::components::mental::Mood;
use crate::components::physical::{Dead, Needs};
use crate::components::skills::Skills;
use crate::components::zodiac::ZodiacSign;
use crate::rendering::ui::{TEXT_COLOR, TEXT_DIM, TEXT_HIGHLIGHT, UiRoot};
use crate::resources::aspiration_registry::AspirationRegistry;
use crate::resources::relationships::Relationships;
use crate::resources::{SimConfig, TimeState};
use crate::ui_data::{InspectionMode, InspectionState};

#[derive(Component)]
pub struct CatInspectPanel;

#[derive(Component)]
pub struct CatInspectContent;

/// Color for critical/low values (red).
const BAR_RED: Color = Color::srgb(0.9, 0.25, 0.2);
/// Color for moderate values (yellow/amber).
const BAR_YELLOW: Color = Color::srgb(0.9, 0.75, 0.2);
/// Color for healthy values (green).
const BAR_GREEN: Color = Color::srgb(0.3, 0.8, 0.3);
/// Color for relationship fondness bars.
const BAR_MAGENTA: Color = Color::srgb(0.8, 0.3, 0.7);
/// Color for aspiration progress bars.
const BAR_PROGRESS: Color = Color::srgb(0.3, 0.7, 0.3);
/// Background of unfilled bar portion.
const BAR_BG: Color = Color::srgba(0.2, 0.2, 0.2, 0.6);

const FONT_SIZE: f32 = 11.0;
const HEADER_FONT_SIZE: f32 = 15.0;
const BAR_WIDTH: f32 = 100.0;
const BAR_HEIGHT: f32 = 10.0;

pub fn setup_cat_inspect_panel(
    mut commands: Commands,
    root_query: Query<Entity, With<UiRoot>>,
) {
    let Ok(root) = root_query.single() else { return };

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                width: Val::Px(340.0),
                height: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            BackgroundColor(crate::rendering::ui::PANEL_BG),
            BorderColor::from(crate::rendering::ui::PANEL_BORDER),
            Visibility::Hidden,
            CatInspectPanel,
        ))
        .id();

    let content = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            CatInspectContent,
        ))
        .id();

    commands.entity(panel).add_children(&[content]);
    commands.entity(root).add_children(&[panel]);
}

#[allow(clippy::too_many_arguments)]
pub fn update_cat_inspect_panel(
    mut commands: Commands,
    inspection: Res<InspectionState>,
    mut panel_query: Query<&mut Visibility, With<CatInspectPanel>>,
    content_query: Query<Entity, With<CatInspectContent>>,
    cats: Query<
        (
            &Name,
            &Age,
            &Needs,
            &Mood,
            &CurrentAction,
            &Skills,
            Option<&Coordinator>,
            Option<&ActiveDirective>,
            Option<&ZodiacSign>,
            Option<&FatedLove>,
            Option<&FatedRival>,
            Option<&Aspirations>,
            Option<&Preferences>,
            Option<&Disposition>,
            Option<&ActionHistory>,
        ),
        (With<Species>, Without<Dead>),
    >,
    names: Query<&Name>,
    relationships: Res<Relationships>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    registry: Option<Res<AspirationRegistry>>,
    mut last_entity: Local<Option<Entity>>,
) {
    let should_show = matches!(inspection.mode, InspectionMode::CatInspect(_));

    for mut vis in &mut panel_query {
        *vis = if should_show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if !should_show {
        *last_entity = None;
        return;
    }

    let InspectionMode::CatInspect(entity) = inspection.mode else {
        return;
    };

    // Only rebuild content when the selected entity changes.
    if *last_entity == Some(entity) {
        return;
    }
    *last_entity = Some(entity);

    let Ok(content_entity) = content_query.single() else { return };

    // Clear previous content.
    commands.entity(content_entity).despawn_related::<Children>();

    // If the cat is dead or despawned, dismiss.
    let Ok((
        name, age, needs, mood, action, skills,
        coordinator, directive, zodiac, fated_love, fated_rival,
        aspirations, preferences, disposition, action_history,
    )) = cats.get(entity) else {
        // Entity gone — clear the panel.
        commands.entity(content_entity).despawn_related::<Children>();
        return;
    };

    let life_stage = age.stage(time.tick, config.ticks_per_season);

    // Build content as child nodes.
    let mut children: Vec<Entity> = Vec::new();

    // --- Header: cat name ---
    children.push(spawn_text(&mut commands, name.as_ref(), HEADER_FONT_SIZE, TEXT_HIGHLIGHT));

    // --- Life stage + mood ---
    let stage_str = match life_stage {
        LifeStage::Kitten => "Kitten",
        LifeStage::Young => "Young",
        LifeStage::Adult => "Adult",
        LifeStage::Elder => "Elder",
    };
    let mood_str = if mood.valence > 0.3 {
        "happy"
    } else if mood.valence > -0.3 {
        "neutral"
    } else {
        "unhappy"
    };
    let mut stage_line = format!("{stage_str} — mood: {mood_str} ({:.1})", mood.valence);
    if coordinator.is_some() {
        stage_line.push_str("  [Coordinator]");
    }
    children.push(spawn_text(&mut commands, &stage_line, FONT_SIZE, TEXT_COLOR));

    // Zodiac
    if let Some(sign) = zodiac {
        children.push(spawn_text(
            &mut commands,
            &format!("Born under {}", sign.label()),
            FONT_SIZE,
            Color::srgb(0.7, 0.5, 0.8),
        ));
    }

    // Current disposition
    if let Some(disp) = disposition {
        let disp_label = if disp.target_completions == u32::MAX {
            disp.kind.label().to_string()
        } else {
            format!("{} ({}/{})", disp.kind.label(), disp.completions, disp.target_completions)
        };
        children.push(spawn_text(
            &mut commands,
            &format!("Disposition: {disp_label}"),
            FONT_SIZE,
            Color::srgb(0.3, 0.8, 0.3),
        ));
    }

    // Current action
    children.push(spawn_text(
        &mut commands,
        &format!("Action: {:?}", action.action),
        FONT_SIZE,
        Color::srgb(0.4, 0.8, 0.9),
    ));

    // Active directive
    if let Some(dir) = directive {
        children.push(spawn_text(
            &mut commands,
            &format!("Directed: {:?}", dir.kind),
            FONT_SIZE,
            TEXT_HIGHLIGHT,
        ));
    }

    // Fated love
    if let Some(fl) = fated_love {
        let partner_name = names
            .get(fl.partner)
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "???".to_string());
        let status = if fl.awakened { "awakened" } else { "dormant" };
        children.push(spawn_text(
            &mut commands,
            &format!("Fated Love: {partner_name} ({status})"),
            FONT_SIZE,
            Color::srgb(0.9, 0.4, 0.4),
        ));
    }

    // Fated rival
    if let Some(fr) = fated_rival {
        let rival_name = names
            .get(fr.rival)
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "???".to_string());
        let status = if fr.awakened { "awakened" } else { "dormant" };
        children.push(spawn_text(
            &mut commands,
            &format!("Fated Rival: {rival_name} ({status})"),
            FONT_SIZE,
            Color::srgb(0.4, 0.5, 0.9),
        ));
    }

    // --- Spacer ---
    children.push(spawn_spacer(&mut commands));

    // --- Needs section ---
    children.push(spawn_text(&mut commands, "Needs", FONT_SIZE + 1.0, TEXT_COLOR));
    children.push(spawn_bar_row(&mut commands, "Hunger", needs.hunger));
    children.push(spawn_bar_row(&mut commands, "Energy", needs.energy));
    children.push(spawn_bar_row(&mut commands, "Warmth", needs.warmth));
    children.push(spawn_bar_row(&mut commands, "Safety", needs.safety));
    children.push(spawn_bar_row(&mut commands, "Social", needs.social));

    children.push(spawn_spacer(&mut commands));

    // --- Skills section ---
    children.push(spawn_text(&mut commands, "Skills", FONT_SIZE + 1.0, TEXT_COLOR));
    children.push(spawn_skill_line(&mut commands, "Hunting", skills.hunting));
    children.push(spawn_skill_line(&mut commands, "Foraging", skills.foraging));
    children.push(spawn_skill_line(&mut commands, "Herbcraft", skills.herbcraft));
    children.push(spawn_skill_line(&mut commands, "Building", skills.building));
    children.push(spawn_skill_line(&mut commands, "Combat", skills.combat));
    children.push(spawn_skill_line(&mut commands, "Magic", skills.magic));

    // --- Relationships ---
    let rels = relationships.all_for(entity);
    if !rels.is_empty() {
        children.push(spawn_spacer(&mut commands));
        children.push(spawn_text(&mut commands, "Relationships", FONT_SIZE + 1.0, TEXT_COLOR));
        for (other, rel) in &rels {
            let other_name = names
                .get(*other)
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "???".to_string());
            let bond_str = match rel.bond {
                Some(crate::resources::relationships::BondType::Friends) => " Friends",
                Some(crate::resources::relationships::BondType::Partners) => " Partners",
                Some(crate::resources::relationships::BondType::Mates) => " Mates",
                None => "",
            };
            // Fondness bar: map [-1, 1] to [0, 1] for display.
            let normalized = (rel.fondness + 1.0) / 2.0;
            children.push(spawn_relationship_row(
                &mut commands,
                &other_name,
                normalized,
                bond_str,
            ));
        }
    }

    // --- Aspirations ---
    if let Some(asps) = aspirations {
        if !asps.active.is_empty() || !asps.completed.is_empty() {
            children.push(spawn_spacer(&mut commands));
            children.push(spawn_text(&mut commands, "Aspirations", FONT_SIZE + 1.0, TEXT_COLOR));

            for asp in &asps.active {
                let (milestone_name, target) = registry
                    .as_ref()
                    .and_then(|reg| reg.chain_by_name(&asp.chain_name))
                    .and_then(|chain| chain.milestones.get(asp.current_milestone))
                    .map(|ms| {
                        let t = milestone_target(&ms.condition);
                        (ms.name.clone(), t)
                    })
                    .unwrap_or_else(|| (format!("milestone {}", asp.current_milestone), 1));

                let frac = if target > 0 {
                    asp.progress as f32 / target as f32
                } else {
                    1.0
                };
                children.push(spawn_aspiration_row(
                    &mut commands,
                    &asp.chain_name,
                    &milestone_name,
                    frac.clamp(0.0, 1.0),
                    asp.progress,
                    target,
                ));
            }

            for name in &asps.completed {
                children.push(spawn_text(
                    &mut commands,
                    &format!("  {name} (complete)"),
                    FONT_SIZE,
                    BAR_GREEN,
                ));
            }
        }
    }

    // --- Likes / Dislikes ---
    if let Some(prefs) = preferences {
        let likes: Vec<String> = prefs
            .action_preferences
            .iter()
            .filter(|(_, p)| *p == Preference::Like)
            .map(|(a, _)| format!("{a:?}"))
            .collect();
        let dislikes: Vec<String> = prefs
            .action_preferences
            .iter()
            .filter(|(_, p)| *p == Preference::Dislike)
            .map(|(a, _)| format!("{a:?}"))
            .collect();

        if !likes.is_empty() || !dislikes.is_empty() {
            children.push(spawn_spacer(&mut commands));
            if !likes.is_empty() {
                children.push(spawn_text(
                    &mut commands,
                    &format!("Likes: {}", likes.join(", ")),
                    FONT_SIZE,
                    BAR_GREEN,
                ));
            }
            if !dislikes.is_empty() {
                children.push(spawn_text(
                    &mut commands,
                    &format!("Dislikes: {}", dislikes.join(", ")),
                    FONT_SIZE,
                    BAR_RED,
                ));
            }
        }
    }

    // Action history
    if let Some(hist) = action_history {
        if !hist.entries.is_empty() {
            children.push(spawn_spacer(&mut commands));
            children.push(spawn_text(
                &mut commands,
                "Recent Actions",
                FONT_SIZE,
                TEXT_HIGHLIGHT,
            ));
            for entry in hist.entries.iter().rev().take(5) {
                use crate::components::disposition::ActionOutcome;
                let color = match entry.outcome {
                    ActionOutcome::Success => TEXT_DIM,
                    ActionOutcome::Failure => BAR_RED,
                    ActionOutcome::Interrupted => BAR_YELLOW,
                };
                let outcome_str = match entry.outcome {
                    ActionOutcome::Success => "ok",
                    ActionOutcome::Failure => "fail",
                    ActionOutcome::Interrupted => "interrupted",
                };
                children.push(spawn_text(
                    &mut commands,
                    &format!("  t{}: {:?} ({})", entry.tick, entry.action, outcome_str),
                    FONT_SIZE,
                    color,
                ));
            }
        }
    }

    commands.entity(content_entity).add_children(&children);
}

// ---------------------------------------------------------------------------
// Helper spawners
// ---------------------------------------------------------------------------

fn milestone_target(condition: &MilestoneCondition) -> u32 {
    match condition {
        MilestoneCondition::ActionCount { count, .. } => *count,
        MilestoneCondition::WitnessEvent { count, .. } => *count,
        MilestoneCondition::Mentor { count } => *count,
        // Skill level and bond formation are binary — either met or not.
        MilestoneCondition::SkillLevel { .. } | MilestoneCondition::FormBond { .. } => 1,
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
            height: Val::Px(6.0),
            ..default()
        })
        .id()
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

/// Spawn a labeled bar row: "Label [====----] 75%"
fn spawn_bar_row(commands: &mut Commands, label: &str, value: f32) -> Entity {
    let pct = (value * 100.0).round() as u32;
    let color = bar_color(value);

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .id();

    // Label
    let label_node = commands
        .spawn((
            Node {
                width: Val::Px(65.0),
                ..default()
            },
            Text::new(format!("  {label}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    // Bar container
    let bar_container = commands
        .spawn(Node {
            width: Val::Px(BAR_WIDTH),
            height: Val::Px(BAR_HEIGHT),
            ..default()
        })
        .id();

    // Filled portion
    let filled = commands
        .spawn((
            Node {
                width: Val::Percent(value.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();

    // Empty portion
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

    commands.entity(bar_container).add_children(&[filled, empty]);

    // Percentage text
    let pct_node = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Px(4.0)),
                ..default()
            },
            Text::new(format!("{pct}%")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands.entity(row).add_children(&[label_node, bar_container, pct_node]);
    row
}

fn spawn_skill_line(commands: &mut Commands, label: &str, value: f32) -> Entity {
    commands
        .spawn((
            Node {
                margin: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            Text::new(format!("  {label:<10} {value:.2}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(Color::srgb(0.4, 0.8, 0.9)),
        ))
        .id()
}

fn spawn_relationship_row(
    commands: &mut Commands,
    name: &str,
    normalized_fondness: f32,
    bond_str: &str,
) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .id();

    let label = commands
        .spawn((
            Node { width: Val::Px(90.0), ..default() },
            Text::new(format!("  {name}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let bar_container = commands
        .spawn(Node {
            width: Val::Px(50.0),
            height: Val::Px(BAR_HEIGHT),
            ..default()
        })
        .id();

    let filled = commands
        .spawn((
            Node {
                width: Val::Percent(normalized_fondness.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(BAR_MAGENTA),
        ))
        .id();

    let empty = commands
        .spawn((
            Node { flex_grow: 1.0, height: Val::Percent(100.0), ..default() },
            BackgroundColor(BAR_BG),
        ))
        .id();

    commands.entity(bar_container).add_children(&[filled, empty]);

    let bond_node = commands
        .spawn((
            Node { margin: UiRect::left(Val::Px(4.0)), ..default() },
            Text::new(bond_str.to_string()),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_HIGHLIGHT),
        ))
        .id();

    commands.entity(row).add_children(&[label, bar_container, bond_node]);
    row
}

fn spawn_aspiration_row(
    commands: &mut Commands,
    chain_name: &str,
    milestone_name: &str,
    fraction: f32,
    progress: u32,
    target: u32,
) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();

    let header = commands
        .spawn((
            Text::new(format!("  {chain_name}: {milestone_name}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(Color::srgb(0.4, 0.8, 0.9)),
        ))
        .id();

    let bar_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::left(Val::Px(12.0)),
            ..default()
        })
        .id();

    let bar_container = commands
        .spawn(Node {
            width: Val::Px(80.0),
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
            BackgroundColor(BAR_PROGRESS),
        ))
        .id();

    let empty_part = commands
        .spawn((
            Node { flex_grow: 1.0, height: Val::Percent(100.0), ..default() },
            BackgroundColor(BAR_BG),
        ))
        .id();

    commands.entity(bar_container).add_children(&[filled, empty_part]);

    let count = commands
        .spawn((
            Node { margin: UiRect::left(Val::Px(4.0)), ..default() },
            Text::new(format!("{progress}/{target}")),
            TextFont { font_size: FONT_SIZE, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands.entity(bar_row).add_children(&[bar_container, count]);
    commands.entity(row).add_children(&[header, bar_row]);
    row
}
