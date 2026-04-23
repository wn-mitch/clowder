use bevy_ecs::prelude::*;

use crate::components::magic::Inventory;
use crate::components::physical::Needs;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `FeedKitten`
///
/// **Real-world effect** — transfers one food item from the adult's
/// `Inventory` (`take_food`) to the target kitten; credits the
/// adult's `needs.social` by 0.05 (belonging-tier reward for the
/// caretaking act, regardless of whether food actually transferred).
/// The kitten-side hunger restoration (0.5) is applied by the
/// caller in a post-loop pass, since `&mut Needs` on kittens
/// conflicts with the cats query.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(Stores)` + `CarryingIs(Carrying::RawFood)` by
/// `src/ai/planner/actions.rs::caretaking_actions`. The
/// `RetrieveFoodForKitten` predecessor step (Phase 4c.4) is what
/// actually puts food in the adult's real `Inventory`; without it
/// `take_food()` would return `None` and the witness would stay
/// `None` — which was the pre-4c.3 silent-failure.
///
/// **Runtime preconditions** — waits `ticks >= 10` (Continue until
/// then). Requires `target_kitten` to be `Some` and
/// `adult_inventory.take_food()` to return `Some`. If either is
/// missing, returns `StepOutcome::unwitnessed(Advance)`: the chain
/// advances but no Feature fires. The chain's `AbortChain` policy
/// drops the plan on the miss, so the adult retries on the next
/// Caretake firing.
///
/// **Witness** — `StepOutcome<Option<Entity>>`. `Some(kitten_entity)`
/// iff food was consumed AND target_kitten was present — i.e. the
/// caller should apply the 0.5 hunger restoration to that kitten in
/// its deferred post-loop pass.
///
/// **Feature emission** — caller passes `Feature::KittenFed`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_feed_kitten(
    ticks: u64,
    target_kitten: Option<Entity>,
    adult_needs: &mut Needs,
    adult_inventory: &mut Inventory,
) -> StepOutcome<Option<Entity>> {
    if ticks < 10 {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    // Adult's own social bonus for the caretaking act: preserved from
    // the pre-4c.3 implementation — caring for kittens is a belonging-
    // tier reward regardless of feeding success.
    adult_needs.social = (adult_needs.social + 0.05).min(1.0);

    let Some(kitten_entity) = target_kitten else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    if adult_inventory.take_food().is_some() {
        StepOutcome::witnessed_with(StepResult::Advance, kitten_entity)
    } else {
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}
