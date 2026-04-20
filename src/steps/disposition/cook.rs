use crate::components::magic::{Inventory, ItemSlot};
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

/// Spend `cook_ticks` at a Kitchen, then flip the first raw food item in the
/// cat's inventory to `cooked = true`. Returns `(result, cooked)` where
/// `cooked` signals that an item was transformed this step (used for event /
/// narrative emission at the call site).
pub fn resolve_cook(
    ticks: u64,
    inventory: &mut Inventory,
    d: &DispositionConstants,
) -> (StepResult, bool) {
    if ticks < d.cook_ticks {
        return (StepResult::Continue, false);
    }
    for slot in inventory.slots.iter_mut() {
        if let ItemSlot::Item(kind, modifiers) = slot {
            if kind.is_food() && !modifiers.cooked {
                modifiers.cooked = true;
                return (StepResult::Advance, true);
            }
        }
    }
    (StepResult::Advance, false)
}
