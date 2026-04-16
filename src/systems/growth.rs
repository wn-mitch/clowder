use bevy_ecs::prelude::*;

use crate::components::identity::Species;
use crate::components::kitten::KittenDependency;
use crate::components::mental::{Mood, MoodModifier};
use crate::components::physical::{Dead, Position};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};

// ---------------------------------------------------------------------------
// tick_kitten_growth system
// ---------------------------------------------------------------------------

/// Advance kitten maturity each tick. At maturity >= 1.0 the
/// `KittenDependency` component is removed and the cat gains full
/// capabilities.
///
/// Maturity rate: `1.0 / (4.0 * ticks_per_season)` per tick — independence
/// after exactly 4 seasons.
pub fn tick_kitten_growth(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut query: Query<(Entity, &mut KittenDependency), Without<Dead>>,
    mut commands: Commands,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let _ = time; // reserved for future use (e.g. nutrition-based growth rate)
    let rate = 1.0 / (4.0 * config.ticks_per_season as f32);

    for (entity, mut dep) in &mut query {
        dep.maturity = (dep.maturity + rate).min(1.0);

        if dep.maturity >= 1.0 {
            commands.entity(entity).remove::<KittenDependency>();
            if let Some(ref mut act) = activation {
                act.record(Feature::KittenMatured);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// kitten_mood_aura system
// ---------------------------------------------------------------------------

/// Kittens provide a persistent mood bonus to nearby adults that scales
/// inversely with maturity. Multiple kittens stack diminishingly.
#[allow(clippy::type_complexity)]
pub fn kitten_mood_aura(
    kittens: Query<(&KittenDependency, &Position), Without<Dead>>,
    mut adults: Query<
        (&Position, &mut Mood),
        (With<Species>, Without<Dead>, Without<KittenDependency>),
    >,
) {
    let kitten_data: Vec<(f32, Position)> = kittens
        .iter()
        .map(|(dep, pos)| (dep.maturity, *pos))
        .collect();

    if kitten_data.is_empty() {
        return;
    }

    for (adult_pos, mut mood) in &mut adults {
        // Collect bonuses from nearby kittens.
        let mut bonuses: Vec<f32> = kitten_data
            .iter()
            .filter(|(_, kpos)| adult_pos.manhattan_distance(kpos) <= 5)
            .map(|(maturity, _)| 0.15 * (1.0 - maturity))
            .filter(|b| *b > 0.0)
            .collect();

        if bonuses.is_empty() {
            continue;
        }

        // Sort descending and stack diminishingly.
        bonuses.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let total: f32 = bonuses
            .iter()
            .enumerate()
            .map(|(i, b)| b * (0.5_f32).powi(i as i32))
            .sum();

        // Refresh the kitten-aura modifier each tick.
        if let Some(existing) = mood
            .modifiers
            .iter_mut()
            .find(|m| m.source == "kitten_aura")
        {
            existing.amount = total;
            existing.ticks_remaining = 2;
        } else {
            mood.modifiers.push_back(MoodModifier {
                amount: total,
                ticks_remaining: 2,
                source: "kitten_aura".to_string(),
            });
        }
    }
}
