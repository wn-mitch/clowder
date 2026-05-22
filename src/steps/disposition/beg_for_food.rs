//! Ticket 450 — `BegForFood` step resolver. Pairs with the
//! [`beg_for_food` DSE](crate::ai::dses::beg_for_food) and the
//! [`Begging`](crate::components::disposition::DispositionKind::Begging)
//! disposition.

use bevy_ecs::prelude::*;

use crate::components::physical::Position;
use crate::steps::{StepOutcome, StepResult};

/// Number of ticks one begging cycle holds before the planner step
/// `Advance`s. Short — kittens cry out in short bursts, not sustained
/// wails. The cadence comes from re-election (`target_completions = 1`):
/// each beg cycle completes, the Begging disposition retires, the cat
/// re-elects on the next tick. If `(NewbornKitten | EyesOpenKitten) ∧
/// ¬HasFoodInInventory ∧ hungry` still holds, the BegForFoodDse wins
/// again and the next cycle begins.
///
/// Kept as a module-local const (not a `SimConstants` knob) because
/// it's a substrate-shape value rather than a balance lever — tuning
/// the cycle length doesn't move colony-level metrics in any
/// directional way. If a future ticket needs to vary it, promote here.
pub const BEG_FOR_FOOD_TICKS: u64 = 5;

/// Witness emitted on each completed beg cycle. Carries the kitten
/// entity + position + hunger reading so post-loop drains (and trace
/// emission) can correlate the cry signal with the kitten's state at
/// emit-time. Hunger is the urgency input — a starving kitten cries
/// louder than a peckish one — and the field is preserved here for
/// downstream tools (focal trace, narrative, post-soak inspection)
/// even though the cry-map stamping itself reads `Needs.hunger`
/// directly from the kittens query in
/// `growth::update_kitten_cry_map`.
#[derive(Debug, Clone, Copy)]
pub struct BegEmitted {
    pub kitten: Entity,
    pub position: Position,
    pub hunger: f32,
}

/// # GOAP step resolver: `BegForFood`
///
/// **Real-world effect** — on cycle completion (`ticks >=
/// BEG_FOR_FOOD_TICKS`), emits the canary witness that drives
/// `Feature::KittenBegged`. The kitten's audible signal to nearby
/// adults is materialized by the `growth::update_kitten_cry_map`
/// system, which on each tick clears the cry-map, reads kittens whose
/// `CurrentAction.action == Action::BegForFood` (i.e. the Begging
/// disposition is elected this tick), and re-stamps the
/// `KittenCryMap` plus authors the `IsParentOfHungryKitten` marker on
/// each kitten's parents. So the substrate state change "the cry is
/// audible from N tiles away" happens via the system sweep keyed off
/// the L3 election; the resolver's role is the canary signal that
/// records each beg cycle for the activation footer + focal trace.
/// (This split avoids dual-emission: only the system stamps the map,
/// only the resolver fires the Feature.)
///
/// **Plan-level preconditions** — emitted under the empty
/// `preconditions: vec![]` template by
/// `src/ai/planner/actions.rs::begging_actions`. The DSE-side
/// eligibility filter (`require(NewbornKitten | EyesOpenKitten),
/// forbid(HasFoodInInventory)`) is what gates the disposition; no
/// runtime predicate needed at the planner layer.
///
/// **Runtime preconditions** — none beyond the wait. Returns
/// `Continue` until `ticks >= BEG_FOR_FOOD_TICKS`, then `Advance` with
/// a `BegEmitted` witness carrying the kitten, position, and hunger
/// reading. The witness fires unconditionally on completion: the
/// kitten is always known (they are the actor) and their position +
/// hunger are always readable from the caller's query, so there is no
/// "advance unwitnessed" path. This shape is intentional — any
/// Advance must record `Feature::KittenBegged` so the seed-42 canary
/// catches silent kitten dispositions.
///
/// **Witness** — `StepOutcome<Option<BegEmitted>>`. `Some` on every
/// completion tick; `None` only while still waiting (`Continue`).
///
/// **Feature emission** — caller passes `Feature::KittenBegged`
/// (Positive, `expected_to_fire_per_soak() => true`) to
/// `record_if_witnessed`. Any seed-42 soak with at least one Stage 1
/// or Stage 2 kitten reaching `hunger < kitten_cry_hunger_threshold`
/// must witness ≥1 emission, otherwise the never-fired canary fails.
pub fn resolve_beg_for_food(
    ticks: u64,
    kitten: Entity,
    position: Position,
    hunger: f32,
) -> StepOutcome<Option<BegEmitted>> {
    if ticks < BEG_FOR_FOOD_TICKS {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    StepOutcome::witnessed_with(
        StepResult::Advance,
        BegEmitted {
            kitten,
            position,
            hunger,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beg_continues_during_wait() {
        let outcome = resolve_beg_for_food(
            0,
            Entity::from_raw_u32(7).unwrap(),
            Position::new(3, 3),
            0.2,
        );
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn beg_advances_with_witness_on_cycle_completion() {
        let kitten = Entity::from_raw_u32(7).unwrap();
        let outcome = resolve_beg_for_food(BEG_FOR_FOOD_TICKS, kitten, Position::new(3, 3), 0.2);
        assert!(matches!(outcome.result, StepResult::Advance));
        let beg = outcome.witness.expect("witness should fire on advance");
        assert_eq!(beg.kitten, kitten);
    }

    #[test]
    fn beg_witness_records_hunger() {
        let kitten = Entity::from_raw_u32(7).unwrap();
        let outcome = resolve_beg_for_food(BEG_FOR_FOOD_TICKS, kitten, Position::new(3, 3), 0.15);
        let beg = outcome.witness.expect("witness fires");
        assert!((beg.hunger - 0.15).abs() < 1e-6);
    }
}
