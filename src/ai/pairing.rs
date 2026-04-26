//! L2 PairingActivity gating — the marker + author system that
//! `pairing_activity_dse` uses for eligibility.
//!
//! The §7.M three-layer Mating model splits into L1 ReproduceAspiration
//! (always-on for reproductive cats), L2 PairingActivity (sustained
//! courtship of a Friends-bonded compatible partner), and L3
//! MateWithGoal (single mating event requiring `Partners`+ bond + a
//! seasonal fertility window).
//!
//! L2's gate is intentionally looser than L3's:
//!
//! - Bond filter is `Friends` (not `Partners`/`Mates`) — that's the
//!   whole point of L2: escalate Friends → Partners by holding the
//!   pair colocated. `HasEligibleMate` (L3) and `HasPairingCandidate`
//!   (L2) are orthogonal gates on different bond tiers; a cat can
//!   carry both, neither, or one.
//! - No season check. Cats build romantic attraction year-round; only
//!   the actual mating event (L3) is photoperiodically gated.
//! - No sated/happy check. Courtship in mild hunger is fine — the
//!   §7.5 anxiety preempt drops Pairing if needs cross critical
//!   thresholds.
//! - No conception-viability check (Postpartum, Anestrus). Courtship
//!   continues through these phases; the L3 hard gate at
//!   `MateDse.eligibility().require(HasEligibleMate)` stops actual
//!   mating events.
//!
//! The author system mirrors `mating.rs::update_mate_eligibility_markers`
//! exactly — same `Has<HasPairingCandidate>` query pattern, same
//! idempotent insert/remove on transitions, same per-tick cadence in
//! the marker-author chain.

use bevy_ecs::prelude::*;

use crate::ai::dses::pairing_activity_target::PAIRING_TARGET_RANGE;
use crate::ai::mating::{MatingFitness, MatingFitnessParams};
use crate::components::fertility::FertilityPhase;
use crate::components::identity::{Gender, LifeStage, Orientation};
use crate::components::physical::{Dead, Position};
use crate::resources::relationships::{BondType, Relationships};
use crate::systems::social::are_orientation_compatible;
use std::collections::HashMap;

/// Per-cat predicate: does an orientation-compatible Friends-bonded
/// candidate exist within `PAIRING_TARGET_RANGE` Manhattan tiles?
///
/// Returns true iff:
/// - self is Adult or Elder,
/// - self is non-asexual and not pregnant (carries `Fertility` if
///   Queen/Nonbinary; Toms always pass the gender-side check),
/// - at least one nearby cat has a `Friends` bond with self, is
///   Adult/Elder, non-asexual, not pregnant, and is orientation-
///   compatible with self.
///
/// Pregnancy on either side blocks pairing — a pregnant cat is in
/// caretaking mode (§7.M.7), not courtship. `Postpartum` / `Anestrus`
/// fertility phases do **not** block — courtship continues; only L3
/// mating is gated by phase.
pub fn has_pairing_candidate(
    self_entity: Entity,
    fitness: &HashMap<Entity, MatingFitness>,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
) -> bool {
    let Some(self_fit) = fitness.get(&self_entity) else {
        return false;
    };
    if !is_pairing_eligible(self_fit) {
        return false;
    }

    let Some(self_pos) = cat_positions
        .iter()
        .find_map(|(e, p)| (*e == self_entity).then_some(*p))
    else {
        return false;
    };

    cat_positions.iter().any(|(other, other_pos)| {
        if *other == self_entity {
            return false;
        }
        let dist = self_pos.manhattan_distance(other_pos) as f32;
        if dist > PAIRING_TARGET_RANGE {
            return false;
        }
        let Some(other_fit) = fitness.get(other) else {
            return false;
        };
        if !is_pairing_eligible(other_fit) {
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
            .is_some_and(|r| matches!(r.bond, Some(BondType::Friends)))
    })
}

/// Per-cat side of the predicate: Adult/Elder, non-asexual, not
/// pregnant. `Postpartum` / `Anestrus` fertility phases pass — those
/// only matter at L3.
fn is_pairing_eligible(f: &MatingFitness) -> bool {
    if !matches!(f.stage, LifeStage::Adult | LifeStage::Elder) {
        return false;
    }
    if f.orientation == Orientation::Asexual {
        return false;
    }
    if f.is_pregnant {
        return false;
    }
    // Toms have no Fertility marker — they pass the per-cat side. For
    // gestation-capable cats, any non-`None` `fertility_phase` is
    // acceptable at L2 (the L3 hard gate handles `is_viable_for_conception`).
    if matches!(f.gender, Gender::Tom) {
        return true;
    }
    // Queen/Nonbinary must carry Fertility (Young or Elder cats lack
    // it; pregnancy already gated above). Postpartum is permitted.
    matches!(
        f.fertility_phase,
        Some(FertilityPhase::Proestrus)
            | Some(FertilityPhase::Estrus)
            | Some(FertilityPhase::Diestrus)
            | Some(FertilityPhase::Anestrus)
            | Some(FertilityPhase::Postpartum)
    )
}

/// Per-tick author for `HasPairingCandidate`. Mirrors
/// `mating.rs::update_mate_eligibility_markers` — same idempotent
/// transition pattern; steady-state ticks are write-free.
pub fn update_pairing_candidate_markers(
    mut commands: Commands,
    mating: MatingFitnessParams,
    relationships: Res<Relationships>,
    cats: Query<
        (
            Entity,
            &Position,
            Has<crate::components::markers::HasPairingCandidate>,
        ),
        Without<Dead>,
    >,
) {
    use crate::components::markers::HasPairingCandidate;
    let fitness = mating.snapshot();
    let cat_positions: Vec<(Entity, Position)> =
        cats.iter().map(|(e, p, _)| (e, *p)).collect();

    for (entity, _pos, has_marker) in cats.iter() {
        let eligible =
            has_pairing_candidate(entity, &fitness, &cat_positions, &relationships);
        match (eligible, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasPairingCandidate);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasPairingCandidate>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::fertility::Fertility;
    use crate::components::identity::{Age, Name};
    use crate::components::markers::HasPairingCandidate;
    use crate::components::mental::Mood;
    use crate::components::physical::{DeathCause, Health, Needs};
    use bevy_ecs::schedule::Schedule;

    fn marker_world() -> World {
        let mut world = World::new();
        let mut time = crate::resources::time::TimeState::default();
        // Tick > ticks_per_season * 12 places cats firmly in Adult.
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(Relationships::default());
        world
    }

    fn spawn_pair_eligible(
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
        schedule.add_systems(update_pairing_candidate_markers);
        schedule.run(world);
    }

    #[test]
    fn marker_inserted_for_friends_bonded_compatible_pair() {
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(2, 0),
        );
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .bond = Some(BondType::Friends);

        run_author(&mut world);

        assert!(
            world.get::<HasPairingCandidate>(a).is_some(),
            "Queen should be marked HasPairingCandidate when bonded to compat Tom as Friends"
        );
        assert!(
            world.get::<HasPairingCandidate>(b).is_some(),
            "Tom partner should also receive the marker — predicate is symmetric"
        );
    }

    #[test]
    fn marker_skipped_when_unbonded() {
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let _b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(2, 0),
        );
        // No bond — relationships left empty.

        run_author(&mut world);

        assert!(
            world.get::<HasPairingCandidate>(a).is_none(),
            "no marker without a Friends bond"
        );
    }

    #[test]
    fn marker_skipped_when_partner_is_partners_tier() {
        // Partners/Mates routes through MateDse + HasEligibleMate, not
        // through L2 Pairing. The L2 marker must NOT fire when bond is
        // already past Friends.
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(2, 0),
        );
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .bond = Some(BondType::Partners);

        run_author(&mut world);

        assert!(
            world.get::<HasPairingCandidate>(a).is_none(),
            "Partners bond is L3 territory, not L2 — marker must not fire"
        );
        assert!(world.get::<HasPairingCandidate>(b).is_none());
    }

    #[test]
    fn marker_removed_when_partner_dies() {
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(2, 0),
        );
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .bond = Some(BondType::Friends);

        run_author(&mut world);
        assert!(world.get::<HasPairingCandidate>(a).is_some());

        world.entity_mut(b).insert(Dead {
            tick: 0,
            cause: DeathCause::OldAge,
        });

        run_author(&mut world);
        assert!(
            world.get::<HasPairingCandidate>(a).is_none(),
            "marker should clear once the partner is filtered by Without<Dead>"
        );
    }

    #[test]
    fn marker_skipped_when_partner_out_of_range() {
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            // Beyond pairing_target_range (10).
            Position::new(50, 0),
        );
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .bond = Some(BondType::Friends);

        run_author(&mut world);

        assert!(
            world.get::<HasPairingCandidate>(a).is_none(),
            "candidate beyond pairing_target_range should not arm the marker"
        );
    }

    #[test]
    fn marker_skipped_when_orientation_incompatible() {
        let mut world = marker_world();
        let a = spawn_pair_eligible(
            &mut world,
            "Fern",
            Gender::Tom,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_pair_eligible(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .bond = Some(BondType::Friends);

        run_author(&mut world);

        assert!(
            world.get::<HasPairingCandidate>(a).is_none(),
            "two straight Toms are not orientation-compatible"
        );
    }
}
