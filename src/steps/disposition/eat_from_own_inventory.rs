use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::mental::{Mood, MoodModifier, MoodSource};
use crate::components::physical::Needs;
use crate::steps::{StepOutcome, StepResult};

/// Witness for the Eat-from-own-inventory Sink — names the food that
/// was consumed and the hunger restoration that resulted. Used by the
/// dispatcher to fire the canary Feature, and reserved for any future
/// caller that wants to credit the cat with a per-tick mood reaction
/// to the meal in a post-loop pass.
#[derive(Debug, Clone, Copy)]
pub struct EatFromOwnInventoryOutcome {
    pub kind: ItemKind,
    pub satiation_delta: f32,
}

/// # GOAP step resolver: `EatFromOwnInventory` (Sink)
///
/// **Real-world effect** — drains one food slot from the actor's
/// `Inventory` (via `take_food`) and credits `Needs.hunger` by
/// `kind.food_value() * freshness` (capped at 1.0). When the consumed
/// item carries `modifiers.from_organ` AND its kind is one of the
/// organ-derived variants (`RawOrgan` / `PreservedOrgan`), pushes a
/// time-limited `MoodModifier` worth `organ_mood_bonus` onto the
/// cat's mood stack (mirrors the pre-429 organ-bump behavior from
/// 367 Commit 6).
///
/// **Plan-level preconditions** — none today; the per-tick dispatcher
/// at `src/systems/needs.rs::eat_from_inventory` calls this resolver
/// only when `needs.hunger < eat_from_inventory_threshold`. (A future
/// follow-on to 429 will plumb this Sink as a `StepKind::EatFromOwnInventory`
/// GOAP step gated on `HasMarker(HasFoodInInventory::KEY)`, swapping
/// the autonomic-tier reflex for L2/L3 election in adults.)
///
/// **Runtime preconditions** — `inventory.take_food()` must return
/// `Some`; if the inventory has no food slot, the resolver returns
/// `StepOutcome::unwitnessed(Advance)` and no Feature fires.
///
/// **Witness** — `StepOutcome<Option<EatFromOwnInventoryOutcome>>`.
/// `Some(outcome)` carries the eaten `kind` and `satiation_delta`
/// (the realized hunger restoration, useful for trace or per-cat
/// follow-up systems); `None` when no food was found.
///
/// **Feature emission** — caller passes `Feature::EatFromOwnInventory`
/// (Positive, enrolled in the seed-42 canary) to `record_if_witnessed`.
pub fn resolve_eat_from_own_inventory(
    needs: &mut Needs,
    inventory: &mut Inventory,
    corruption_food_penalty: f32,
    mood: Option<&mut Mood>,
    organ_mood_bonus: f32,
) -> StepOutcome<Option<EatFromOwnInventoryOutcome>> {
    let Some((kind, modifiers)) = inventory.take_food() else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let freshness = 1.0 - modifiers.corruption * corruption_food_penalty;
    let prior = needs.hunger;
    needs.hunger = (needs.hunger + kind.food_value() * freshness).min(1.0);
    let satiation_delta = needs.hunger - prior;

    // 367 Commit 6 organ-mood bump, preserved verbatim — bounded
    // `MoodModifier` (`ORGAN_MOOD_BONUS_DURATION_TICKS` ≈ one day-phase)
    // tagged `MoodSource::Physical`. Dual-gate (`from_organ` modifier
    // plus organ kind) guards against spurious firing if the modifier
    // ever leaks onto non-organ items.
    if modifiers.from_organ && matches!(kind, ItemKind::RawOrgan | ItemKind::PreservedOrgan) {
        if let Some(mood) = mood {
            mood.modifiers.push_back(
                MoodModifier::new(
                    organ_mood_bonus,
                    ORGAN_MOOD_BONUS_DURATION_TICKS,
                    "ate organ meat",
                )
                .with_kind(MoodSource::Physical),
            );
        }
    }

    StepOutcome::witnessed_with(
        StepResult::Advance,
        EatFromOwnInventoryOutcome {
            kind,
            satiation_delta,
        },
    )
}

/// 367 Commit 6 — wall-clock duration for the organ-mood bump. 400
/// ticks ≈ one day-phase at canonical SimConfig; long enough that the
/// cat feels the lift across the immediate post-meal window, short
/// enough that it doesn't bleed into the next sleep cycle. Local
/// constant rather than a CraftingConstants knob because the duration
/// is opinionated narrative pacing — the *amount* (`organ_mood_bonus`)
/// is the load-bearing knob.
const ORGAN_MOOD_BONUS_DURATION_TICKS: u64 = 400;
