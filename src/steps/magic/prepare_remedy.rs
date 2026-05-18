use crate::components::magic::{Inventory, RemedyKind};
use crate::components::skills::Skills;
use crate::resources::sim_constants::MagicConstants;
use crate::resources::time::TimeScale;
use crate::steps::StepResult;

/// # GOAP step resolver: `PrepareRemedy`
///
/// **Real-world effect** — consumes one herb of the required
/// kind from the actor's inventory; deposits a prepared
/// `ItemKind::Remedy*` (matching `remedy.to_item_kind()`) into
/// the same inventory; grows herbcraft skill. Ticket 365 (016
/// Phase 1a) replaced the prior virtual `Carrying::Remedy` carry
/// with a real inventory slot — `Carrying::from_inventory` now
/// projects to `Carrying::Remedy` based on this slot. Because
/// every herb removal vacates the slot the remedy is added to,
/// the slot count is invariant across one prep and `add_item`
/// can never reject due to fullness.
///
/// **Plan-level preconditions** — emitted by herbcraft planner;
/// `at_workshop` parameter controls the tick budget. The
/// planner-side effect `SetCarrying(Carrying::Remedy)` mirrors
/// the runtime inventory transition for in-plan A* expansion.
///
/// **Runtime preconditions** — `inventory.take_herb(required)`
/// must succeed or Fail("missing herb for remedy"). No silent-
/// advance surface.
///
/// **Witness** — returns plain `StepResult`. Predates the
/// `StepOutcome<W>` convention. Success is implicit in Advance
/// (the `take_herb` + `add_item` calls only succeed on Advance
/// path).
///
/// **Feature emission** — caller records `Feature::RemedyPrepared`
/// (Positive) on Advance at `src/systems/goap.rs` (alongside
/// `Feature::RemedyApplied` for the downstream apply step).
pub fn resolve_prepare_remedy(
    ticks: u64,
    remedy: RemedyKind,
    at_workshop: bool,
    inventory: &mut Inventory,
    skills: &mut Skills,
    m: &MagicConstants,
    time_scale: &TimeScale,
) -> StepResult {
    let required_ticks = if at_workshop {
        m.prepare_remedy_duration_workshop.ticks(time_scale)
    } else {
        m.prepare_remedy_duration_default.ticks(time_scale)
    };
    if ticks >= required_ticks {
        let herb_needed = remedy.required_herb();
        if inventory.take_herb(herb_needed) {
            // Slot-count invariant: take_herb vacated exactly one
            // slot; add_item below fills exactly one. is_full check
            // is unnecessary — capacity stays the same across the
            // pair. Returning Fail without re-adding the herb would
            // leak it.
            inventory.add_item(remedy.to_item_kind());
            skills.herbcraft += skills.growth_rate() * m.herbcraft_prepare_skill_growth;
            StepResult::Advance
        } else {
            StepResult::Fail("missing herb for remedy".into())
        }
    } else {
        StepResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::magic::HerbKind;

    fn time_scale() -> TimeScale {
        TimeScale::from_config(&crate::resources::time::SimConfig::default(), 16.6667)
    }

    #[test]
    fn prepare_remedy_consumes_herb_and_adds_remedy_item() {
        let mut inventory = Inventory::default();
        inventory.add_herb(HerbKind::HealingMoss);
        let mut skills = Skills::default();
        let m = MagicConstants::default();
        let ts = time_scale();

        let required = m.prepare_remedy_duration_workshop.ticks(&ts);
        let result = resolve_prepare_remedy(
            required,
            RemedyKind::HealingPoultice,
            true,
            &mut inventory,
            &mut skills,
            &m,
            &ts,
        );

        assert!(matches!(result, StepResult::Advance));
        assert!(!inventory.has_herb(HerbKind::HealingMoss));
        assert!(inventory.has_remedy(RemedyKind::HealingPoultice));
        assert_eq!(inventory.slots.len(), 1);
    }

    #[test]
    fn prepare_remedy_fails_without_required_herb() {
        let mut inventory = Inventory::default();
        // Wrong herb in inventory.
        inventory.add_herb(HerbKind::Thornbriar);
        let mut skills = Skills::default();
        let m = MagicConstants::default();
        let ts = time_scale();

        let required = m.prepare_remedy_duration_default.ticks(&ts);
        let result = resolve_prepare_remedy(
            required,
            RemedyKind::HealingPoultice,
            false,
            &mut inventory,
            &mut skills,
            &m,
            &ts,
        );

        assert!(matches!(result, StepResult::Fail(_)));
        assert!(inventory.has_herb(HerbKind::Thornbriar));
        assert!(!inventory.has_remedy(RemedyKind::HealingPoultice));
    }

    #[test]
    fn prepare_remedy_continues_before_required_ticks() {
        let mut inventory = Inventory::default();
        inventory.add_herb(HerbKind::HealingMoss);
        let mut skills = Skills::default();
        let m = MagicConstants::default();
        let ts = time_scale();

        let result = resolve_prepare_remedy(
            0,
            RemedyKind::HealingPoultice,
            true,
            &mut inventory,
            &mut skills,
            &m,
            &ts,
        );

        assert!(matches!(result, StepResult::Continue));
        // Herb still present, no remedy yet.
        assert!(inventory.has_herb(HerbKind::HealingMoss));
        assert!(!inventory.has_remedy(RemedyKind::HealingPoultice));
    }
}
