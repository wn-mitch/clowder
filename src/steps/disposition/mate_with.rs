use bevy_ecs::prelude::*;

use crate::components::identity::Gender;
use crate::components::physical::Needs;
use crate::resources::relationships::Relationships;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `MateWith`
///
/// **Real-world effect** — clears `needs.mating` to 1.0, boosts
/// `needs.social` + the romantic-axis relationship delta with the
/// partner. On successful gestation-side pairing, yields a
/// `(gestator_entity, litter_size)` payload the caller uses to
/// insert a `Pregnant` component (the resolver can't do this
/// itself — it'd need a separate `&mut` on the gestator).
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::SocialTarget)` by
/// `src/ai/planner/actions.rs::mating_actions`. Partner selection
/// runs in `src/ai/dses/mate_target.rs`; `ZoneIs` alone does not
/// guarantee a compatible partner.
///
/// **Runtime preconditions** — waits `ticks >= 10` (Continue
/// until then). Requires `target_entity` to be `Some`. When a
/// target is present, the step always runs the needs + romantic
/// side-effects: this gives both Tom×Tom ("courtship without
/// gestation") and Queen×Tom ("successful mating") the same
/// clear-mating-need effect — otherwise Tom×Tom would re-score
/// Mate forever.
///
/// **Witness** — `StepOutcome<Option<(Entity, u8)>>`.
/// `Some((gestator, litter_size))` iff at least one partner could
/// gestate (Queen or Nonbinary on either side). `None` for
/// Tom×Tom or missing target — those do NOT fire `MatingOccurred`
/// because no pregnancy was inserted.
///
/// **Feature emission** — caller passes `Feature::MatingOccurred`
/// (Positive) to `record_if_witnessed`. Separately: when the
/// witness is `None` AND `target_entity.is_some()`, the caller
/// records `Feature::CourtshipInteraction` (Positive) directly —
/// a Tom×Tom bonding still happened, just without gestation.
/// This second signal is the only currently-documented bypass of
/// the `record_if_witnessed` helper pattern (§Phase 5a), since
/// the resolver's witness type can't distinguish "no target"
/// from "target but no gestation" without a richer payload.
///
/// §7.M.7.4 gender rule: if both partners can gestate
/// (Queen×Queen, Queen×Nonbinary, Nonbinary×Nonbinary), the
/// initiator wins the tie; if exactly one can, that one is the
/// gestator regardless of initiator; if neither can, witness is
/// `None`.
pub fn resolve_mate_with(
    ticks: u64,
    cat_entity: Entity,
    cat_gender: Gender,
    target_entity: Option<Entity>,
    target_gender: Option<Gender>,
    needs: &mut Needs,
    relationships: &mut Relationships,
) -> StepOutcome<Option<(Entity, u8)>> {
    if ticks < 10 {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    let Some(partner) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let partner_gender = target_gender.unwrap_or(cat_gender);

    let gestator = match (cat_gender.can_gestate(), partner_gender.can_gestate()) {
        (true, _) => Some(cat_entity),
        (false, true) => Some(partner),
        (false, false) => None,
    };

    // Side-effects run whenever a partner is present: clear mating,
    // nudge social + romantic. Tom×Tom pairs still need the mating
    // need to clear, otherwise the initiator re-scores Mate forever.
    needs.mating = 1.0;
    needs.social = (needs.social + 0.15).min(1.0);
    relationships.modify_romantic(cat_entity, partner, 0.1);

    let pregnancy = gestator.map(|g| {
        let mut litter_size: u8 = 1;
        if needs.hunger > 0.7 {
            litter_size += 1;
        }
        litter_size = litter_size.min(3);
        (g, litter_size)
    });

    match pregnancy {
        Some(p) => StepOutcome::witnessed_with(StepResult::Advance, p),
        None => StepOutcome::unwitnessed(StepResult::Advance),
    }
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
        let outcome = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert_eq!(outcome.witness.map(|(e, _)| e), Some(initiator));
    }

    #[test]
    fn tom_initiator_queen_partner_pregnates_queen() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_mate_with(
            10,
            initiator,
            Gender::Tom,
            Some(partner),
            Some(Gender::Queen),
            &mut needs,
            &mut rel,
        );
        assert_eq!(outcome.witness.map(|(e, _)| e), Some(partner));
    }

    #[test]
    fn both_gestators_initiator_wins_tie() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Nonbinary),
            &mut needs,
            &mut rel,
        );
        assert_eq!(outcome.witness.map(|(e, _)| e), Some(initiator));
    }

    #[test]
    fn tom_tom_returns_none_but_advances() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_mate_with(
            10,
            initiator,
            Gender::Tom,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness.is_none());
        // Mating need still clears so the step isn't re-scored forever.
        assert!((needs.mating - 1.0).abs() < 1e-6);
    }

    #[test]
    fn continues_before_tick_threshold() {
        let mut needs = fresh_needs();
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_mate_with(
            5,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn hunger_over_threshold_increments_litter_size() {
        let mut needs = fresh_needs();
        needs.hunger = 0.9;
        let mut rel = fresh_relationships();
        let initiator = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_mate_with(
            10,
            initiator,
            Gender::Queen,
            Some(partner),
            Some(Gender::Tom),
            &mut needs,
            &mut rel,
        );
        assert_eq!(outcome.witness.map(|(_, n)| n), Some(2));
    }
}
