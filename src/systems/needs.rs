use bevy_ecs::prelude::*;

use crate::components::identity::{Age, LifeStage, Orientation};
use crate::components::magic::Inventory;
use crate::components::mental::{LocationPreferences, Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::pregnancy::Pregnant;

use crate::resources::sim_constants::SimConstants;
use crate::resources::time::{Season, SimConfig, TimeState};
use crate::resources::weather::{Weather, WeatherState};

// ---------------------------------------------------------------------------
// decay_needs system
// ---------------------------------------------------------------------------

/// Advance need decay for every cat entity each tick.
///
/// - Physiological needs (hunger, energy, warmth) drain continuously.
/// - Safety *recovers* passively toward 1.0; it drops only from events.
/// - Social/acceptance/respect/mastery/purpose decay faster for cats whose
///   personality traits make them more invested in those needs.
/// - Warmth takes additional drain from cold weather and winter seasons.
/// - **Starvation** (hunger == 0): drains health, drops safety, doubles
///   social decay, and applies a persistent mood penalty.
pub fn decay_needs(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
    mut query: Query<
        (
            &mut Needs,
            &Personality,
            &mut Health,
            &mut Mood,
            &Position,
            Option<&LocationPreferences>,
            Option<&crate::components::grooming::GroomingCondition>,
            &Age,
            &Orientation,
            Option<&Pregnant>,
        ),
        Without<Dead>,
    >,
) {
    let c = &constants.needs;
    let season = time.season(&config);

    // Additional warmth drain from weather.
    let weather_warmth_drain = match weather.current {
        Weather::Snow => c.weather_warmth_snow,
        Weather::Storm => c.weather_warmth_storm,
        Weather::Wind => c.weather_warmth_wind,
        Weather::HeavyRain => c.weather_warmth_heavy_rain,
        Weather::LightRain => c.weather_warmth_light_rain,
        _ => 0.0,
    };

    // Additional warmth drain from season.
    let season_warmth_drain = match season {
        Season::Winter => c.season_warmth_winter,
        Season::Autumn => c.season_warmth_autumn,
        _ => 0.0,
    };

    let warmth_drain = c.base_warmth_drain + weather_warmth_drain + season_warmth_drain;

    for (
        mut needs,
        personality,
        mut health,
        mut mood,
        pos,
        loc_prefs,
        grooming,
        age,
        orientation,
        pregnant,
    ) in &mut query
    {
        // --- Level 1: Physiological ---
        needs.hunger = (needs.hunger - c.hunger_decay).max(0.0);
        needs.energy = (needs.energy - c.energy_decay).max(0.0);
        needs.warmth = (needs.warmth - warmth_drain).max(0.0);

        // --- Starvation cascade ---
        let starving = needs.hunger == 0.0;
        if starving {
            // Health drains when starving.
            health.current = (health.current - c.starvation_health_drain).max(0.0);

            // Safety drops from existential anxiety.
            needs.safety = (needs.safety - c.starvation_safety_drain).max(0.0);

            // Persistent mood penalty (refresh each tick while starving).
            if !mood.modifiers.iter().any(|m| m.source == "starvation") {
                mood.modifiers.push_back(MoodModifier {
                    amount: c.starvation_mood_penalty,
                    ticks_remaining: c.starvation_mood_ticks,
                    source: "starvation".to_string(),
                });
            } else {
                // Refresh the existing starvation modifier.
                for m in mood.modifiers.iter_mut() {
                    if m.source == "starvation" {
                        m.ticks_remaining = c.starvation_mood_ticks;
                    }
                }
            }
        }

        // --- Level 2: Safety — recovers passively (unless starving) ---
        if !starving {
            needs.safety = (needs.safety + c.safety_recovery_rate).min(1.0);
        }

        // --- Level 3: Belonging ---
        let social_multiplier = if starving {
            c.starvation_social_multiplier
        } else {
            1.0
        };
        let social_drain = c.social_base_drain
            * (1.0 + personality.sociability * c.social_sociability_scale)
            * social_multiplier;
        needs.social = (needs.social - social_drain).max(0.0);

        let acceptance_drain =
            c.acceptance_base_drain * (1.0 + personality.warmth * c.acceptance_warmth_scale);
        needs.acceptance = (needs.acceptance - acceptance_drain).max(0.0);

        // Mating need: decays only for Adult/Elder, Spring/Summer, non-Asexual, not pregnant.
        let life_stage = age.stage(time.tick, config.ticks_per_season);
        let is_fertile = matches!(life_stage, LifeStage::Adult | LifeStage::Elder)
            && matches!(season, Season::Spring | Season::Summer)
            && *orientation != Orientation::Asexual
            && pregnant.is_none();
        if is_fertile {
            let mating_drain =
                c.mating_base_decay * (1.0 + personality.warmth * c.mating_warmth_scale);
            needs.mating = (needs.mating - mating_drain).max(0.0);
        }

        // --- Level 4: Esteem ---
        // Pride amplifies respect decay when respect is already low.
        let pride_amplifier = if needs.respect < c.respect_low_threshold {
            1.0 + personality.pride
                * c.pride_amplifier_scale
                * (1.0 - needs.respect / c.respect_low_threshold)
        } else {
            1.0
        };
        let respect_drain = c.respect_base_drain
            * (1.0 + personality.ambition * c.respect_ambition_scale)
            * pride_amplifier;
        needs.respect = (needs.respect - respect_drain).max(0.0);

        // Grooming penalty: unkempt cats lose additional pride/respect.
        // High-pride cats care more about appearance — amplifies the depression spiral.
        if let Some(g) = grooming {
            let grooming_penalty = (1.0 - g.0) * personality.pride * c.grooming_pride_penalty_scale;
            needs.respect = (needs.respect - grooming_penalty).max(0.0);
        }

        let mastery_drain =
            c.mastery_base_drain * (1.0 + personality.diligence * c.mastery_diligence_scale);
        needs.mastery = (needs.mastery - mastery_drain).max(0.0);

        // --- Level 5: Self-actualisation ---
        // Patience slows purpose drain; independence speeds it up.
        let purpose_drain = c.purpose_base_drain
            * (1.0 + personality.curiosity * c.purpose_curiosity_scale)
            * (1.0 - personality.patience * c.purpose_patience_scale)
            * (1.0 + personality.independence * c.purpose_independence_scale);
        needs.purpose = (needs.purpose - purpose_drain).max(0.0);

        // --- Tradition: familiar territory modifies safety ---
        if let Some(prefs) = loc_prefs {
            if let Some((fam_x, fam_y)) = prefs.most_frequented() {
                let fam_pos = Position::new(fam_x, fam_y);
                let dist = pos.manhattan_distance(&fam_pos);
                if dist <= c.tradition_familiar_distance {
                    needs.safety =
                        (needs.safety + personality.tradition * c.tradition_safety_boost).min(1.0);
                } else {
                    needs.safety =
                        (needs.safety - personality.tradition * c.tradition_safety_drain).max(0.0);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// decay_grooming — passive grooming condition degradation
// ---------------------------------------------------------------------------

/// Grooming condition decays passively at a fixed rate. A fully groomed cat
/// (1.0) reaches unkempt territory (~0.3) in about one season.
pub fn decay_grooming(
    constants: Res<SimConstants>,
    mut query: Query<&mut crate::components::grooming::GroomingCondition, Without<Dead>>,
) {
    let rate = constants.needs.grooming_decay;
    for mut grooming in &mut query {
        grooming.0 = (grooming.0 - rate).max(0.0);
    }
}

// ---------------------------------------------------------------------------
// eat_from_inventory — hungry cats eat carried food
// ---------------------------------------------------------------------------

/// A hungry cat with food in its inventory eats directly rather than
/// waiting to deposit at stores. Keeps cats alive during long hunts.
/// Corruption penalty comes from the item's modifiers (stamped at catch
/// time), not from the cat's current tile.
pub fn eat_from_inventory(
    constants: Res<SimConstants>,
    mut query: Query<(&mut Needs, &mut Inventory), Without<Dead>>,
) {
    let c = &constants.needs;
    for (mut needs, mut inventory) in &mut query {
        if needs.hunger < c.eat_from_inventory_threshold {
            if let Some((kind, modifiers)) = inventory.take_food() {
                let freshness = 1.0 - modifiers.corruption * c.corruption_food_penalty;
                needs.hunger = (needs.hunger + kind.food_value() * freshness).min(1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// bond_proximity_social — friends nearby restore social need
// ---------------------------------------------------------------------------

/// Cats within range of a bonded companion (Friend, Partner, or Mate) get
/// a small per-tick social need recovery — being around friends is inherently
/// social even without an active Socialize action.
pub fn bond_proximity_social(
    mut query: Query<(Entity, &Position, &mut Needs), Without<Dead>>,
    relationships: Res<crate::resources::relationships::Relationships>,
    constants: Res<SimConstants>,
) {
    let c = &constants.needs;
    // Read pass: snapshot positions.
    let snapshot: Vec<(Entity, Position)> = query.iter().map(|(e, p, _)| (e, *p)).collect();

    // Write pass: boost social for cats near bonded companions.
    for (entity, pos, mut needs) in &mut query {
        let has_nearby_bond = snapshot.iter().any(|&(other, other_pos)| {
            if other == entity {
                return false;
            }
            let dist = pos.manhattan_distance(&other_pos);
            if dist == 0 || dist > c.bond_proximity_range {
                return false;
            }
            relationships
                .get(entity, other)
                .is_some_and(|r| r.bond.is_some())
        });

        if has_nearby_bond {
            needs.social = (needs.social + c.bond_proximity_social_rate).min(1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// decay_exploration — fog-of-war tile decay
// ---------------------------------------------------------------------------

/// Slowly decay explored tiles so old discoveries become worth revisiting.
pub fn decay_exploration(
    constants: Res<SimConstants>,
    mut exploration_map: ResMut<crate::resources::ExplorationMap>,
) {
    exploration_map.decay(constants.disposition.exploration_decay_rate);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::SimConstants;
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_needs);
        (world, schedule)
    }

    fn default_personality() -> Personality {
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

    fn spawn_cat(world: &mut World, needs: Needs, personality: Personality) -> Entity {
        world
            .spawn((
                needs,
                personality,
                Health::default(),
                Mood::default(),
                Position::new(5, 5),
                Age::new(0),
                Orientation::Straight,
            ))
            .id()
    }

    #[test]
    fn hunger_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        let before = world.get::<Needs>(entity).unwrap().hunger;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().hunger;

        let expected = SimConstants::default().needs.hunger_decay;
        assert!(
            after < before,
            "hunger should decrease; before={before}, after={after}"
        );
        assert!(
            (before - after - expected).abs() < 1e-6,
            "expected drain of {expected}"
        );
    }

    #[test]
    fn energy_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        let before = world.get::<Needs>(entity).unwrap().energy;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().energy;

        let expected = SimConstants::default().needs.energy_decay;
        assert!(
            after < before,
            "energy should decrease; before={before}, after={after}"
        );
        assert!(
            (before - after - expected).abs() < 1e-6,
            "expected drain of {expected}"
        );
    }

    #[test]
    fn social_decays_faster_for_social_cats() {
        let (mut world, mut schedule) = setup_world();

        let mut high_social = default_personality();
        high_social.sociability = 1.0;

        let mut low_social = default_personality();
        low_social.sociability = 0.0;

        let cat_social = spawn_cat(&mut world, Needs::default(), high_social);
        let cat_loner = spawn_cat(&mut world, Needs::default(), low_social);

        schedule.run(&mut world);

        let social_after = world.get::<Needs>(cat_social).unwrap().social;
        let loner_after = world.get::<Needs>(cat_loner).unwrap().social;

        assert!(
            social_after < loner_after,
            "highly social cat should lose more social need; social={social_after}, loner={loner_after}"
        );
    }

    #[test]
    fn safety_recovers_each_tick() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.safety = 0.5;
        let entity = spawn_cat(&mut world, needs, default_personality());

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        let recovery = SimConstants::default().needs.safety_recovery_rate;
        assert!(after > 0.5, "safety should recover toward 1.0; got {after}");
        assert!(
            (after - (0.5 + recovery)).abs() < 1e-6,
            "expected recovery of {recovery}"
        );
    }

    #[test]
    fn safety_clamped_at_one() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        assert_eq!(after, 1.0, "safety should not exceed 1.0; got {after}");
    }

    #[test]
    fn needs_do_not_go_below_zero() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;
        needs.energy = 0.0;
        needs.warmth = 0.0;
        needs.social = 0.0;
        needs.acceptance = 0.0;
        needs.respect = 0.0;
        needs.mastery = 0.0;
        needs.purpose = 0.0;

        let entity = spawn_cat(&mut world, needs, default_personality());
        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, 0.0);
        assert_eq!(n.energy, 0.0);
        assert_eq!(n.warmth, 0.0);
        assert_eq!(n.social, 0.0);
        assert_eq!(n.acceptance, 0.0);
        assert_eq!(n.respect, 0.0);
        assert_eq!(n.mastery, 0.0);
        assert_eq!(n.purpose, 0.0);
    }

    #[test]
    fn warmth_drains_extra_in_snow() {
        let (mut world_clear, mut schedule_clear) = setup_world();
        let cat_clear = spawn_cat(&mut world_clear, Needs::default(), default_personality());
        schedule_clear.run(&mut world_clear);
        let warmth_clear = world_clear.get::<Needs>(cat_clear).unwrap().warmth;

        let (mut world_snow, mut schedule_snow) = setup_world();
        world_snow.insert_resource(WeatherState {
            current: Weather::Snow,
            ticks_until_change: 50,
        });
        let cat_snow = spawn_cat(&mut world_snow, Needs::default(), default_personality());
        schedule_snow.run(&mut world_snow);
        let warmth_snow = world_snow.get::<Needs>(cat_snow).unwrap().warmth;

        assert!(
            warmth_snow < warmth_clear,
            "snow should drain warmth faster; snow={warmth_snow}, clear={warmth_clear}"
        );
    }

    #[test]
    fn starvation_drains_health() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0; // starving

        let entity = spawn_cat(&mut world, needs, default_personality());

        let health_before = world.get::<Health>(entity).unwrap().current;
        schedule.run(&mut world);
        let health_after = world.get::<Health>(entity).unwrap().current;

        assert!(
            health_after < health_before,
            "starvation should drain health; before={health_before}, after={health_after}"
        );
        let expected = SimConstants::default().needs.starvation_health_drain;
        assert!(
            (health_before - health_after - expected).abs() < 1e-6,
            "expected {expected} health drain per tick"
        );
    }

    #[test]
    fn starvation_drops_safety() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;
        needs.safety = 0.5;

        let entity = spawn_cat(&mut world, needs, default_personality());

        schedule.run(&mut world);
        let safety = world.get::<Needs>(entity).unwrap().safety;

        // Starvation drains 0.005; no passive recovery when starving.
        assert!(safety < 0.5, "starvation should drain safety; got {safety}");
    }

    #[test]
    fn starvation_doubles_social_decay() {
        let (mut world_fed, mut schedule_fed) = setup_world();
        let fed = spawn_cat(&mut world_fed, Needs::default(), default_personality());
        schedule_fed.run(&mut world_fed);
        let social_fed = world_fed.get::<Needs>(fed).unwrap().social;

        let (mut world_starving, mut schedule_starving) = setup_world();
        let mut starving_needs = Needs::default();
        starving_needs.hunger = 0.0;
        let starving = spawn_cat(&mut world_starving, starving_needs, default_personality());
        schedule_starving.run(&mut world_starving);
        let social_starving = world_starving.get::<Needs>(starving).unwrap().social;

        assert!(
            social_starving < social_fed,
            "starving cat should lose social faster; starving={social_starving}, fed={social_fed}"
        );
    }

    #[test]
    fn starvation_applies_mood_penalty() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;

        let entity = spawn_cat(&mut world, needs, default_personality());
        schedule.run(&mut world);

        let mood = world.get::<Mood>(entity).unwrap();
        let has_starvation_modifier = mood.modifiers.iter().any(|m| m.source == "starvation");
        assert!(
            has_starvation_modifier,
            "starving cat should have starvation mood modifier"
        );
    }

    // --- Personality modifier tests ---

    #[test]
    fn pride_amplifies_respect_drain_when_low() {
        let (mut world_proud, mut sched_proud) = setup_world();
        let (mut world_humble, mut sched_humble) = setup_world();

        let mut needs = Needs::default();
        needs.respect = 0.2; // below 0.4 threshold

        let proud = Personality {
            pride: 1.0,
            ..default_personality()
        };
        let humble = Personality {
            pride: 0.0,
            ..default_personality()
        };

        let cat_proud = spawn_cat(&mut world_proud, needs.clone(), proud);
        let cat_humble = spawn_cat(&mut world_humble, needs, humble);

        sched_proud.run(&mut world_proud);
        sched_humble.run(&mut world_humble);

        let resp_proud = world_proud.get::<Needs>(cat_proud).unwrap().respect;
        let resp_humble = world_humble.get::<Needs>(cat_humble).unwrap().respect;

        assert!(
            resp_proud < resp_humble,
            "proud cat should lose respect faster; proud={resp_proud}, humble={resp_humble}"
        );
    }

    #[test]
    fn pride_no_amplification_when_respect_high() {
        let (mut world_proud, mut sched_proud) = setup_world();
        let (mut world_humble, mut sched_humble) = setup_world();

        let mut needs = Needs::default();
        needs.respect = 0.6; // above 0.4 threshold

        let proud = Personality {
            pride: 1.0,
            ..default_personality()
        };
        let humble = Personality {
            pride: 0.0,
            ..default_personality()
        };

        let cat_proud = spawn_cat(&mut world_proud, needs.clone(), proud);
        let cat_humble = spawn_cat(&mut world_humble, needs, humble);

        sched_proud.run(&mut world_proud);
        sched_humble.run(&mut world_humble);

        let resp_proud = world_proud.get::<Needs>(cat_proud).unwrap().respect;
        let resp_humble = world_humble.get::<Needs>(cat_humble).unwrap().respect;

        // Both should drain at the same rate (pride doesn't amplify above 0.4).
        assert!(
            (resp_proud - resp_humble).abs() < 1e-6,
            "pride should not amplify when respect > 0.4; proud={resp_proud}, humble={resp_humble}"
        );
    }

    #[test]
    fn patience_slows_purpose_drain() {
        let (mut world_patient, mut sched_patient) = setup_world();
        let (mut world_impatient, mut sched_impatient) = setup_world();

        let patient = Personality {
            patience: 1.0,
            ..default_personality()
        };
        let impatient = Personality {
            patience: 0.0,
            ..default_personality()
        };

        let cat_patient = spawn_cat(&mut world_patient, Needs::default(), patient);
        let cat_impatient = spawn_cat(&mut world_impatient, Needs::default(), impatient);

        sched_patient.run(&mut world_patient);
        sched_impatient.run(&mut world_impatient);

        let purpose_patient = world_patient.get::<Needs>(cat_patient).unwrap().purpose;
        let purpose_impatient = world_impatient.get::<Needs>(cat_impatient).unwrap().purpose;

        assert!(
            purpose_patient > purpose_impatient,
            "patient cat should retain more purpose; patient={purpose_patient}, impatient={purpose_impatient}"
        );
    }

    #[test]
    fn independence_increases_purpose_drain() {
        let (mut world_ind, mut sched_ind) = setup_world();
        let (mut world_dep, mut sched_dep) = setup_world();

        let independent = Personality {
            independence: 1.0,
            ..default_personality()
        };
        let dependent = Personality {
            independence: 0.0,
            ..default_personality()
        };

        let cat_ind = spawn_cat(&mut world_ind, Needs::default(), independent);
        let cat_dep = spawn_cat(&mut world_dep, Needs::default(), dependent);

        sched_ind.run(&mut world_ind);
        sched_dep.run(&mut world_dep);

        let purpose_ind = world_ind.get::<Needs>(cat_ind).unwrap().purpose;
        let purpose_dep = world_dep.get::<Needs>(cat_dep).unwrap().purpose;

        assert!(
            purpose_ind < purpose_dep,
            "independent cat should drain purpose faster; ind={purpose_ind}, dep={purpose_dep}"
        );
    }

    #[test]
    fn tradition_familiar_territory_boosts_safety() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.safety = 0.8;

        let traditional = Personality {
            tradition: 1.0,
            ..default_personality()
        };
        let mut prefs = LocationPreferences::default();
        prefs.record_success(5, 5, crate::ai::Action::Hunt); // Familiar at (5,5)

        let entity = world
            .spawn((
                needs,
                traditional,
                Health::default(),
                Mood::default(),
                Position::new(5, 5),
                prefs,
                Age::new(0),
                Orientation::Straight,
            ))
            .id();

        schedule.run(&mut world);

        let safety = world.get::<Needs>(entity).unwrap().safety;
        let nc = &SimConstants::default().needs;
        // Safety normally recovers by safety_recovery_rate. With tradition bonus: +tradition_safety_boost extra.
        let expected_min = 0.8 + nc.safety_recovery_rate + nc.tradition_safety_boost - 1e-6;
        assert!(
            safety > expected_min,
            "traditional cat near familiar territory should get safety boost; got {safety}"
        );
    }
}
