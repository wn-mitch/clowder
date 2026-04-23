use crate::components::magic::{Inventory, ItemSlot};
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Cook`
///
/// **Real-world effect** — flips the `cooked` flag on the first raw food
/// item in the cat's inventory, granting a hunger-value multiplier when
/// the item is later eaten.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::CarryingIs(Carrying::RawFood)` by
/// `src/ai/planner/actions.rs`. `CarryingIs` is a coarse abstraction
/// over the richer runtime `Inventory`; planner state can drift from
/// reality (e.g. the cat may have foraged additional non-food items
/// between planning and execution), so the resolver re-scans the real
/// inventory.
///
/// **Runtime preconditions** — reads `Inventory::slots` for the first
/// `ItemSlot::Item(kind, modifiers)` where `kind.is_food()` and
/// `!modifiers.cooked`. If no such slot exists the outcome is
/// `unwitnessed(Advance)` — the plan still advances (the planner's
/// `CarryingIs(RawFood)` gate cannot distinguish cooked from raw, and
/// idling the cat at the kitchen is worse than advancing and letting
/// the next eat step pull whatever's there).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff exactly one item was
/// flipped to cooked this call.
///
/// **Feature emission** — caller passes `Feature::FoodCooked` (Positive)
/// to `record_if_witnessed`.
pub fn resolve_cook(
    ticks: u64,
    inventory: &mut Inventory,
    d: &DispositionConstants,
) -> StepOutcome<bool> {
    if ticks < d.cook_ticks {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    for slot in inventory.slots.iter_mut() {
        if let ItemSlot::Item(kind, modifiers) = slot {
            if kind.is_food() && !modifiers.cooked {
                modifiers.cooked = true;
                return StepOutcome::witnessed(StepResult::Advance);
            }
        }
    }
    StepOutcome::unwitnessed(StepResult::Advance)
}
