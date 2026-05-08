//! 176 Drop resolver — `Action::Drop` / `DispositionKind::Discarding`,
//! plus 231 means-to-end DropItem-as-prefix when sequenced inside a
//! pickup-class plan.
//!
//! Releases one carried item from a cat's inventory onto the ground at
//! the cat's current position. The dropped item becomes a real `Item`
//! entity with `ItemLocation::OnGround`; another cat can later plan
//! `Action::PickUp` to retrieve it.

use bevy_ecs::prelude::*;

use crate::components::disposition::DispositionKind;
use crate::components::item_transfer::transfer_item_inventory_to_ground;
use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::steps::{StepOutcome, StepResult};

/// Witness emitted on a successful drop. Carries the spawned ground-
/// item entity so the caller can record `Feature::ItemDropped` and
/// thread the entity into any focal-trace observability surface.
#[derive(Debug, Clone, Copy)]
pub struct DropOutcome {
    pub item_entity: Entity,
}

/// 231: drop priority for a slot's `ItemKind` given the cat's current
/// state and elected disposition. Lower score = more droppable.
///
/// Composed of:
/// - **Static base** by item class — curios cheapest, food/herbs most
///   kept.
/// - **Goal modifier** — a cat about to acquire X values its current X
///   less ("about to get more"); a cat about to USE X values it more.
/// - **State modifier** — critical hunger lifts food; an active
///   construction site lifts build materials; the cat already carrying
///   remedy herbs (potential-healer state) lifts herbs.
///
/// Reads only existing perception inputs (no new perception axis
/// authored), per the single-axis-perception-scalars doctrine.
/// `hunger_satiation` is `Needs::hunger` (1.0 sated, 0.0 starving).
fn drop_priority(
    kind: ItemKind,
    disposition: DispositionKind,
    hunger_satiation: f32,
    has_construction_site: bool,
    has_remedy_herbs: bool,
) -> f32 {
    let base = match kind {
        ItemKind::ShinyPebble | ItemKind::GlassShard | ItemKind::ColorfulShell => 0.05,
        ItemKind::Wood | ItemKind::Stone => 0.30,
        k if k.is_herb() => 0.50,
        k if k.is_food() => 0.50,
        _ => 0.40,
    };

    let goal = match disposition {
        // Going to acquire food; current food less critical.
        DispositionKind::Hunting | DispositionKind::Foraging => {
            if kind.is_food() {
                -0.20
            } else {
                0.0
            }
        }
        // Going to use materials.
        DispositionKind::Building => {
            if matches!(kind, ItemKind::Wood | ItemKind::Stone) {
                0.40
            } else {
                0.0
            }
        }
        // Going to feed dependents.
        DispositionKind::Caretaking => {
            if kind.is_food() {
                0.40
            } else {
                0.0
            }
        }
        // Going to use herbs.
        DispositionKind::Herbalism => {
            if kind.is_herb() {
                0.40
            } else {
                0.0
            }
        }
        _ => 0.0,
    };

    let state = {
        let mut m = 0.0;
        if kind.is_food() && hunger_satiation < 0.3 {
            m += 0.30;
        }
        if matches!(kind, ItemKind::Wood | ItemKind::Stone) && has_construction_site {
            m += 0.20;
        }
        if kind.is_herb() && has_remedy_herbs {
            m += 0.20;
        }
        m
    };

    base + goal + state
}

/// # GOAP step resolver: `DropItem`
///
/// **Real-world effect** — spawns one `Item` entity at `cat_pos` with
/// `ItemLocation::OnGround` and removes the chosen slot from the cat's
/// `Inventory`. The drop is instant on entry; if the cat has nothing
/// to drop the step Fails. Slot selection is goal-aware: drops the
/// lowest-priority slot under the cat's current goal + state (231).
///
/// **Plan-level preconditions** — emitted with no zone gate by
/// `src/ai/planner/actions.rs::discarding_actions` (terminal disposal),
/// or as a means-to-end prefix in pickup-class plans (ticket 231). The
/// Discarding disposition is at-position, no travel.
///
/// **Runtime preconditions** — at least one slot must be present in
/// `inventory`. Empty inventories cause a `Fail`; any slot kind is
/// droppable (the unified-pool `Inventory` makes herbs and items
/// indistinguishable for capacity purposes).
///
/// **Witness** — `StepOutcome<Option<DropOutcome>>`. `Some(outcome)`
/// on `StepResult::Advance` carries the spawned ground-item entity.
/// `None` on `Fail` (empty inventory).
///
/// **Feature emission** — caller passes `Feature::ItemDropped`
/// (Neutral) to `record_if_witnessed`.
pub fn resolve_drop_item(
    inventory: &mut Inventory,
    cat_pos: Position,
    disposition: DispositionKind,
    hunger_satiation: f32,
    has_construction_site: bool,
    commands: &mut Commands,
) -> StepOutcome<Option<DropOutcome>> {
    if inventory.slots.is_empty() {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "drop: empty inventory".to_string(),
        ));
    }

    let has_remedy_herbs = inventory.has_remedy_herb();
    let Some((slot_idx, _)) = inventory
        .slots
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            drop_priority(
                a.kind,
                disposition,
                hunger_satiation,
                has_construction_site,
                has_remedy_herbs,
            )
            .total_cmp(&drop_priority(
                b.kind,
                disposition,
                hunger_satiation,
                has_construction_site,
                has_remedy_herbs,
            ))
        })
    else {
        // Empty case is handled above; this is unreachable, but keep a
        // belt-and-suspenders Fail path rather than panicking.
        return StepOutcome::unwitnessed(StepResult::Fail(
            "drop: empty inventory".to_string(),
        ));
    };

    match transfer_item_inventory_to_ground(inventory, slot_idx, cat_pos, commands) {
        Ok(item_entity) => {
            StepOutcome::witnessed_with(StepResult::Advance, DropOutcome { item_entity })
        }
        // The ground primitive cannot fail on capacity; surface as
        // Fail so the caller sees a concrete reason if it ever does.
        Err(_) => StepOutcome::unwitnessed(StepResult::Fail(
            "drop: transfer-to-ground primitive refused".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pebble_drops_before_food_for_hunting_cat() {
        // Cat with [ShinyPebble, RawMouse], hunger high (low satiation),
        // electing Hunt → drops ShinyPebble (base 0.05) over food
        // (base 0.5 + state 0.3 - goal 0.2 = 0.6).
        let pebble_score = drop_priority(
            ItemKind::ShinyPebble,
            DispositionKind::Hunting,
            0.2, // hungry
            false,
            false,
        );
        let food_score = drop_priority(
            ItemKind::RawMouse,
            DispositionKind::Hunting,
            0.2,
            false,
            false,
        );
        assert!(
            pebble_score < food_score,
            "pebble {} should drop before food {} for hungry hunting cat",
            pebble_score,
            food_score
        );
    }

    #[test]
    fn pebble_drops_before_wood_when_building() {
        // Cat with [Wood, ShinyPebble], has_construction_site=true,
        // electing Building → drops ShinyPebble over Wood. Wood gets
        // both the goal lift (+0.4) and the state lift (+0.2).
        let pebble_score = drop_priority(
            ItemKind::ShinyPebble,
            DispositionKind::Building,
            1.0,
            true,
            false,
        );
        let wood_score = drop_priority(
            ItemKind::Wood,
            DispositionKind::Building,
            1.0,
            true,
            false,
        );
        assert!(
            pebble_score < wood_score,
            "pebble {} should drop before wood {} for building cat with site",
            pebble_score,
            wood_score
        );
    }

    #[test]
    fn herb_kept_when_remedy_potential() {
        // Cat with [HerbHealingMoss, ShinyPebble], has_remedy_herbs=true
        // (this cat IS the potential healer). Herb stays; pebble drops.
        let pebble_score = drop_priority(
            ItemKind::ShinyPebble,
            DispositionKind::Hunting,
            1.0,
            false,
            true,
        );
        let herb_score = drop_priority(
            ItemKind::HerbHealingMoss,
            DispositionKind::Hunting,
            1.0,
            false,
            true,
        );
        assert!(
            pebble_score < herb_score,
            "pebble {} should drop before herb {} when cat has remedy potential",
            pebble_score,
            herb_score
        );
    }

    #[test]
    fn hungry_cat_keeps_food_over_curio() {
        // Hungry cat, no Hunt goal — food still keeps over curio.
        let pebble_score = drop_priority(
            ItemKind::ShinyPebble,
            DispositionKind::Resting,
            0.1,
            false,
            false,
        );
        let food_score = drop_priority(
            ItemKind::RawMouse,
            DispositionKind::Resting,
            0.1,
            false,
            false,
        );
        assert!(
            pebble_score < food_score,
            "pebble {} should drop before food {} when hungry",
            pebble_score,
            food_score
        );
    }

    #[test]
    fn hunting_cat_drops_food_over_pebble_only_when_well_fed() {
        // Sated cat electing Hunt with [ShinyPebble, Berries] — food
        // gets the goal -0.2 modifier with no hunger lift, dropping it
        // to 0.30. Pebble at 0.05 still drops first. Documented as v1
        // behavior — A* doesn't see the priority, so cost stays uniform.
        let pebble_score = drop_priority(
            ItemKind::ShinyPebble,
            DispositionKind::Hunting,
            1.0,
            false,
            false,
        );
        let berries_score = drop_priority(
            ItemKind::Berries,
            DispositionKind::Hunting,
            1.0,
            false,
            false,
        );
        assert!(pebble_score < berries_score);
    }
}
