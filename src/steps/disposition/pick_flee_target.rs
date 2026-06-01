use crate::components::physical::Position;
use crate::components::route_cost_field::{RouteCostField, MAX_COST_BUDGET};
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

#[inline]
fn passable(map: &TileMap, p: Position) -> bool {
    map.in_bounds(p.x(), p.y()) && map.get(p.x(), p.y()).terrain.is_passable()
}

/// # GOAP step resolver: `PickFleeTarget` (ticket 230, witness rebind 254)
///
/// **Real-world effect** — picks the best passable tile within
/// Chebyshev `flee_distance` of the cat's current position by reading
/// the per-cat `RouteCostField` (the per-replan substrate built at
/// `goap.rs::evaluate_and_plan` lines 1648-1698, with boldness-scaled
/// fox-scent + corruption overlays). "Best" minimizes
/// `effective_cost = field.cost_at(candidate) as i32 - chebyshev(candidate, threat)`,
/// rewarding tiles that are simultaneously cheap-to-reach AND
/// far-from-threat. Returns the picked tile via the
/// `Option<Position>` witness so the caller can write it to the GOAP
/// step's `target_position` for the downstream `Flee` umbrella.
///
/// Replaces the naive vector projection at the legacy
/// `check_anxiety_interrupts` arm (`disposition.rs:280-291`,
/// pre-230), which collapsed onto whichever tile was geometrically
/// "away from the threat" without consulting the substrate — leading
/// to chronic re-projection into adjacent fox-scent zones and the
/// 39,536-preempt thrash spiral that motivates ticket 230.
///
/// **Witness rebind (254):** the original 230 contract emitted only
/// when `cost < current_cost`, but `flood_dijkstra` (`route_cost.rs:74`)
/// hardcodes `field.costs[origin] = 0`, so no candidate could ever
/// satisfy `cost < 0` (u32) and the witness was unreachable in
/// production. 254 R5 (extend) replaces the absolute-cost compare
/// with the effective-cost minimization above — the picker now emits
/// whenever any reachable, passable, non-self tile exists in the disc.
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
/// the disc contains at least one reachable, passable tile other
/// than the cat's current position. `None` (witness = `None`, result
/// = `Advance`) means the cat is boxed in (no reachable non-self
/// tile in radius) — the chain advances and `HoldUntilSafe` will
/// immediately count its first tick on the cat's current position.
///
/// **Feature emission** — caller passes `Feature::FleeTargetPicked`
/// (Positive) to `record_if_witnessed`. The Feature ships
/// `expected_to_fire_per_soak() => false` (cascade from
/// `ThreatProximityAdrenalineFlee` lifting Flee, which is rare on a
/// healthy colony — pre-251 also `AcuteHealthAdrenalineFlee` lifted
/// Flee on injury, but 251 retired that lift). Promote to `true`
/// after the post-254 multi-seed baseline.
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

    // Track (candidate, effective_cost, raw_cost). Smaller effective_cost
    // wins; ties broken by smaller raw_cost (cheap-to-reach wins at
    // equal threat-distance).
    let mut best: Option<(Position, i32, u32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let candidate = Position::new(self_pos.x() + dx, self_pos.y() + dy);
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
            // effective_cost = cheap-to-reach AND far-from-threat.
            // No threat visible → behave as a pure cheap-tile picker.
            let effective_cost = match threat_pos {
                Some(tp) => cost as i32 - chebyshev(candidate, tp),
                None => cost as i32,
            };
            match best {
                None => best = Some((candidate, effective_cost, cost)),
                Some((_, best_effective, best_raw)) => {
                    if effective_cost < best_effective
                        || (effective_cost == best_effective && cost < best_raw)
                    {
                        best = Some((candidate, effective_cost, cost));
                    }
                }
            }
        }
    }

    match best {
        Some((target, _, _)) => StepOutcome::witnessed_with(StepResult::Advance, target),
        None => StepOutcome::unwitnessed(StepResult::Advance),
    }
}

/// Chebyshev distance — `max(|dx|, |dy|)`. Matches the disc-iteration
/// shape (the picker sweeps a square of side `2 * radius + 1`).
#[inline]
fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x() - b.x()).abs().max((a.y() - b.y()).abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::route_cost_field::RouteCostField;
    use crate::resources::map::{Terrain, TileMap};

    fn open_map(width: u32, height: u32) -> TileMap {
        TileMap::new(width as i32, height as i32, Terrain::Grass)
    }

    fn field_with_costs(
        width: u32,
        height: u32,
        origin: Position,
        costs: Vec<u32>,
    ) -> RouteCostField {
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
    fn picks_low_effective_cost_tile() {
        // 3x3 field, cat at (1,1) with the flood-origin cost = 0.
        // Threat at (2,2). With effective_cost = cost - chebyshev_to_threat:
        //   (0,0) raw=10, cheb=2 → effective = 8
        //   (2,2) raw=200, cheb=0 → effective = 200 (toward threat: bad)
        //   (0,2) raw=MAX (unreachable, skipped)
        //   ... corners are equidistant under chebyshev to (2,2):
        //     (0,1) cheb=2, (0,0) cheb=2, (1,0) cheb=2, (2,0) cheb=2
        // Only (0,0) and (2,2) carry sub-MAX cost; (0,0) wins.
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[0] = 10; // (0,0)
        costs[4] = 0; // (1,1) — cat (flood origin)
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
    fn returns_unwitnessed_when_no_reachable_tile_in_disc() {
        // Cat at (1,1) is the only reachable tile; every other slot is
        // MAX_COST_BUDGET (unreached) or self. Picker should advance
        // without emitting a witness — the cat is boxed in.
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[4] = 0; // (1,1) — cat (flood origin); self is skipped
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
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn skips_unreachable_tiles() {
        // Cat at (1,1). Only (1,0) is reachable at cost 30; (0,0) is
        // sentinel (MAX_COST_BUDGET, unreached). Without a threat the
        // picker minimizes raw cost; (1,0) wins.
        let mut costs = vec![MAX_COST_BUDGET; 9];
        costs[0] = MAX_COST_BUDGET; // (0,0) — unreached
        costs[1] = 30; // (1,0) — reachable, cheap
        costs[4] = 0; // (1,1) — cat (flood origin)
        let field = field_with_costs(3, 3, Position::new(1, 1), costs);
        let map = open_map(3, 3);
        let outcome = resolve_pick_flee_target(Position::new(1, 1), Some(&field), None, 8.0, &map);
        assert_eq!(outcome.witness, Some(Position::new(1, 0)));
    }

    #[test]
    fn prefers_far_from_threat_when_costs_tied() {
        // 5x1 strip. Cat at (2,0). Two candidates at equal raw cost:
        // (0,0) cost=20 and (4,0) cost=20. Threat at (0,0).
        // Chebyshev(threat, (4,0)) = 4; chebyshev(threat, (0,0)) = 0.
        // Effective: (4,0) = 20-4 = 16; (0,0) = 20-0 = 20 (and also
        // skipped because the threat sits ON it). (4,0) wins —
        // farther from the threat at equal raw cost.
        // Other slots: (1,0) cost=20, eff = 20-1 = 19; (3,0) cost=20,
        // eff = 20-3 = 17. So (4,0) is the global min.
        let mut costs = vec![20u32; 5];
        costs[2] = 0; // cat origin
        let field = field_with_costs(5, 1, Position::new(2, 0), costs);
        let map = open_map(5, 1);
        let outcome = resolve_pick_flee_target(
            Position::new(2, 0),
            Some(&field),
            Some(Position::new(0, 0)),
            8.0,
            &map,
        );
        assert_eq!(outcome.witness, Some(Position::new(4, 0)));
    }

    #[test]
    fn picks_when_only_origin_is_zero() {
        // Reproduces the production scenario that 254 fixes: cat sits
        // at flood origin (cost 0), every reachable neighbor has
        // uniform positive cost. Pre-254 the witness was unreachable;
        // post-254 the picker always emits when at least one
        // reachable non-self tile exists.
        let mut costs = vec![10u32; 9];
        costs[4] = 0; // (1,1) — cat (flood origin, cost 0)
        let field = field_with_costs(3, 3, Position::new(1, 1), costs);
        let map = open_map(3, 3);
        let outcome = resolve_pick_flee_target(
            Position::new(1, 1),
            Some(&field),
            Some(Position::new(0, 0)),
            8.0,
            &map,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(
            outcome.witness.is_some(),
            "cat-at-origin must still emit a witness when a passable+reachable disc exists"
        );
        // Several corner tiles tie at effective_cost = 8 (raw 10,
        // chebyshev 2 to threat at (0,0)). Iteration order emits the
        // first one encountered in (dy, dx) sweep: (2, 0).
        assert_eq!(outcome.witness, Some(Position::new(2, 0)));
    }
}
