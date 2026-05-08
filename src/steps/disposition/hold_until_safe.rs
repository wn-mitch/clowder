use crate::components::physical::Position;
use crate::components::route_cost_field::RouteCostField;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `HoldUntilSafe` (ticket 230)
///
/// **Real-world effect** — none directly. The cat sits on its current
/// tile while the resolver counts ticks toward `flee_hold_ticks`.
/// The trip increment that closes the Fleeing disposition is fired
/// by the planner-level `StateEffect::IncrementTrips` on the action's
/// effects list (see `fleeing_actions()` in `src/ai/planner/actions.rs`),
/// not by the resolver — so the resolver's job is to gate `Advance`
/// on the safety hysteresis, and let the planner-substrate machinery
/// handle the trip count and the post-loop `proxies_for_plan` arm.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::FleeTargetPicked(true)` by `fleeing_actions()`.
/// Effects on success: `IncrementTrips` + `SetFleeTargetPicked(false)`,
/// closing the trip and clearing the chain-ordering predicate so a
/// fresh re-plan would re-pick.
///
/// **Runtime preconditions** — `route_cost_field.is_some()`. Each
/// tick: if the cat's current tile has cost ≤
/// `route_cost_safe_threshold` AND `safety_need ≥ safety_need_threshold`,
/// the per-step `ticks` counter (managed by the executor; supplied as
/// the `ticks` argument) advances; once `ticks >= flee_hold_ticks`,
/// returns `witnessed(Advance)`. Otherwise the resolver returns
/// `unwitnessed(Continue)` — the cat stays in the step but the
/// counter doesn't progress. If the field is missing entirely the
/// resolver returns `unwitnessed(Advance)` so the chain doesn't strand
/// the cat (cascade-style fallback mirroring `PickFleeTarget`).
///
/// **Witness** — `StepOutcome<bool>`. `witnessed(Advance)` only on the
/// hold-completion tick. `unwitnessed(Continue)` while still holding
/// or while not yet on a safe tile.
///
/// **Feature emission** — caller passes `Feature::FleeRecovered`
/// (Positive) to `record_if_witnessed`. Cascade-from-rare-event;
/// ships `expected_to_fire_per_soak() => false` until the post-230
/// multi-seed baseline shows reliable firing.
pub fn resolve_hold_until_safe(
    ticks: u64,
    self_pos: Position,
    route_cost_field: Option<&RouteCostField>,
    safety_need: f32,
    flee_hold_ticks: u64,
    route_cost_safe_threshold: u32,
    safety_need_threshold: f32,
) -> StepOutcome<bool> {
    let Some(field) = route_cost_field else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let on_safe_tile = field.cost_at(self_pos) <= route_cost_safe_threshold;
    let safety_recovered = safety_need >= safety_need_threshold;
    if !(on_safe_tile && safety_recovered) {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    if ticks >= flee_hold_ticks {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::route_cost_field::{RouteCostField, MAX_COST_BUDGET};

    fn field_with(cost_at_origin: u32) -> RouteCostField {
        let costs = vec![cost_at_origin; 9];
        RouteCostField {
            costs,
            width: 3,
            height: 3,
            origin: Position::new(1, 1),
            origin_tick: 0,
        }
    }

    #[test]
    fn advances_at_unbuilt_field() {
        let outcome = resolve_hold_until_safe(0, Position::new(1, 1), None, 1.0, 30, 100, 0.6);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(!outcome.witness);
    }

    #[test]
    fn continues_when_unsafe_tile() {
        // High-cost tile: hysteresis can't begin.
        let field = field_with(MAX_COST_BUDGET);
        let outcome =
            resolve_hold_until_safe(50, Position::new(1, 1), Some(&field), 1.0, 30, 100, 0.6);
        assert!(matches!(outcome.result, StepResult::Continue));
    }

    #[test]
    fn continues_when_safety_low() {
        let field = field_with(50);
        let outcome =
            resolve_hold_until_safe(50, Position::new(1, 1), Some(&field), 0.3, 30, 100, 0.6);
        assert!(matches!(outcome.result, StepResult::Continue));
    }

    #[test]
    fn witnesses_advance_on_completion_tick() {
        let field = field_with(50);
        let outcome =
            resolve_hold_until_safe(30, Position::new(1, 1), Some(&field), 0.7, 30, 100, 0.6);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness);
    }

    #[test]
    fn continues_before_hold_ticks_elapse() {
        let field = field_with(50);
        let outcome =
            resolve_hold_until_safe(5, Position::new(1, 1), Some(&field), 0.7, 30, 100, 0.6);
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(!outcome.witness);
    }
}
