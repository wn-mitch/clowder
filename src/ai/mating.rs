//! Mating eligibility — the hard gate used by both `evaluate_dispositions`
//! (legacy path) and `evaluate_and_plan` (GOAP path) to decide whether a cat
//! should even consider the Mate action.
//!
//! Extracted from inline checks because the gate has to live in two systems
//! that must stay in lockstep (see `CLAUDE.md` §Headless Mode). Placing the
//! rules here keeps the two call sites textually identical.

use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::fertility::{Fertility, FertilityPhase};
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
    /// Current `Fertility.phase` if the cat carries the component
    /// (Queen / Nonbinary in Adult stage, not pregnant). `None` for
    /// Toms and for any cat pre-/post-Fertility-marker (Kitten, Young,
    /// Elder, pregnant). §7.M.7.6 hard-gate reads this field.
    pub fertility_phase: Option<FertilityPhase>,
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
            Option<&'static Fertility>,
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
            .map(
                |(e, age, gender, orient, mood, needs, pregnant, fertility)| {
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
                            fertility_phase: fertility.map(|f| f.phase),
                        },
                    )
                },
            )
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

/// §7.M.7.6 viability: is this cat a gestation-capable partner in a
/// phase that can conceive? Toms are always non-viable here — they
/// contribute to a pair via their partner's viability (§7.M.7.5
/// fallback), not their own.
fn is_conception_viable(f: &MatingFitness) -> bool {
    if matches!(f.gender, Gender::Tom) {
        return false;
    }
    f.fertility_phase
        .is_some_and(FertilityPhase::is_viable_for_conception)
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
        // §7.M.7.6 hard gate: at least one partner must be gestation-
        // capable (Queen or Nonbinary) with Fertility phase ∉
        // {Anestrus, Postpartum}. Tom×Tom fails unconditionally.
        // Queen×Tom / Queen×Queen requires at least one non-Tom's
        // Fertility phase to pass `is_viable_for_conception`.
        if !is_conception_viable(self_fit) && !is_conception_viable(other_fit) {
            return false;
        }
        relationships
            .get(self_entity, *other)
            .is_some_and(|r| matches!(r.bond, Some(BondType::Partners) | Some(BondType::Mates)))
    })
}

/// Per-tick author for the §4.3 `HasEligibleMate` ZST. Wraps the
/// [`has_eligible_mate`] predicate so `MateDse::eligibility()` can
/// `.require(HasEligibleMate::KEY)` instead of carrying a lifted
/// outer gate at `scoring.rs:916` (ticket 027 Bug 2 — same
/// silent-divergence pattern CLAUDE.md flags from the §7.2 regression).
///
/// Idempotent: insert/remove the ZST only on transitions, so steady-
/// state ticks are write-free.
pub fn update_mate_eligibility_markers(
    mut commands: Commands,
    mating: MatingFitnessParams,
    relationships: Res<Relationships>,
    constants: Res<crate::resources::sim_constants::SimConstants>,
    cats: Query<
        (
            Entity,
            &Position,
            &Needs,
            Has<crate::components::markers::HasEligibleMate>,
        ),
        Without<Dead>,
    >,
) {
    use crate::components::markers::HasEligibleMate;
    let fitness = mating.snapshot();
    let season = mating.current_season();
    let scoring = &constants.scoring;
    let cat_positions: Vec<(Entity, Position)> =
        cats.iter().map(|(e, p, _, _)| (e, *p)).collect();

    for (entity, _pos, needs, has_marker) in cats.iter() {
        let eligible = has_eligible_mate(
            entity,
            needs.mating,
            season,
            scoring,
            &fitness,
            &cat_positions,
            &relationships,
        );
        match (eligible, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasEligibleMate);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasEligibleMate>();
            }
            _ => {}
        }
    }
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
            // Default to Estrus so the §7.M.7.6 viability gate opens —
            // tests that exercise the gate override this explicitly.
            fertility_phase: Some(FertilityPhase::Estrus),
        }
    }

    #[allow(clippy::type_complexity)]
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
                // §7.M.7.4: Toms never carry Fertility.
                fertility_phase: None,
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
    fn ineligible_when_no_partner_is_conception_viable() {
        // §7.M.7.6 hard gate: with both partners in Anestrus or
        // Postpartum, the pair cannot conceive — gate rejects.
        let scoring = ScoringConstants::default();
        let (a, b, mut fitness, relationships, positions) = setup_eligible_pair();
        // Partner b is a Tom per `setup_eligible_pair` — make self (a)
        // Anestrus. Neither partner is now conception-viable.
        fitness.get_mut(&a).unwrap().fertility_phase = Some(FertilityPhase::Anestrus);
        // Tom already has no Fertility marker (contributes nothing here).
        fitness.get_mut(&b).unwrap().fertility_phase = None;

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
    fn eligible_when_queen_is_in_estrus_with_tom_partner() {
        // §7.M.7.6 hard gate opens on a single viable Queen + Tom pair.
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
    fn ineligible_when_queen_is_postpartum() {
        // Post-birth Postpartum phase pins receptivity to 0 — gate
        // must reject the pair even though the Tom partner is viable
        // in a seasonal sense.
        let scoring = ScoringConstants::default();
        let (a, _, mut fitness, relationships, positions) = setup_eligible_pair();
        fitness.get_mut(&a).unwrap().fertility_phase = Some(FertilityPhase::Postpartum);
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

    // -----------------------------------------------------------------------
    // Ticket 027 Bug 2: update_mate_eligibility_markers (author system)
    // -----------------------------------------------------------------------

    use crate::components::identity::Name;
    use crate::components::markers::HasEligibleMate;
    use crate::components::physical::{DeathCause, Health};
    use bevy_ecs::schedule::Schedule;

    /// Spawn an Adult cat that satisfies all per-cat fertility +
    /// sated-and-happy gates. Caller composes a partner bond + season
    /// in the `Relationships` resource and `SimConstants` season fertility.
    fn spawn_eligible_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
        position: Position,
    ) -> Entity {
        let mut needs = Needs::default();
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.mating = 0.3; // below mating_interest_threshold (default 0.6)
        let mut mood = Mood::default();
        // Default Mood::valence = 0.2, which is == the breeding_mood_floor;
        // the predicate uses strict `>`, so bump above the floor.
        mood.valence = 0.5;
        let fertility = if matches!(gender, Gender::Tom) {
            None
        } else {
            Some(Fertility {
                phase: FertilityPhase::Estrus,
                cycle_offset: 0,
                post_partum_remaining_ticks: 0,
            })
        };
        let mut entity = world.spawn((
            Name(name.to_string()),
            Age { born_tick: 0 },
            gender,
            orientation,
            mood,
            needs,
            position,
            Health::default(),
        ));
        if let Some(f) = fertility {
            entity.insert(f);
        }
        entity.id()
    }

    fn run_author(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(update_mate_eligibility_markers);
        schedule.run(world);
    }

    fn marker_world() -> World {
        let mut world = World::new();
        // Age cats to Adult: tick > ticks_per_season * 12 with the
        // default 20_000 tps.
        let mut time = crate::resources::time::TimeState::default();
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(Relationships::default());
        world
    }

    #[test]
    fn update_marker_inserts_for_eligible_partners_pair_in_spring() {
        let mut world = marker_world();
        // Default season for tick 20_000 * 13 must land in Spring; force
        // it explicitly by setting season_started_tick.
        world.resource_mut::<crate::resources::time::TimeState>().tick = 20_000 * 12 + 5;

        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        // Bond them as Partners so `has_eligible_mate` accepts them.
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Partners);

        run_author(&mut world);

        assert!(
            world.get::<HasEligibleMate>(a).is_some(),
            "Queen should be marked HasEligibleMate when bonded to fertile Tom in Spring"
        );
        assert!(
            world.get::<HasEligibleMate>(b).is_some(),
            "Tom partner should also receive the marker (predicate is symmetric)"
        );
    }

    #[test]
    fn update_marker_skips_unbonded_compatible_pair() {
        let mut world = marker_world();
        world.resource_mut::<crate::resources::time::TimeState>().tick = 20_000 * 12 + 5;

        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let _b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        // No Partners bond — relationships left empty.

        run_author(&mut world);

        assert!(
            world.get::<HasEligibleMate>(a).is_none(),
            "no marker without a Partners/Mates bond"
        );
    }

    #[test]
    fn update_marker_removes_when_partner_dies() {
        let mut world = marker_world();
        world.resource_mut::<crate::resources::time::TimeState>().tick = 20_000 * 12 + 5;

        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = Some(BondType::Partners);

        // First tick — partner alive, marker should land on both.
        run_author(&mut world);
        assert!(world.get::<HasEligibleMate>(a).is_some());

        // Mark partner Dead — the `Without<Dead>` query filter drops
        // them from the candidate pool, so the predicate fails.
        world.entity_mut(b).insert(Dead {
            tick: 0,
            cause: DeathCause::OldAge,
        });

        run_author(&mut world);
        assert!(
            world.get::<HasEligibleMate>(a).is_none(),
            "marker should be removed once the partner is filtered out by Without<Dead>"
        );
    }
}
