//! Bucket-queue Dijkstra flood for the per-cat `RouteCostField`,
//! plus the gradient-descent step-walker used by `CatPathPlan`
//! (commit 10).
//!
//! See `src/components/route_cost_field.rs` for the data shape and
//! the substrate-vs-search-state framing.
//!
//! **Why bucket-queue, not BinaryHeap.** Edge weights here are
//! bounded small integers — terrain max=4 + per-tile weighted overlay
//! max ≈ 18 (fox-scent max=8 + corruption max=10) ≈ 22. With a known
//! upper bound on edge cost the flat-queue Dijkstra runs in
//! `O(V·D + E)` versus `BinaryHeap`'s `O(E log V)`, an
//! order-of-magnitude win at our 100×100 scale (Walker's published
//! Brogue 2010 pattern — same lineage as `pathfinding::find_path`'s
//! A\*, just for floods).
//!
//! Determinism: neighbor expansion order is fixed by `NEIGHBORS`;
//! ties are broken by the order tiles enter the bucket.

use crate::ai::pathfinding::{
    find_path, step_toward, sum_overlay_cost, CorruptionOverlay, FoxScentOverlay, WeightedOverlay,
};
use crate::components::physical::Position;
use crate::components::route_cost_field::{RouteCostField, MAX_COST_BUDGET};
use crate::resources::map::TileMap;

/// 8-directional neighbor offsets — same order as
/// `pathfinding::NEIGHBORS` for parity between the flood and A\*.
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Flood from `from` outward, computing cost-to-reach for every
/// tile within the flood radius implied by `max_cost`.
///
/// Edge weights = `terrain.movement_cost() + sum_overlay_cost(neighbor)`
/// — identical to `pathfinding::find_path`'s edge cost so a flood and
/// an A\* path through the same edge see the same number. The
/// `cat_path_weight_from_boldness(boldness)` factor is baked into
/// each `WeightedOverlay::weight` by the caller (per-cat boldness is
/// what the cat *sees*; bold cats flood with low overlay weight,
/// timid cats with high).
///
/// `max_cost` is clamped to `MAX_COST_BUDGET`. Tiles whose tentative
/// cost would exceed the cap are not relaxed; their `costs[]` entry
/// stays at `MAX_COST_BUDGET`. Returns a fully-populated field even
/// when `from` is out-of-bounds (every tile at `MAX_COST_BUDGET`).
pub fn flood_dijkstra(
    from: Position,
    map: &TileMap,
    overlays: &[WeightedOverlay<'_>],
    max_cost: u32,
    origin_tick: u64,
    // Ticket 427 Step 2 — caller-provided bucket arena. Replaces the
    // per-call `Vec<Vec<Position>>` allocation that was the second-largest
    // alloc hotspot in the 427 survey (~96 MB/soak at 500-cat projection).
    // The function grows the outer vec to `cap+1`, clears each inner Vec
    // in-place, then drains by `mem::swap`-out / `mem::swap`-back so inner
    // Vec capacities persist across calls.
    buckets: &mut Vec<Vec<Position>>,
) -> RouteCostField {
    let width = map.width.max(0) as u32;
    let height = map.height.max(0) as u32;
    let mut field = RouteCostField::empty(width, height, from, origin_tick);

    if !map.in_bounds(from.x, from.y) {
        return field;
    }

    let cap = max_cost.min(MAX_COST_BUDGET);
    // Grow the outer Vec only if this call needs more buckets than any
    // prior call. Clear every existing inner Vec to drop residue without
    // releasing its capacity.
    let target_len = cap as usize + 1;
    if buckets.len() < target_len {
        buckets.resize_with(target_len, Vec::new);
    }
    for b in buckets.iter_mut() {
        b.clear();
    }

    let from_idx = (from.y as u32 * width + from.x as u32) as usize;
    field.costs[from_idx] = 0;
    buckets[0].push(from);

    // Reusable scratch Vec for the `mem::swap`-drain pattern (preserves
    // capacity of the bucket we swapped out across the inner loop).
    let mut current_bucket: Vec<Position> = Vec::new();

    for current_cost in 0..=cap as usize {
        // Pull the bucket out so we can mutate `buckets` (push to
        // higher buckets) while draining the current one. `mem::swap`
        // (not `mem::take`) preserves the inner Vec's capacity across
        // flood calls — `take` would replace with a brand-new empty Vec.
        std::mem::swap(&mut current_bucket, &mut buckets[current_cost]);
        while let Some(pos) = current_bucket.pop() {
            let pos_idx = (pos.y as u32 * width + pos.x as u32) as usize;
            // Stale entry — a cheaper relaxation already settled this
            // tile, skip.
            if field.costs[pos_idx] != current_cost as u32 {
                continue;
            }
            for &(dx, dy) in &NEIGHBORS {
                let nx = pos.x + dx;
                let ny = pos.y + dy;
                if !map.in_bounds(nx, ny) {
                    continue;
                }
                let terrain = map.get(nx, ny).terrain;
                if !terrain.is_passable() {
                    continue;
                }
                let neighbor = Position::new(nx, ny);
                let edge = terrain
                    .movement_cost()
                    .saturating_add(sum_overlay_cost(overlays, neighbor));
                let tentative = (current_cost as u32).saturating_add(edge);
                if tentative > cap {
                    continue;
                }
                let n_idx = (ny as u32 * width + nx as u32) as usize;
                if tentative < field.costs[n_idx] {
                    field.costs[n_idx] = tentative;
                    buckets[tentative as usize].push(neighbor);
                }
            }
        }
        // Put the now-empty (but capacity-preserved) Vec back so it
        // can be reused for any later bucket in the next call.
        std::mem::swap(&mut current_bucket, &mut buckets[current_cost]);
    }

    field
}

/// Walk one tile from `from` toward `to` along the cost gradient of
/// `field`. Lex-orders neighbors by `(chebyshev_to_to, route_cost)`:
/// forward-progress (lower chebyshev to destination) dominates, with
/// route-cost as the tiebreak among equal-progress candidates.
///
/// **Gradient direction caveat.** The flood origin is the cat
/// itself, so `cost_at(from) = 0` (assuming `from == field.origin`).
/// Pure cost-gradient descent (`min cost_at(neighbor)`) would walk
/// back to the origin — the chebyshev primary key is what keeps the
/// walk directional. The route-cost secondary key is what routes
/// the walk *around* expensive overlay tiles (fox scent, corruption)
/// among neighbors that all make equal forward progress. This is the
/// cost of a single per-replan field supporting both score-time
/// reads (closer-is-better at `to`) and step-time walks (toward
/// `to`).
///
/// Returns `None` if every neighbor is impassable, out-of-bounds, or
/// out-of-budget (`MAX_COST_BUDGET`).
pub fn step_along_field(
    from: Position,
    to: Position,
    field: &RouteCostField,
    map: &TileMap,
) -> Option<Position> {
    if from == to {
        return None;
    }
    let mut best: Option<(Position, (u64, u32))> = None;
    for &(dx, dy) in &NEIGHBORS {
        let nx = from.x + dx;
        let ny = from.y + dy;
        if !map.in_bounds(nx, ny) {
            continue;
        }
        if !map.get(nx, ny).terrain.is_passable() {
            continue;
        }
        let neighbor = Position::new(nx, ny);
        let cost = field.cost_at(neighbor);
        if cost >= MAX_COST_BUDGET {
            continue;
        }
        let cdx = (neighbor.x - to.x).unsigned_abs() as u64;
        let cdy = (neighbor.y - to.y).unsigned_abs() as u64;
        let h = cdx.max(cdy);
        // Lex order: (chebyshev_to_destination, route_cost). Strict-`<`
        // so first candidate in `NEIGHBORS` order wins true ties —
        // mirrors `step_toward`'s tiebreak.
        let key = (h, cost);
        if best.as_ref().is_none_or(|&(_, k)| key < k) {
            best = Some((neighbor, key));
        }
    }
    best.map(|(p, _)| p)
}

// ---------------------------------------------------------------------------
// CatPathPlan — gradient-walk vs A* fallback dispatcher (commit 10)
// ---------------------------------------------------------------------------

/// Per-cat path-resolution dispatcher. Step resolvers (commit 11+)
/// construct one of these per call, then call `next_step` /
/// `find_full_path` against it. The `Field` arm walks the cached
/// route-cost field's gradient (cheap, replan-cadence stale); the
/// `AStarFallback` arm calls `find_path` with overlay slices
/// (expensive, always-fresh) when the field is missing, stale, or
/// doesn't reach the destination.
///
/// **Fallback observability** — callers should emit
/// `Feature::RouteCostFieldFallback` whenever they construct an
/// `AStarFallback` so the canary can detect chronic field-build or
/// staleness bugs (`expected_to_fire_per_soak() == false`).
pub enum CatPathPlan<'a> {
    /// Walk the gradient of `field` toward the destination. Cheapest
    /// path (no per-step search). Use when the field is fresh and
    /// reaches the destination.
    Field(&'a RouteCostField),
    /// Fall back to A\* with overlay slices. Use when no field is
    /// available, or when the field is stale / doesn't reach the
    /// destination. Boldness factor `weight` is baked into both
    /// overlay weights — same input shape as `find_path` callers
    /// historically used.
    AStarFallback {
        fox: FoxScentOverlay<'a>,
        corr: CorruptionOverlay<'a>,
        weight: f32,
    },
    /// Plain A\* without overlays. Used by legacy disposition-chain
    /// paths in `systems/task_chains.rs` that historically passed
    /// `find_path(.., &[])` and don't yet plumb FoxScentMap. Behavior
    /// is byte-identical to `AStarFallback { weight: 0.0, .. }` (the
    /// `WeightedOverlay::cost_at` of a zero-weight overlay is always
    /// 0), but skipping overlay construction avoids the FoxScentMap
    /// dependency in those callers. Foxes also use this shape via
    /// their own `find_path` path; this variant is cat-side legacy.
    NoOverlay,
}

impl<'a> CatPathPlan<'a> {
    /// True iff this plan's underlying field reaches `to` in
    /// budget. `Field` arms with `cost_at(to) >= MAX_COST_BUDGET`
    /// or older than `replan_window_ticks` should fall back to A\*
    /// — `should_fall_back_at` answers the staleness/reach check
    /// callers run before constructing the plan.
    pub fn should_fall_back_at(
        field: &RouteCostField,
        to: Position,
        current_tick: u64,
        replan_window_ticks: u64,
    ) -> bool {
        if !field.is_reachable(to) {
            return true;
        }
        // `origin_tick + replan_window < current_tick` ⇒ stale.
        // Saturating to avoid underflow at very low tick counts.
        current_tick.saturating_sub(field.origin_tick) > replan_window_ticks
    }

    /// One-step walk from `from` toward `to`. Returns `None` when
    /// the cat is stuck (no passable neighbor) or already at `to`.
    pub fn next_step(&self, from: Position, to: Position, map: &TileMap) -> Option<Position> {
        match self {
            CatPathPlan::Field(field) => step_along_field(from, to, field, map),
            CatPathPlan::AStarFallback { fox, corr, weight } => {
                let overlays = [
                    WeightedOverlay::new(fox, *weight),
                    WeightedOverlay::new(corr, *weight),
                ];
                step_toward(&from, &to, map, &overlays)
            }
            CatPathPlan::NoOverlay => step_toward(&from, &to, map, &[]),
        }
    }

    /// Full path from `from` to `to` as a `Vec<Position>` (excluding
    /// `from`, ending at `to`). The `Field` arm reconstructs by
    /// repeatedly calling `step_along_field`; the `AStarFallback`
    /// arm calls `find_path` directly. Returns `None` if no path
    /// exists.
    pub fn find_full_path(
        &self,
        from: Position,
        to: Position,
        map: &TileMap,
    ) -> Option<Vec<Position>> {
        match self {
            CatPathPlan::Field(field) => {
                if from == to {
                    return Some(Vec::new());
                }
                let mut path = Vec::new();
                let mut cur = from;
                while cur != to {
                    let next = step_along_field(cur, to, field, map)?;
                    if next == cur {
                        return None; // stuck
                    }
                    path.push(next);
                    cur = next;
                    if path.len() > (map.width as usize) * (map.height as usize) {
                        return None; // safety cap; field-walk shouldn't loop
                    }
                }
                Some(path)
            }
            CatPathPlan::AStarFallback { fox, corr, weight } => {
                let overlays = [
                    WeightedOverlay::new(fox, *weight),
                    WeightedOverlay::new(corr, *weight),
                ];
                find_path(from, to, map, &overlays)
            }
            CatPathPlan::NoOverlay => find_path(from, to, map, &[]),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::pathfinding::{find_path, TileCostOverlay};
    use crate::resources::map::{Terrain, TileMap};

    fn open_map() -> TileMap {
        TileMap::new(10, 10, Terrain::Grass)
    }

    /// Sparse overlay: returns `cost` at one tile, `0` everywhere else.
    struct PointOverlay {
        target: Position,
        cost: u32,
    }
    impl TileCostOverlay for PointOverlay {
        fn cost_at(&self, pos: Position) -> u32 {
            if pos == self.target {
                self.cost
            } else {
                0
            }
        }
    }

    /// Flood from origin on open terrain — every reachable tile has
    /// cost equal to chebyshev distance × terrain cost (1 for grass).
    #[test]
    fn flood_open_terrain_costs_match_chebyshev() {
        let map = open_map();
        let from = Position::new(0, 0);
        let field = flood_dijkstra(from, &map, &[], MAX_COST_BUDGET, 0, &mut Vec::new());
        assert_eq!(field.cost_at(from), 0);
        // Diagonal: cost = 5 (5 × grass=1).
        assert_eq!(field.cost_at(Position::new(5, 5)), 5);
        // Cardinal: same — 5 grass steps.
        assert_eq!(field.cost_at(Position::new(0, 5)), 5);
        assert_eq!(field.cost_at(Position::new(5, 0)), 5);
        // Far corner reachable.
        assert!(field.is_reachable(Position::new(9, 9)));
    }

    /// Flood with a single high-cost overlay tile in the direct path
    /// forces the cost-to-reach beyond the obstacle to reflect the
    /// detour, not the direct path.
    #[test]
    fn flood_correctness_5x5_with_obstacle() {
        let map = TileMap::new(5, 5, Terrain::Grass);
        // Plant a 50-cost overlay on (2,2) — the natural diagonal from
        // (0,0) to (4,4) would pass through it.
        let blocker = PointOverlay {
            target: Position::new(2, 2),
            cost: 50,
        };
        let overlays: [WeightedOverlay; 1] = [WeightedOverlay::new(&blocker, 1.0)];

        let field = flood_dijkstra(Position::new(0, 0), &map, &overlays, MAX_COST_BUDGET, 0, &mut Vec::new());

        // Direct cost to (4,4) on open terrain is 4 (4 diagonal grass
        // steps). Detour via (2,1) or (2,3) adds at most ~1 extra
        // grass step. Either way the cost stays ≤ 5 — vastly cheaper
        // than crossing the 50-cost tile.
        let dest_cost = field.cost_at(Position::new(4, 4));
        assert!(
            dest_cost > 0 && dest_cost <= 5,
            "detour cost {dest_cost} should reflect the path AROUND the 50-cost blocker, not through it"
        );
        // The blocker tile itself has high reach cost.
        assert!(field.cost_at(Position::new(2, 2)) >= 50);
    }

    /// Iteratively walking the field from `from` to `to` via
    /// `step_along_field` traces a path of equal length to A\*'s on
    /// open terrain (no overlays) — the equivalence claim that lets
    /// `CatPathPlan::Field` substitute for A\* without behavior drift
    /// at no-overlay state.
    #[test]
    fn gradient_descent_matches_astar_no_overlay() {
        let map = open_map();
        let from = Position::new(0, 0);
        let to = Position::new(7, 5);

        let field = flood_dijkstra(from, &map, &[], MAX_COST_BUDGET, 0, &mut Vec::new());
        let astar = find_path(from, to, &map, &[]).expect("A* path should exist");

        // Trace the field-walk.
        let mut walk: Vec<Position> = Vec::new();
        let mut cur = from;
        while cur != to {
            let next = step_along_field(cur, to, &field, &map)
                .expect("step_along_field should advance on open terrain");
            assert_ne!(next, cur, "step_along_field must not stall");
            walk.push(next);
            cur = next;
            assert!(walk.len() < 100, "field walk should terminate quickly");
        }

        assert_eq!(
            walk.len(),
            astar.len(),
            "field walk length {} should match A* path length {}",
            walk.len(),
            astar.len()
        );
        assert_eq!(*walk.last().unwrap(), to);
    }

    /// A small `max_cost` budget bounds the flood radius — far
    /// tiles stay unreached at `MAX_COST_BUDGET`.
    #[test]
    fn flood_respects_max_cost_budget() {
        let map = open_map();
        let from = Position::new(0, 0);
        // Budget of 3 — only tiles within ~3 chebyshev steps reachable.
        let field = flood_dijkstra(from, &map, &[], 3, 0, &mut Vec::new());
        assert_eq!(field.cost_at(Position::new(3, 3)), 3);
        // (4,4) needs 4 grass steps = cost 4 > 3, so unreached.
        assert_eq!(
            field.cost_at(Position::new(4, 4)),
            MAX_COST_BUDGET,
            "tile beyond budget should retain MAX_COST_BUDGET"
        );
        assert!(!field.is_reachable(Position::new(4, 4)));
    }

    /// Flood from an out-of-bounds origin returns an empty (all-MAX)
    /// field — never panics.
    #[test]
    fn flood_oob_origin_returns_empty_field() {
        let map = open_map();
        let field = flood_dijkstra(Position::new(-1, -1), &map, &[], MAX_COST_BUDGET, 0, &mut Vec::new());
        assert_eq!(field.width, 10);
        assert_eq!(field.height, 10);
        for c in &field.costs {
            assert_eq!(*c, MAX_COST_BUDGET);
        }
    }

    /// Flood respects impassable tiles — water blocks expansion.
    #[test]
    fn flood_skips_impassable_tiles() {
        let mut map = open_map();
        for y in 0..9 {
            map.set(5, y, Terrain::Water);
        }
        // Reach (6,4) only via (5,9) gap.
        let field = flood_dijkstra(Position::new(4, 4), &map, &[], MAX_COST_BUDGET, 0, &mut Vec::new());
        assert_eq!(
            field.cost_at(Position::new(5, 4)),
            MAX_COST_BUDGET,
            "water tile must remain unreached"
        );
        assert!(field.is_reachable(Position::new(6, 4)));
        // The detour cost should exceed the direct distance (which
        // would have been 2 chebyshev steps through (5,4)).
        assert!(field.cost_at(Position::new(6, 4)) > 2);
    }

    /// `step_along_field` returns None when from == to.
    #[test]
    fn step_along_field_none_at_destination() {
        let map = open_map();
        let from = Position::new(3, 3);
        let field = flood_dijkstra(from, &map, &[], MAX_COST_BUDGET, 0, &mut Vec::new());
        assert!(step_along_field(from, from, &field, &map).is_none());
    }
}
