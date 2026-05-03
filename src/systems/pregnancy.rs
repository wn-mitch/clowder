use bevy_ecs::prelude::*;

use crate::components::grooming::GroomingCondition;
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Age, Appearance, Gender, Name, Species};
use crate::components::kitten::KittenDependency;
use crate::components::magic::Inventory;
use crate::components::mental::{Memory, Mood};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::pregnancy::{GestationStage, Pregnant};
use crate::components::skills::{Corruption, MagicAffinity, Training};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};

// ---------------------------------------------------------------------------
// tick_pregnancy system
// ---------------------------------------------------------------------------

/// Advance gestation for all pregnant cats each tick.
///
/// - Tracks nutrition (queen's hunger averaged over pregnancy)
/// - Advances gestation stage at 33%/66% of ticks_per_season
/// - Applies physical effects (hunger/energy drain multipliers) — done in needs system
/// - Triggers birth when gestation is complete
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn tick_pregnancy(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    _constants: Res<SimConstants>,
    mut rng: ResMut<SimRng>,
    mut relationships: ResMut<Relationships>,
    mut query: Query<
        (
            Entity,
            &mut Pregnant,
            &Needs,
            &Position,
            &Personality,
            &Gender,
            &Name,
        ),
        Without<Dead>,
    >,
    mut commands: Commands,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    mut activation: Option<ResMut<SystemActivation>>,
    mut pushback: MessageWriter<crate::systems::magic::CorruptionPushback>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
) {
    let tps = config.ticks_per_season;
    let mut births: Vec<BirthEvent> = Vec::new();

    for (entity, mut preg, needs, pos, personality, gender, name) in &mut query {
        let elapsed = time.tick.saturating_sub(preg.conceived_tick);

        // Track nutrition.
        preg.nutrition_sum += needs.hunger;
        preg.nutrition_samples += 1;

        // Advance stage.
        let progress = elapsed as f32 / tps as f32;
        let old_stage = preg.stage;
        preg.stage = if progress < 0.33 {
            GestationStage::Early
        } else if progress < 0.66 {
            GestationStage::Mid
        } else {
            GestationStage::Late
        };
        if preg.stage != old_stage {
            if let Some(ref mut act) = activation {
                act.record(Feature::GestationAdvanced);
            }
        }

        // Birth trigger.
        if elapsed >= tps {
            births.push(BirthEvent {
                mother: entity,
                mother_name: name.0.clone(),
                partner: preg.partner,
                litter_size: preg.litter_size,
                avg_nutrition: preg.avg_nutrition(),
                pos: *pos,
                mother_personality: personality.clone(),
                _mother_gender: *gender,
            });
        }
    }

    // Process births outside the query loop.
    for birth in births {
        commands.entity(birth.mother).remove::<Pregnant>();

        // Spawn kittens.
        for _ in 0..birth.litter_size {
            let kitten_health = 0.7 + birth.avg_nutrition * 0.3;
            let kitten_personality = mutate_personality(&birth.mother_personality, &mut rng.rng);
            let kitten_gender = roll_gender(&mut rng.rng);
            let kitten_orientation = crate::world_gen::colony::roll_orientation(&mut rng.rng);

            let kitten_entity = commands
                .spawn((
                    (
                        Name(generate_kitten_name(&mut rng.rng)),
                        Species,
                        Age::new(time.tick),
                        kitten_gender,
                        kitten_orientation,
                        kitten_personality,
                        Appearance {
                            fur_color: "tabby brown".to_string(),
                            pattern: "tabby".to_string(),
                            eye_color: "blue".to_string(),
                            distinguishing_marks: Vec::new(),
                        },
                        Position::new(birth.pos.x, birth.pos.y),
                        Health {
                            current: kitten_health,
                            max: 1.0,
                            injuries: Vec::new(),
                            total_starvation_damage: 0.0,
                        },
                        Needs {
                            hunger: 0.5,
                            energy: 0.8,
                            mating: 1.0,
                            ..Needs::default()
                        },
                        Mood::default(),
                        Memory::default(),
                    ),
                    (
                        crate::components::zodiac::ZodiacSign::from_season(
                            time.tick / config.ticks_per_season,
                            &mut rng.rng,
                        ),
                        crate::components::skills::Skills::default(),
                        MagicAffinity(crate::world_gen::colony::roll_magic_affinity(&mut rng.rng)),
                        Corruption(0.0),
                        Training::default(),
                        crate::ai::CurrentAction::default(),
                        Inventory::default(),
                        crate::components::disposition::ActionHistory::default(),
                        HuntingPriors::default(),
                        GroomingCondition(1.0),
                        KittenDependency::new(birth.mother, birth.partner.unwrap_or(birth.mother)),
                        crate::components::SensorySpecies::Cat,
                        crate::components::SensorySignature::CAT,
                        // Ticket 073 — per-cat recently-failed target memory.
                        crate::components::RecentTargetFailures::default(),
                    ),
                ))
                .id();

            // Initialize parent-kitten relationships.
            relationships
                .get_or_insert(birth.mother, kitten_entity)
                .fondness = 0.5;
            relationships
                .get_or_insert(birth.mother, kitten_entity)
                .familiarity = 0.3;
            if let Some(partner) = birth.partner {
                relationships.get_or_insert(partner, kitten_entity).fondness = 0.5;
                relationships
                    .get_or_insert(partner, kitten_entity)
                    .familiarity = 0.3;
            }

            if let Some(ref mut score) = colony_score {
                score.kittens_born += 1;
            }
            if let Some(ref mut act) = activation {
                act.record(Feature::KittenBorn);
            }
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    crate::resources::event_log::EventKind::KittenBorn {
                        mother: birth.mother_name.clone(),
                        kitten: format!("{kitten_entity:?}"),
                        location: (birth.pos.x, birth.pos.y),
                    },
                );
            }
        }

        // New life pushes back darkness.
        pushback.write(crate::systems::magic::CorruptionPushback {
            position: birth.pos,
            radius: 5,
            amount: 0.10,
        });
    }
}

struct BirthEvent {
    mother: Entity,
    mother_name: String,
    partner: Option<Entity>,
    litter_size: u8,
    avg_nutrition: f32,
    pos: Position,
    mother_personality: Personality,
    _mother_gender: Gender,
}

/// Mutate a personality by averaging with random variation.
fn mutate_personality(parent: &Personality, rng: &mut impl rand::Rng) -> Personality {
    let mut mutate = |v: f32| -> f32 { (v + rng.random_range(-0.1_f32..=0.1)).clamp(0.0, 1.0) };
    Personality {
        boldness: mutate(parent.boldness),
        sociability: mutate(parent.sociability),
        curiosity: mutate(parent.curiosity),
        diligence: mutate(parent.diligence),
        warmth: mutate(parent.warmth),
        spirituality: mutate(parent.spirituality),
        ambition: mutate(parent.ambition),
        patience: mutate(parent.patience),
        anxiety: mutate(parent.anxiety),
        optimism: mutate(parent.optimism),
        temper: mutate(parent.temper),
        stubbornness: mutate(parent.stubbornness),
        playfulness: mutate(parent.playfulness),
        loyalty: mutate(parent.loyalty),
        tradition: mutate(parent.tradition),
        compassion: mutate(parent.compassion),
        pride: mutate(parent.pride),
        independence: mutate(parent.independence),
    }
}

fn roll_gender(rng: &mut impl rand::Rng) -> Gender {
    match rng.random_range(0..20u32) {
        0..=9 => Gender::Tom,
        10..=18 => Gender::Queen,
        _ => Gender::Nonbinary,
    }
}

const KITTEN_NAMES: &[&str] = &[
    "Kit", "Pebble", "Acorn", "Dewdrop", "Spark", "Bramble", "Wisp", "Fern", "Moss", "Pip",
    "Midge", "Cricket", "Clover", "Sorrel", "Ember", "Wren", "Finch", "Nettle", "Thistle", "Lark",
    "Hazel", "Robin", "Sage", "Flint", "Reed", "Ivy", "Maple", "Thyme", "Cloud", "Berry", "Dusk",
    "Dawn",
];

fn generate_kitten_name(rng: &mut impl rand::Rng) -> String {
    let base = KITTEN_NAMES[rng.random_range(0..KITTEN_NAMES.len())];
    let suffix = rng.random_range(1..100u32);
    format!("{base}kit-{suffix}")
}
