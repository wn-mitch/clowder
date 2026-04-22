use bevy_ecs::prelude::*;

use crate::components::identity::Gender;
use crate::components::physical::Needs;
use crate::resources::relationships::Relationships;
use crate::steps::StepResult;

/// Resolve a `MateWith` step.
///
/// §7.M.7.4 gender fix: the second return, on successful mating,
/// is `Some((gestator_entity, litter_size))` where `gestator_entity`
/// is the gestation-capable cat that will receive the `Pregnant`
/// component. Selection rule:
///
/// - If both partners can gestate (Queen×Queen, Queen×Nonbinary,
///   Nonbinary×Nonbinary) → initiator wins the tie.
/// - If exactly one can gestate → that cat is the gestator
///   regardless of which side initiated.
/// - If neither can gestate (Tom×Tom) → return `None`; no `Pregnant`
///   is inserted and no `MatingOccurred` event fires.
///
/// Callers are expected to `commands.entity(gestator).insert(Pregnant::...)`
/// with the returned entity rather than using `cat_entity`.
pub fn resolve_mate_with(
    ticks: u64,
    cat_entity: Entity,
    cat_gender: Gender,
    target_entity: Option<Entity>,
    target_gender: Option<Gender>,
    needs: &mut Needs,
    relationships: &mut Relationships,
) -> (StepResult, Option<(Entity, u8)>) {
    if ticks < 10 {
        return (StepResult::Continue, None);
    }
    let Some(partner) = target_entity else {
        return (StepResult::Advance, None);
    };
    let partner_gender = target_gender.unwrap_or(cat_gender);

    let gestator = match (cat_gender.can_gestate(), partner_gender.can_gestate()) {
        (true, _) => Some(cat_entity),
        (false, true) => Some(partner),
        (false, false) => None,
    };

    let pregnancy = gestator.map(|g| {
        let mut litter_size: u8 = 1;
        if needs.hunger > 0.7 {
            litter_size += 1;
        }
        litter_size = litter_size.min(3);

        needs.mating = 1.0;
        needs.social = (needs.social + 0.15).min(1.0);
        relationships.modify_romantic(cat_entity, partner, 0.1);

        (g, litter_size)
    });

    // Tom×Tom: clear mating need so the initiator isn't stuck
    // re-scoring Mate indefinitely, and nudge social/romantic the
    // same way a successful mating would. No `Pregnant` insert.
    if pregnancy.is_none() {
        needs.mating = 1.0;
        needs.social = (needs.social + 0.15).min(1.0);
        relationships.modify_romantic(cat_entity, partner, 0.1);
    }

    (StepResult::Advance, pregnancy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_needs() -> Needs {
        Needs::default()
    }

    fn fresh_relationships() -> Relationships {
        Relationships::default()
    }

    #[test]
    fn queen_initiator_tom_partner_pregnates_queen() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (_, pregnancy) = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert_eq!(pregnancy.map(|(e, _)| e), Some(initiator));
    }

    #[test]
    fn tom_initiator_queen_partner_pregnates_queen() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (_, pregnancy) = resolve_mate_with(
            10,
            initiator,
            Gender::Tom,
            Some(partner),
            Some(Gender::Queen),
            &mut needs,
            &mut rel,
        );
        assert_eq!(pregnancy.map(|(e, _)| e), Some(partner));
    }

    #[test]
    fn both_gestators_initiator_wins_tie() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (_, pregnancy) = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Nonbinary),
            &mut needs,
            &mut rel,
        );
        assert_eq!(pregnancy.map(|(e, _)| e), Some(initiator));
    }

    #[test]
    fn tom_tom_returns_none_but_advances() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (result, pregnancy) = resolve_mate_with(
            10,
            initiator,
            Gender::Tom,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert!(matches!(result, StepResult::Advance));
        assert!(pregnancy.is_none());
        // Mating need still clears so the step isn't re-scored forever.
        assert!((needs.mating - 1.0).abs() < 1e-6);
    }

    #[test]
    fn continues_before_tick_threshold() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (result, pregnancy) = resolve_mate_with(
            5,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert!(matches!(result, StepResult::Continue));
        assert!(pregnancy.is_none());
    }

    #[test]
    fn hunger_over_threshold_increments_litter_size() {
        let mut needs = fresh_needs();
        needs.hunger = 0.9;
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let (_, pregnancy) = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert_eq!(pregnancy.map(|(_, n)| n), Some(2));
    }
}
