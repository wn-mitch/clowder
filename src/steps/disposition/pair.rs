use bevy_ecs::prelude::Entity;

use crate::components::physical::Position;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{DispositionConstants, SocialConstants};
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Pair`
///
/// **Real-world effect** — when `cat_entity` and `target_entity` are
/// within `social.pairing_proximity_threshold` Manhattan tiles, bumps
/// `relationships[cat, target].romantic` by `social.pairing_romantic_rate`
/// (clamped to 1.0). This is the §7.M.1 L2 PairingActivity primitive:
/// active courtship accumulates romantic attachment ~3× faster than
/// the passive `check_bonds` drift, but only fires per-tick when the
/// pair is actually colocated. Cats that wander apart lose the bonus
/// until they re-converge — the geometry is load-bearing.
///
/// **Plan-level preconditions** — emitted by `build_pairing_chain`
/// after a `MoveTo(target_pos)` step. The chain shape is always
/// `[MoveTo(target_pos), Pair]`; the planner guarantees `MoveTo`
/// completes (cat is within Manhattan range 1 of `target_pos`)
/// before `Pair` runs, but does **not** guarantee the target stayed
/// put — see runtime preconditions.
///
/// **Runtime preconditions** — requires `target_entity` and
/// `target_position` to be `Some`. If either is missing (chain built
/// without a target, or target despawned), returns
/// `StepOutcome::unwitnessed(Advance)` so the chain advances and the
/// disposition replans next tick. If the target is no longer adjacent
/// (Manhattan > `pairing_proximity_threshold`), returns
/// `StepOutcome::unwitnessed(Advance)` — the chain advances, the cat
/// reassesses, and the §7.2 OpenMinded commitment can either re-pick
/// the same partner (re-arming a fresh `MoveTo` + `Pair` pair) or
/// drop on desire drift. Continues for up to `d.pair_duration` ticks
/// while adjacent.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the cat was adjacent
/// to a real target this call AND the romantic bump was applied.
///
/// **Feature emission** — caller passes `Feature::CourtshipInteraction`
/// (Positive) to `record_if_witnessed`. Same Feature variant the §Bug-1
/// passive drift in `social.rs::check_bonds` emits — both feed the
/// `continuity_tallies.courtship` canary, so the canary measures
/// "any courtship-related relationship event" without distinguishing
/// active vs. passive sources.
#[allow(clippy::too_many_arguments)]
pub fn resolve_pairing(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    target_position: Option<Position>,
    cat_position: Position,
    relationships: &mut Relationships,
    social: &SocialConstants,
    d: &DispositionConstants,
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Some(target_pos) = target_position else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    let dist = cat_position.manhattan_distance(&target_pos);
    if dist > social.pairing_proximity_threshold {
        // Target moved out of adjacency mid-session. Drop back to the
        // chain dispatcher; an OpenMinded re-evaluation either picks
        // the same partner and re-arms `MoveTo + Pair`, or drops on
        // desire drift.
        return StepOutcome::unwitnessed(StepResult::Advance);
    }

    relationships.modify_romantic(cat_entity, target, social.pairing_romantic_rate);

    let result = if ticks >= d.pair_duration {
        StepResult::Advance
    } else {
        StepResult::Continue
    };

    StepOutcome::witnessed(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_constants() -> (SocialConstants, DispositionConstants) {
        let sc = crate::resources::sim_constants::SimConstants::default();
        (sc.social.clone(), sc.disposition.clone())
    }

    fn cat_target() -> (Entity, Entity) {
        (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        )
    }

    #[test]
    fn pairing_when_adjacent_witnesses_and_bumps_romantic() {
        let (social, disp) = test_constants();
        let (cat, target) = cat_target();
        let mut relationships = Relationships::default();
        let initial = relationships.get_or_insert(cat, target).romantic;

        let outcome = resolve_pairing(
            0,
            cat,
            Some(target),
            Some(Position::new(1, 0)),
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );

        assert!(outcome.witness, "adjacent pair should witness");
        let after = relationships.get_or_insert(cat, target).romantic;
        let expected = initial + social.pairing_romantic_rate;
        assert!(
            (after - expected).abs() < 1e-6,
            "expected romantic={expected}, got {after}"
        );
    }

    #[test]
    fn pairing_when_non_adjacent_advances_unwitnessed() {
        let (social, disp) = test_constants();
        let (cat, target) = cat_target();
        let mut relationships = Relationships::default();
        let initial = relationships.get_or_insert(cat, target).romantic;

        let outcome = resolve_pairing(
            0,
            cat,
            Some(target),
            Some(Position::new(5, 5)),
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );

        assert!(!outcome.witness, "non-adjacent pair must not witness");
        assert!(matches!(outcome.result, StepResult::Advance));
        let after = relationships.get_or_insert(cat, target).romantic;
        assert!(
            (after - initial).abs() < f32::EPSILON,
            "romantic must not change when out of range"
        );
    }

    #[test]
    fn pairing_with_missing_target_advances_unwitnessed() {
        let (social, disp) = test_constants();
        let (cat, _) = cat_target();
        let mut relationships = Relationships::default();

        let outcome = resolve_pairing(
            0,
            cat,
            None,
            None,
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );

        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
    }

    #[test]
    fn pairing_continues_until_duration_elapses() {
        let (social, disp) = test_constants();
        let (cat, target) = cat_target();
        let mut relationships = Relationships::default();

        let mid = resolve_pairing(
            disp.pair_duration / 2,
            cat,
            Some(target),
            Some(Position::new(1, 0)),
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );
        assert!(
            matches!(mid.result, StepResult::Continue),
            "should Continue mid-session while adjacent"
        );

        let end = resolve_pairing(
            disp.pair_duration,
            cat,
            Some(target),
            Some(Position::new(1, 0)),
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );
        assert!(
            matches!(end.result, StepResult::Advance),
            "should Advance once duration elapses"
        );
    }

    #[test]
    fn pairing_romantic_saturates_at_one() {
        let (social, disp) = test_constants();
        let (cat, target) = cat_target();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, target).romantic = 0.999;

        let _ = resolve_pairing(
            0,
            cat,
            Some(target),
            Some(Position::new(1, 0)),
            Position::new(0, 0),
            &mut relationships,
            &social,
            &disp,
        );

        let after = relationships.get_or_insert(cat, target).romantic;
        assert!(
            after <= 1.0,
            "romantic must clamp at 1.0; got {after}"
        );
    }
}
