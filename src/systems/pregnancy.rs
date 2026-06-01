use bevy_ecs::prelude::*;

use crate::components::fulfillment::Fulfillment;
use crate::components::grooming::GroomingCondition;
use crate::components::identity::{Appearance, Gender, Name};
use crate::components::kitten::KittenDependency;
use crate::components::markers;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::pregnancy::{GestationStage, Pregnant};
use crate::plugins::setup::cat_bundle;
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};
use crate::world_gen::colony::CatBlueprint;

/// Ticket 452 — newborn `GroomingCondition` spawn value. Cats are born
/// covered in birth membrane and amniotic fluid; their coat condition
/// reads low until the mother (or another caretaker) grooms them clean.
/// The §3.5 grooming substrate restores +0.12 per `GroomOther` action
/// (see `src/steps/disposition/groom_other.rs`); the matching
/// `target_grooming_deficit` axis on `GroomOtherTargetDse` lets dirty
/// recipients amplify the maternal-grooming lift.
pub const NEWBORN_GROOMING: f32 = 0.15;

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
            let (blueprint, position, needs, fulfillment, health) =
                build_kitten_blueprint(&birth, time.tick, config.ticks_per_season, &mut rng.rng);

            // Ticket 452 — route through the canonical `cat_bundle` so the
            // production kitten spawn cannot drift from `spawn_cat_from_blueprint`.
            // Kitten-only markers (`KittenDependency`, `BornInSim`) are
            // post-inserted; everything else lives in the canonical bundle.
            let kitten_entity = commands
                .spawn(cat_bundle(
                    blueprint,
                    position,
                    needs,
                    fulfillment,
                    health,
                    GroomingCondition(NEWBORN_GROOMING),
                ))
                .insert((
                    KittenDependency::new(birth.mother, birth.partner.unwrap_or(birth.mother)),
                    // Ticket 166 — born-once identity marker; consumed by
                    // `colony_score.kittens_matured` increment/decrement
                    // in `growth.rs::tick_kitten_growth` and `death.rs::check_death`.
                    markers::BornInSim,
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
                        location: (birth.pos.x(), birth.pos.y()),
                    },
                );
            }
        }

        // New life pushes back darkness.
        pushback.write(crate::systems::magic::CorruptionPushback {
            position: birth.pos,
            radius: 5.0,
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

/// Ticket 452 — build the per-spawn inputs for a newborn kitten. Returns
/// the `CatBlueprint` (identity slot for `cat_bundle`) plus the three
/// per-spawn values that `cat_bundle` takes as parameters (`position`,
/// `needs`, `fulfillment`, `health`). Extracted as a free function so
/// the unit suite can exercise it without scheduling the full
/// `tick_pregnancy` system.
fn build_kitten_blueprint(
    birth: &BirthEvent,
    tick: u64,
    ticks_per_season: u64,
    rng: &mut impl rand::Rng,
) -> (CatBlueprint, Position, Needs, Fulfillment, Health) {
    let kitten_health = 0.7 + birth.avg_nutrition * 0.3;
    let blueprint = CatBlueprint {
        name: generate_kitten_name(rng),
        gender: roll_gender(rng),
        orientation: crate::world_gen::colony::roll_orientation(rng),
        personality: mutate_personality(&birth.mother_personality, rng),
        appearance: Appearance {
            fur_color: "tabby brown".to_string(),
            pattern: "tabby".to_string(),
            eye_color: "blue".to_string(),
            distinguishing_marks: Vec::new(),
        },
        skills: crate::components::skills::Skills::default(),
        magic_affinity: crate::world_gen::colony::roll_magic_affinity(rng),
        zodiac_sign: crate::components::zodiac::ZodiacSign::from_season(
            tick / ticks_per_season,
            rng,
        ),
        position: Position::new(birth.pos.x(), birth.pos.y()),
        born_tick: tick,
    };
    let position = Position::new(birth.pos.x(), birth.pos.y());
    let needs = Needs {
        hunger: 0.5,
        energy: 0.8,
        mating: 1.0,
        ..Needs::default()
    };
    let fulfillment = Fulfillment::newborn();
    let health = Health {
        current: kitten_health,
        max: 1.0,
        total_starvation_damage: 0.0,
    };
    (blueprint, position, needs, fulfillment, health)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::identity::{Gender, Orientation};
    use crate::components::Appearance;
    use crate::plugins::setup::{cat_bundle, spawn_cat_from_blueprint};
    use bevy_ecs::world::World;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn neutral_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    fn sample_birth() -> BirthEvent {
        BirthEvent {
            mother: Entity::from_raw_u32(1).unwrap(),
            mother_name: "Mother".to_string(),
            partner: Some(Entity::from_raw_u32(2).unwrap()),
            litter_size: 1,
            avg_nutrition: 0.9,
            pos: Position::new(5, 5),
            mother_personality: neutral_personality(),
            _mother_gender: Gender::Queen,
        }
    }

    fn sample_blueprint() -> CatBlueprint {
        let mut rng = StdRng::seed_from_u64(42);
        CatBlueprint {
            name: "Founder".to_string(),
            gender: Gender::Queen,
            orientation: crate::world_gen::colony::roll_orientation(&mut rng),
            personality: neutral_personality(),
            appearance: Appearance {
                fur_color: "calico".to_string(),
                pattern: "spotted".to_string(),
                eye_color: "amber".to_string(),
                distinguishing_marks: Vec::new(),
            },
            skills: crate::components::skills::Skills::default(),
            magic_affinity: 0.0,
            zodiac_sign: crate::components::zodiac::ZodiacSign::from_season(0, &mut rng),
            position: Position::new(0, 0),
            born_tick: 0,
        }
    }

    #[test]
    fn build_kitten_blueprint_produces_low_grooming_and_high_social_warmth() {
        // Ticket 452 — newborn defaults reflect "physically dirty,
        // socially nurtured." `Fulfillment::newborn()` sets
        // social_warmth 0.9; the `NEWBORN_GROOMING` constant (0.15)
        // is fed into the spawn via the call site, not the blueprint
        // helper itself, so this test asserts the per-spawn pieces:
        // the Fulfillment, Needs, and Health that travel with the
        // blueprint.
        let mut rng = StdRng::seed_from_u64(42);
        let birth = sample_birth();
        let (_, position, needs, fulfillment, health) =
            build_kitten_blueprint(&birth, 100, 20_000, &mut rng);

        assert!((fulfillment.social_warmth - 0.9).abs() < f32::EPSILON);
        assert!((fulfillment.body_condition - 1.0).abs() < f32::EPSILON);
        assert_eq!(position, Position::new(5, 5));
        assert!((needs.hunger - 0.5).abs() < f32::EPSILON);
        // Health = 0.7 + avg_nutrition (0.9) * 0.3 = 0.97
        assert!((health.current - 0.97).abs() < 1e-5);
        // The grooming-condition value isn't on the blueprint tuple —
        // it's passed at the call site as `GroomingCondition(NEWBORN_GROOMING)`.
        assert!((NEWBORN_GROOMING - 0.15).abs() < f32::EPSILON);
    }

    #[test]
    fn kitten_bundle_archetype_matches_founder() {
        // Ticket 452 — structural anti-drift gate. A founder cat spawned
        // via `spawn_cat_from_blueprint` and a kitten spawned via
        // `cat_bundle + post-insert(KittenDependency, BornInSim)` must
        // have identical archetypes except for the two kitten-only
        // markers. Any future component added to `cat_bundle` that
        // doesn't reach the kitten path (or vice versa) fails this test.
        let mut world = World::new();

        // Founder spawn — canonical path.
        let founder_bp = sample_blueprint();
        let founder = spawn_cat_from_blueprint(
            &mut world,
            founder_bp,
            Position::new(0, 0),
            Needs::default(),
            Fulfillment::default(),
        );

        // Kitten spawn — same canonical bundle, plus the two kitten-only
        // markers. Mirrors the pregnancy.rs production path.
        let kitten_bp = sample_blueprint();
        let kitten = world
            .spawn(cat_bundle(
                kitten_bp,
                Position::new(0, 0),
                Needs::default(),
                Fulfillment::newborn(),
                Health::default(),
                GroomingCondition(NEWBORN_GROOMING),
            ))
            .insert((
                KittenDependency::new(
                    Entity::from_raw_u32(100).unwrap(),
                    Entity::from_raw_u32(101).unwrap(),
                ),
                markers::BornInSim,
            ))
            .id();

        let founder_components: std::collections::HashSet<_> = world
            .entity(founder)
            .archetype()
            .components()
            .iter()
            .copied()
            .collect();
        let kitten_components: std::collections::HashSet<_> = world
            .entity(kitten)
            .archetype()
            .components()
            .iter()
            .copied()
            .collect();

        // Components on kitten but not founder must be exactly
        // {KittenDependency, BornInSim}.
        let kitten_only: std::collections::HashSet<_> = kitten_components
            .difference(&founder_components)
            .copied()
            .collect();
        let kitten_dep_id = world
            .component_id::<KittenDependency>()
            .expect("KittenDependency component should be registered after kitten spawn");
        let born_in_sim_id = world
            .component_id::<markers::BornInSim>()
            .expect("BornInSim marker should be registered after kitten spawn");
        let expected_extras: std::collections::HashSet<_> =
            [kitten_dep_id, born_in_sim_id].into_iter().collect();
        assert_eq!(
            kitten_only, expected_extras,
            "kitten archetype has unexpected extras (or is missing expected kitten-only markers)"
        );

        // Components on founder but not kitten must be empty — every
        // founder-bundle component must reach the kitten spawn path.
        let founder_only: std::collections::HashSet<_> = founder_components
            .difference(&kitten_components)
            .copied()
            .collect();
        assert!(
            founder_only.is_empty(),
            "founder has {} components missing from the kitten spawn — \
             a future founder-bundle addition has not been routed through `cat_bundle`",
            founder_only.len()
        );
    }

    // Silence unused-import warnings when Orientation isn't read.
    #[allow(dead_code)]
    fn _force_orientation_use(_o: Orientation) {}
}
