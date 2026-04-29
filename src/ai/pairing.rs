//! L2 PairingActivity author / drop system — §7.M.
//!
//! Per-tick maintenance of the [`PairingActivity`] Intention component.
//! For each Adult/Elder cat without a `Pregnant` marker:
//!
//! - **If the cat already holds `PairingActivity`**: evaluate
//!   [`should_drop_pairing`] against fresh proxies. On any drop branch,
//!   remove the component and fire `Feature::PairingDropped`.
//! - **Otherwise**: scan within `pairing.candidate_range` for
//!   Friends-or-better bonded, orientation-compatible peers; score each
//!   on the fondness/romantic/bond-tier axis; if the best candidate
//!   clears `pairing.emission_threshold`, insert
//!   `PairingActivity::new(partner, tick)` and fire
//!   `Feature::PairingIntentionEmitted`.
//!
//! Runs every tick (idempotent — re-evaluation against an already-correct
//! state is a no-op). Schedule edge: after
//! [`crate::ai::mating::update_mate_eligibility_markers`] in
//! `crate::plugins::simulation::SimulationPlugin::build` so the snapshot
//! it builds is the same one [`MateDse`]'s eligibility marker reads.
//!
//! **Commit A scope.** This file lands the substrate only. No DSE
//! consumes the Intention yet — the bias readers are wired in Commit B.
//! `Feature::PairingIntentionEmitted` and `Feature::PairingDropped`
//! both fire from this commit, but their `expected_to_fire_per_soak()`
//! flags stay at `false` until Commit B promotes them once the bias
//! mechanism is in place.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::ai::mating::{MatingFitness, MatingFitnessParams};
use crate::components::identity::{LifeStage, Orientation};
use crate::components::markers::{Banished, Incapacitated};
use crate::components::pairing::{
    PairingActivity, PairingDropConfig, PairingProxies, should_drop_pairing,
};
use crate::components::physical::{Dead, Position};
use crate::components::pregnancy::Pregnant;
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, TimeState};
use crate::systems::social::are_orientation_compatible;

/// Map a bond tier to a graduated scalar for the pairing-quality
/// score. Mirrors [`crate::ai::dses::socialize_target::bond_score`]
/// (Friends → 0.5, Partners/Mates → 1.0) so the L2 emission decision
/// uses the same vocabulary the bias readers will use in Commit B.
fn bond_tier_score(bond: Option<BondType>) -> f32 {
    match bond {
        Some(BondType::Mates | BondType::Partners) => 1.0,
        Some(BondType::Friends) => 0.5,
        None => 0.0,
    }
}

/// L1-equivalent reproductive-eligibility predicate. The L1
/// `ReproduceAspiration` aspiration-catalog entry is not yet authored
/// (verified 2026-04-28: no `Reproduce` chain in
/// `assets/narrative/aspirations/*.ron`), so the gate reads the
/// `MatingFitness` snapshot directly. When the catalog entry lands,
/// this can switch to a `With<HasReproduceAspiration>` marker without
/// changing the L2 shape.
fn is_reproductive(f: &MatingFitness) -> bool {
    matches!(f.stage, LifeStage::Adult | LifeStage::Elder)
        && f.orientation != Orientation::Asexual
        && !f.is_pregnant
}

/// Per-tick author/drop system. Idempotent — only insert/remove on
/// transitions; record activation features only when a transition
/// fires.
#[allow(clippy::too_many_arguments)]
pub fn author_pairing_intentions(
    mut commands: Commands,
    mating: MatingFitnessParams,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    config: Res<crate::resources::time::SimConfig>,
    mut activation: ResMut<SystemActivation>,
    cats: Query<(Entity, &Position, Option<&PairingActivity>), Without<Dead>>,
    invalidity: Query<(Has<Dead>, Has<Banished>, Has<Incapacitated>)>,
    pregnant_q: Query<(), With<Pregnant>>,
) {
    let fitness = mating.snapshot();
    let season = time.season(&config);
    let pairing_constants = &constants.pairing;
    let drop_config = PairingDropConfig {
        romantic_floor: pairing_constants.romantic_floor,
        fondness_floor: pairing_constants.fondness_floor,
    };

    let positions: Vec<(Entity, Position)> =
        cats.iter().map(|(e, pos, _)| (e, *pos)).collect();

    for (entity, position, held) in cats.iter() {
        let Some(self_fit) = fitness.get(&entity).copied() else {
            continue;
        };

        if let Some(pairing) = held {
            let drop_outcome = evaluate_drop(
                entity,
                pairing,
                self_fit,
                season,
                &relationships,
                &invalidity,
                &pregnant_q,
                &drop_config,
            );
            if drop_outcome.is_some() {
                commands.entity(entity).remove::<PairingActivity>();
                activation.record(Feature::PairingDropped);
            }
            continue;
        }

        if !is_reproductive(&self_fit) {
            continue;
        }

        let Some(partner) = pick_partner(
            entity,
            position,
            self_fit,
            &positions,
            &fitness,
            &relationships,
            pairing_constants,
        ) else {
            continue;
        };

        commands
            .entity(entity)
            .insert(PairingActivity::new(partner, time.tick));
        activation.record(Feature::PairingIntentionEmitted);
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_drop(
    self_entity: Entity,
    pairing: &PairingActivity,
    self_fit: MatingFitness,
    season: Season,
    relationships: &Relationships,
    invalidity: &Query<(Has<Dead>, Has<Banished>, Has<Incapacitated>)>,
    pregnant_q: &Query<(), With<Pregnant>>,
    drop_config: &PairingDropConfig,
) -> Option<crate::components::pairing::PairingDropBranch> {
    let partner_invalid = match invalidity.get(pairing.partner) {
        Ok((dead, banished, incapacitated)) => dead || banished || incapacitated,
        // Despawned partner -> get errors -> treat as invalid.
        Err(_) => true,
    };
    let bond = relationships
        .get(self_entity, pairing.partner)
        .and_then(|r| r.bond);
    let (romantic, fondness) = relationships
        .get(self_entity, pairing.partner)
        .map(|r| (r.romantic, r.fondness))
        .unwrap_or((0.0, 0.0));
    let self_is_pregnant = pregnant_q.get(self_entity).is_ok() || self_fit.is_pregnant;

    let proxies = PairingProxies {
        self_stage: self_fit.stage,
        self_orientation: self_fit.orientation,
        self_gender: self_fit.gender,
        self_is_pregnant,
        self_fertility_phase: self_fit.fertility_phase,
        partner_invalid,
        bond,
        romantic,
        fondness,
        season,
    };
    should_drop_pairing(&proxies, drop_config)
}

#[allow(clippy::too_many_arguments)]
fn pick_partner(
    self_entity: Entity,
    self_position: &Position,
    self_fit: MatingFitness,
    positions: &[(Entity, Position)],
    fitness: &HashMap<Entity, MatingFitness>,
    relationships: &Relationships,
    pairing_constants: &crate::resources::sim_constants::PairingConstants,
) -> Option<Entity> {
    let range = pairing_constants.candidate_range;
    let weights = (
        pairing_constants.quality_fondness_weight,
        pairing_constants.quality_romantic_weight,
        pairing_constants.quality_bond_weight,
    );

    let mut best: Option<(Entity, f32)> = None;
    for (other, other_pos) in positions.iter() {
        if *other == self_entity {
            continue;
        }
        let manhattan = (self_position.x - other_pos.x).abs()
            + (self_position.y - other_pos.y).abs();
        if manhattan > range {
            continue;
        }
        let Some(other_fit) = fitness.get(other) else {
            continue;
        };
        if !is_reproductive(other_fit) {
            continue;
        }
        if !are_orientation_compatible(
            self_fit.gender,
            self_fit.orientation,
            other_fit.gender,
            other_fit.orientation,
        ) {
            continue;
        }
        let Some(rel) = relationships.get(self_entity, *other) else {
            continue;
        };
        let bond_score = bond_tier_score(rel.bond);
        if bond_score == 0.0 {
            // No bond tier ⇒ not a Friends+ candidate.
            continue;
        }
        // Tom×Tom should already have failed the orientation check;
        // belt-and-suspenders: skip pairs where neither side could
        // ever conceive (matches §7.M.7.6's hard gate read by
        // `mating::has_eligible_mate`). At Friends-bond formation
        // time we don't enforce conception viability — that's L3's
        // gate, not L2's — so we deliberately don't check it here.
        let fondness = rel.fondness.max(0.0);
        let romantic = rel.romantic.max(0.0);
        let score = weights.0 * fondness + weights.1 * romantic + weights.2 * bond_score;
        if score < pairing_constants.emission_threshold {
            continue;
        }
        // Stable tie-break: Entity::index() asc ensures determinism
        // across runs.
        let candidate = (*other, score);
        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some(candidate),
        }
    }
    best.map(|(e, _)| e)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::fertility::{Fertility, FertilityPhase};
    use crate::components::identity::{Age, Gender, Name, Orientation};
    use crate::components::mental::Mood;
    use crate::components::physical::{Health, Needs};
    use crate::resources::SimConstants;
    use crate::resources::time::SimConfig;
    use bevy_ecs::schedule::Schedule;

    /// Spawn an Adult cat with all per-cat fertility / sated-and-happy
    /// gates open. Adapted from `mating::tests::spawn_eligible_adult`.
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
        needs.mating = 0.3;
        let mut mood = Mood::default();
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
        schedule.add_systems(author_pairing_intentions);
        schedule.run(world);
    }

    fn pairing_world() -> World {
        let mut world = World::new();
        let mut time = TimeState::default();
        // Tick > 12 seasons of default tps (20_000) -> Adult life-stage.
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(SimConfig::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(SystemActivation::default());
        world
    }

    #[test]
    fn emits_intention_for_friends_bonded_compatible_pair() {
        let mut world = pairing_world();
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
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_author(&mut world);

        assert!(
            world.get::<PairingActivity>(a).is_some(),
            "Queen should hold a PairingActivity Intention with Reed"
        );
        let pairing_a = world.get::<PairingActivity>(a).unwrap();
        assert_eq!(pairing_a.partner, b);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation.counts.get(&Feature::PairingIntentionEmitted).copied(),
            Some(2),
            "the bond is symmetric -> both cats emit one Pairing each"
        );
    }

    #[test]
    fn does_not_emit_for_unbonded_compatible_pair() {
        let mut world = pairing_world();
        let _a = spawn_eligible_adult(
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
        // No bond.
        run_author(&mut world);

        let mut held = 0;
        for entity in [_a, _b] {
            if world.get::<PairingActivity>(entity).is_some() {
                held += 1;
            }
        }
        assert_eq!(held, 0, "no Friends bond -> no Intention emitted");
    }

    #[test]
    fn does_not_emit_outside_candidate_range() {
        let mut world = pairing_world();
        // Range default = 25; place the partner well outside.
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
            Position::new(50, 50),
        );
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = Some(BondType::Friends);

        run_author(&mut world);

        assert!(world.get::<PairingActivity>(a).is_none());
        assert!(world.get::<PairingActivity>(b).is_none());
    }

    #[test]
    fn does_not_emit_for_pregnant_cat() {
        let mut world = pairing_world();
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
        // Insert Pregnant on a -> MatingFitnessParams's
        // is_reproductive gate filters her out.
        world.entity_mut(a).insert(Pregnant {
            conceived_tick: 0,
            partner: Some(b),
            litter_size: 2,
            stage: crate::components::pregnancy::GestationStage::Early,
            nutrition_sum: 0.0,
            nutrition_samples: 0,
        });
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = Some(BondType::Friends);

        run_author(&mut world);

        assert!(
            world.get::<PairingActivity>(a).is_none(),
            "pregnant cats do not emit new Pairings"
        );
    }

    #[test]
    fn drops_intention_when_partner_dies() {
        let mut world = pairing_world();
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
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_author(&mut world);
        assert!(world.get::<PairingActivity>(a).is_some());

        // Kill partner -> author drops the Intention next tick.
        world.entity_mut(b).insert(Dead {
            tick: 0,
            cause: crate::components::physical::DeathCause::OldAge,
        });
        run_author(&mut world);

        assert!(
            world.get::<PairingActivity>(a).is_none(),
            "Intention dropped when partner becomes Dead"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation.counts.get(&Feature::PairingDropped).copied().unwrap_or(0) >= 1,
            "PairingDropped activation must fire on the drop transition"
        );
    }

    #[test]
    fn drops_intention_when_bond_lost() {
        let mut world = pairing_world();
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
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_author(&mut world);
        assert!(world.get::<PairingActivity>(a).is_some());

        // Drop the bond (defensive — check_bonds only upgrades today,
        // but the L2 system must respect a downgrade).
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = None;
        run_author(&mut world);
        assert!(world.get::<PairingActivity>(a).is_none());
    }

    #[test]
    fn does_not_double_emit_when_already_held() {
        let mut world = pairing_world();
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
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_author(&mut world);
        run_author(&mut world);
        run_author(&mut world);

        let count = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::PairingIntentionEmitted)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            count, 2,
            "first tick emits two (symmetric pair); subsequent ticks must not re-emit"
        );
    }
}
