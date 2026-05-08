use crate::components::physical::Position;
use crate::components::route_cost_field::{RouteCostField, MAX_COST_BUDGET};
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

#[inline]
fn passable(map: &TileMap, p: Position) -> bool {
    map.in_bounds(p.x, p.y) && map.get(p.x, p.y).terrain.is_passable()
}

/// # GOAP step resolver: `PickFleeTarget` (ticket 230)
///
/// **Real-world effect** — picks the lowest-cost passable tile within
/// Chebyshev `flee_distance` of the cat's current position by reading
/// the per-cat `RouteCostField` (the per-replan substrate built at
/// `goap.rs::evaluate_and_plan` lines 1648-1698, with boldness-scaled
/// fox-scent + corruption overlays). Returns the picked tile via the
/// `Option<Position>` witness so the caller can write it to the GOAP
/// step's `target_position` for the downstream `Flee` umbrella.
///
/// Replaces the naive vector projection at the legacy
/// `check_anxiety_interrupts` arm (`disposition.rs:280-291`,
/// pre-230), which collapsed onto whichever tile was geometrically
/// "away from the threat" without consulting the substrate — leading
/// to chronic re-projection into adjacent fox-scent zones and the
/// 39,536-preempt thrash spiral that motivates this ticket.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::FleeTargetPicked(false)` by
/// `src/ai/planner/actions.rs::fleeing_actions`. Effect on success:
/// `StateEffect::SetFleeTargetPicked(true)`, gating the downstream
/// `Flee` umbrella on the predicate.
///
/// **Runtime preconditions** — `route_cost_field.is_some()`. If the
/// per-cat field hasn't been built yet (pre-flood, despawned and
/// respawned cats) the resolver returns `unwitnessed(Advance)` so the
/// chain still progresses; the umbrella `Flee` step's `cat_path_plan!`
/// will fall back to A\* and `Feature::RouteCostFieldFallback` will
/// fire on the canary. Tile passability is checked via
/// `TileMap::is_passable`.
///
/// **Witness** — `StepOutcome<Option<Position>>`. `Some(target)` iff
/// the resolver picked a tile that strictly improves on the cat's
/// current cost (i.e., somewhere safer to go). `None` (witness =
/// `None`, result = `Advance`) means the cat's current tile is
/// already minimum-cost in the disc — the chain advances and
/// `HoldUntilSafe` will immediately count its first tick.
///
/// **Feature emission** — caller passes `Feature::FleeTargetPicked`
/// (Positive) to `record_if_witnessed`. The Feature ships
/// `expected_to_fire_per_soak() => false` (cascade from
/// `AcuteHealthAdrenalineFlee` / `ThreatProximityAdrenalineFlee`
/// lifting Flee, which is rare on a healthy colony). Promote after
/// the post-230 multi-seed baseline.
pub fn resolve_pick_flee_target(
    self_pos: Position,
    route_cost_field: Option<&RouteCostField>,
    threat_pos: Option<Position>,
    flee_distance: f32,
    map: &TileMap,
) -> StepOutcome<Option<Position>> {
    let radius = flee_distance.max(1.0).round() as i32;
    let Some(field) = route_cost_field else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let current_cost = field.cost_at(self_pos);

    let mut best: Option<(Position, u32, i32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let candidate = Position::new(self_pos.x + dx, self_pos.y + dy);
            if candidate == self_pos {
                continue;
            }
            if !passable(map, candidate) {
                continue;
            }
            let cost = field.cost_at(candidate);
            if cost >= MAX_COST_BUDGET {
                continue;
            }
            // Tie-break: farther from threat wins. When no threat is
            // visible, fall back to farther-from-self (Chebyshev) so
            // the picker still moves the cat off its current tile.
            let tiebreak = match threat_pos {
                Some(tp) => -squared_distance(candidate, tp),
                None => -squared_distance(candidate, self_pos),
            };
            match best {
                None => best = Some((candidate, cost, tiebreak)),
                Some((_, best_cost, best_tb)) => {
                    if cost < best_cost || (cost == best_cost && tiebreak > best_tb) {
                        best = Some((candidate, cost, tiebreak));
                    }
                }
            }
        }
    }

    match best {
        Some((target, cost, _)) if cost < current_cost => {
            StepOutcome::witnessed_with(StepResult::Advance, target)
        }
        _ => StepOutcome::unwitnessed(StepResult::Advance),
    }
}

/// Squared Chebyshev-style distance for tie-break ordering. Negated by
/// the caller so "farther wins" is "larger value wins."
#[inline]
fn squared_distance(a: Position, b: Position) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::route_cost_field::RouteCostField;
    use crate::resources::map::{Terrain, TileMap};

    fn open_map(width: u32, height: u32) -> TileMap {
        TileMap::new(width as i32, height as i32, Terrain::Grass)
    }

    fn field_with_costs(width: u32, height: u32, origin: Position, costs: Vec<u32>) -> RouteCostField {
        RouteCostField {
            costs,
            width,
            height,
            origin,
            origin_tick: 0,
        }
    }

    #[test]
    fn returns_none_when_no_field_built() {
        let map = open_map(10, 10);
        let outcome = resolve_pick_flee_target(
            Position::new(5, 5),
            None,
            Some(Position::new(5, 4)),
            8.0,
            &map,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn picks_low_cost_tile_when_cheaper_than_current() {
        // 3x3 field. Cat is at (1,1) with cost 50. (0,0) has cost 10
        // (cheap exit), (2,2) has cost 200 (high). Expect (0,0).
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[0] = 10; // (0,0)
        costs[4] = 50; // (1,1) — cat
        costs[8] = 200; // (2,2)
        let field = field_with_costs(3, 3, Position::new(1, 1), costs);
        let map = open_map(3, 3);
        let outcome = resolve_pick_flee_target(
            Position::new(1, 1),
            Some(&field),
            Some(Position::new(2, 2)),
            8.0,
            &map,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(outcome.witness, Some(Position::new(0, 0)));
    }

    #[test]
    fn returns_unwitnessed_when_current_already_minimum() {
        // Cat at (0,0) which is the min-cost tile. Picker should
        // advance without emitting a witness.
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[0] = 10;
        costs[8] = 50;
        let field = field_with_costs(3, 3, Position::new(0, 0), costs);
        let map = open_map(3, 3);
        let outcome = resolve_pick_flee_target(
            Position::new(0, 0),
            Some(&field),
            Some(Position::new(2, 2)),
            8.0,
            &map,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn skips_unreachable_tiles() {
        // Two tiles share the minimum reachable cost; only one is
        // strictly less than current. Verify unreached sentinels are
        // skipped.
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[0] = MAX_COST_BUDGET; // unreached
        costs[1] = 30; // (1,0) — reachable, cheap
        costs[4] = 100; // (1,1) — cat
        let field = field_with_costs(3, 3, Position::new(1, 1), costs);
        let map = open_map(3, 3);
        let outcome = resolve_pick_flee_target(
            Position::new(1, 1),
            Some(&field),
            None,
            8.0,
            &map,
        );
        assert_eq!(outcome.witness, Some(Position::new(1, 0)));
    }
}
