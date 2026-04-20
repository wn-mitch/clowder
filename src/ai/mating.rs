//! Mating eligibility — the hard gate used by both `evaluate_dispositions`
//! (legacy path) and `evaluate_and_plan` (GOAP path) to decide whether a cat
//! should even consider the Mate action.
//!
//! Extracted from inline checks because the gate has to live in two systems
//! that must stay in lockstep (see `CLAUDE.md` §Headless Mode). Placing the
//! rules here keeps the two call sites textually identical.

use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::identity::{Age, Gender, LifeStage, Orientation};
use crate::components::mental::Mood;
use crate::components::physical::{Dead, Needs, Position};
use crate::components::pregnancy::Pregnant;
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::time::{DayPhase, Season};
use crate::systems::social::are_orientation_compatible;

/// Per-cat snapshot of every field used by the mating gate.
///
/// Built once at the top of the evaluate system, then looked up per-entity
/// without re-running queries. Matches the snapshot pattern used elsewhere
/// in `goap.rs` / `disposition.rs` to stay under Bevy's 16-param limit.
#[derive(Clone, Copy, Debug)]
pub struct MatingFitness {
    pub stage: LifeStage,
    pub gender: Gender,
    pub orientation: Orientation,
    pub mood_valence: f32,
    pub hunger: f32,
    pub energy: f32,
    pub is_pregnant: bool,
}

/// SystemParam bundle gathering everything the mating gate needs beyond what
/// the calling system already holds. Bundling the Query + TimeState + SimConfig
/// here keeps the caller's parameter count low (disposition.rs already sits at
/// Bevy's 16-param limit; see `CLAUDE.md` §ECS Rules).
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct MatingFitnessParams<'w, 's> {
    pub query: Query<
        'w,
        's,
        (
            Entity,
            &'static Age,
            &'static Gender,
            &'static Orientation,
            &'static Mood,
            &'static Needs,
            Option<&'static Pregnant>,
        ),
        Without<Dead>,
    >,
    pub time: Res<'w, crate::resources::time::TimeState>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
}

impl<'w, 's> MatingFitnessParams<'w, 's> {
    /// Snapshot the entire fertile-cat population into a lookup table.
    pub fn snapshot(&self) -> HashMap<Entity, MatingFitness> {
        let tick = self.time.tick;
        let tps = self.config.ticks_per_season;
        self.query
            .iter()
            .map(|(e, age, gender, orient, mood, needs, pregnant)| {
                (
                    e,
                    MatingFitness {
                        stage: age.stage(tick, tps),
                        gender: *gender,
                        orientation: *orient,
                        mood_valence: mood.valence,
                        hunger: needs.hunger,
                        energy: needs.energy,
                        is_pregnant: pregnant.is_some(),
                    },
                )
            })
            .collect()
    }

    /// The current season, computed from the bundle's time + config.
    pub fn current_season(&self) -> Season {
        self.time.season(&self.config)
    }

    /// The current day phase, computed from the bundle's time + config. Lives
    /// here (rather than as an ad-hoc `Res<TimeState>` + `Res<SimConfig>` pair
    /// on each caller) for the same 16-param-budget reason `current_season`
    /// does — both `evaluate_and_plan` and `evaluate_dispositions` already
    /// thread this bundle through.
    pub fn current_day_phase(&self) -> DayPhase {
        DayPhase::from_tick(self.time.tick, &self.config)
    }
}

/// Per-cat fertility: adult+, non-asexual, not pregnant.
fn is_fertile(f: &MatingFitness) -> bool {
    matches!(f.stage, LifeStage::Adult | LifeStage::Elder)
        && f.orientation != Orientation::Asexual
        && !f.is_pregnant
}

/// Does this cat (by fitness snapshot) meet the "sated and happy" floor?
fn is_sated_and_happy(f: &MatingFitness, scoring: &ScoringConstants) -> bool {
    f.hunger > scoring.breeding_hunger_floor
        && f.energy > scoring.breeding_energy_floor
        && f.mood_valence > scoring.breeding_mood_floor
}

/// The full eligibility gate for the Mate action.
///
/// Returns true iff:
///   - the current season has non-zero fertility (photoperiodic window —
///     Spring peak, Summer secondary, Autumn tail, Winter anestrous by default),
///   - self is fertile + sated + happy + past the interest threshold,
///   - at least one nearby cat with a Partners/Mates bond also passes fertile
///     + sated + happy and is orientation-compatible with self.
#[allow(clippy::too_many_arguments)]
pub fn has_eligible_mate(
    self_entity: Entity,
    self_mating_need: f32,
    season: Season,
    scoring: &ScoringConstants,
    fitness: &HashMap<Entity, MatingFitness>,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
) -> bool {
    if scoring.season_fertility(season) <= 0.0 {
        return false;
    }
    if self_mating_need >= scoring.mating_interest_threshold {
        return false;
    }
    let Some(self_fit) = fitness.get(&self_entity) else {
        return false;
    };
    if !is_fertile(self_fit) || !is_sated_and_happy(self_fit, scoring) {
        return false;
    }

    cat_positions.iter().any(|(other, _)| {
        if *other == self_entity {
            return false;
        }
        let Some(other_fit) = fitness.get(other) else {
            return false;
        };
        if !is_fertile(other_fit) || !is_sated_and_happy(other_fit, scoring) {
            return false;
        }
        if !are_orientation_compatible(
            self_fit.gender,
            self_fit.orientation,
            other_fit.gender,
            other_fit.orientation,
        ) {
            return false;
        }
        relationships
            .get(self_entity, *other)
            .is_some_and(|r| matches!(r.bond, Some(BondType::Partners) | Some(BondType::Mates)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_fitness() -> MatingFitness {
        MatingFitness {
            stage: LifeStage::Adult,
            gender: Gender::Queen,
            orientation: Orientation::Straight,
            mood_valence: 0.5,
            hunger: 0.9,
            energy: 0.9,
            is_pregnant: false,
        }
    }

    fn setup_eligible_pair() -> (
        Entity,
        Entity,
        HashMap<Entity, MatingFitness>,
        Relationships,
        Vec<(Entity, Position)>,
    ) {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut fitness = HashMap::new();
        fitness.insert(a, default_fitness());
        fitness.insert(
            b,
            MatingFitness {
                gender: Gender::Tom,
                ..default_fitness()
            },
        );

        let mut relationships = Relationships::default();
        let rel = relationships.get_or_insert(a, b);
        rel.bond = Some(BondType::Partners);

        let positions = vec![(a, Position::new(0, 0)), (b, Position::new(1, 0))];
        (a, b, fitness, relationships, positions)
    }

    #[test]
    fn eligible_when_all_gates_pass_in_spring() {
        let scoring = ScoringConstants::default();
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn eligible_in_summer_secondary_peak() {
        let scoring = ScoringConstants::default();
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(has_eligible_mate(
            a,
            0.3,
            Season::Summer,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn eligible_in_autumn_tail() {
        let scoring = ScoringConstants::default();
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(has_eligible_mate(
            a,
            0.3,
            Season::Autumn,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_in_winter_anestrous() {
        let scoring = ScoringConstants::default();
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Winter,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_configured_spring_fertility_is_zero() {
        let mut scoring = ScoringConstants::default();
        scoring.mating_fertility_spring = 0.0;
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_hungry() {
        let scoring = ScoringConstants::default();
        let (a, _, mut fitness, relationships, positions) = setup_eligible_pair();
        fitness.get_mut(&a).unwrap().hunger = 0.3; // below floor

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_sad() {
        let scoring = ScoringConstants::default();
        let (a, _, mut fitness, relationships, positions) = setup_eligible_pair();
        fitness.get_mut(&a).unwrap().mood_valence = 0.0; // below floor (0.2)

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_partner_is_pregnant() {
        let scoring = ScoringConstants::default();
        let (a, b, mut fitness, relationships, positions) = setup_eligible_pair();
        fitness.get_mut(&b).unwrap().is_pregnant = true;

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_mating_need_too_high() {
        let scoring = ScoringConstants::default();
        let (a, _, fitness, relationships, positions) = setup_eligible_pair();

        assert!(!has_eligible_mate(
            a,
            0.95, // above mating_interest_threshold (0.6)
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_without_partners_bond() {
        let scoring = ScoringConstants::default();
        let (a, b, fitness, mut relationships, positions) = setup_eligible_pair();
        relationships.get_or_insert(a, b).bond = Some(BondType::Friends);

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }

    #[test]
    fn ineligible_when_orientation_incompatible() {
        let scoring = ScoringConstants::default();
        let (a, b, mut fitness, relationships, positions) = setup_eligible_pair();
        // Make both straight Toms — not compatible.
        fitness.get_mut(&a).unwrap().gender = Gender::Tom;
        assert_eq!(fitness.get(&b).unwrap().gender, Gender::Tom);

        assert!(!has_eligible_mate(
            a,
            0.3,
            Season::Spring,
            &scoring,
            &fitness,
            &positions,
            &relationships
        ));
    }
}
