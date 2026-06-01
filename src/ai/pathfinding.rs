use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use crate::components::physical::Position;
use crate::resources::map::TileMap;

// ---------------------------------------------------------------------------
// Tile-cost overlays (substrate, not search state — §4.7)
// ---------------------------------------------------------------------------

/// Per-tile additive cost contribution overlaid on terrain movement cost.
///
/// Implementors sense properties of the cat's environment (fox scent,
/// corruption, etc.) and surface them as routing cost. Per the §4.7
/// substrate-vs-search-state boundary in `docs/systems/ai-substrate-refactor.md`,
/// these overlays are **substrate** — the cat perceives scent in the world
/// it moves through — not search state.
///
/// **Admissibility constraint.** The Chebyshev heuristic in [`find_path`] is
/// admissible iff every edge cost is non-negative. Implementors MUST return
/// `cost_at >= 0` (`u32` enforces this at the type level — `cost_at` cannot
/// produce a negative value). Any non-negative additive g-cost preserves
/// `f(n) ≤ g(n) + h(n)` so optimality is preserved.
pub trait TileCostOverlay: Send + Sync {
    fn cost_at(&self, pos: Position) -> u32;
}

/// Per-overlay weight applied to [`TileCostOverlay::cost_at`] before
/// summation. Ticket 224 — per-cat boldness conditions the threat-cost
/// weight: bold cats use `weight ≈ 0.1` (still respect a 10% threat
/// floor — no suicidal direct routes through fox dens), timid cats use
/// `weight = 1.0` (full-weight detours).
///
/// Express `weight = 0.0` by **omitting** the overlay rather than
/// passing a zero-weight one — avoids the `f32→u32` round footgun on
/// small weights × small contributions.
///
/// Note on the boldness double-read: `Personality.boldness` reads at
/// two layers in the AI substrate — the L2 axis on Patrol (and similar)
/// in `src/ai/scoring.rs:649` decides *whether* the cat patrols; the
/// path-cost weight here decides *where* the patrol routes. These are
/// **complementary, not redundant** — the L2 axis gates the action; the
/// path weight gates the route. Do not collapse in refactor.
pub struct WeightedOverlay<'a> {
    pub inner: &'a dyn TileCostOverlay,
    pub weight: f32,
}

impl<'a> WeightedOverlay<'a> {
    pub fn new(inner: &'a dyn TileCostOverlay, weight: f32) -> Self {
        Self { inner, weight }
    }
}

/// Convert per-cat boldness into the per-overlay weight used by the
/// cat-side path-cost overlays. Bold cats (boldness=1.0) get weight 0.1;
/// timid cats (boldness=0.0) get weight 1.0. The 0.1 floor preserves
/// some threat-cost respect even at maximum boldness — bold cats route
/// past fox scent rather than directly through fox dens.
#[inline]
pub fn cat_path_weight_from_boldness(boldness: f32) -> f32 {
    (1.0 - boldness).clamp(0.1, 1.0)
}

/// Sum the per-overlay weighted contribution at `pos`, rounded to
/// `u32`. Reused by `flood_dijkstra` (route-cost field, ticket 228)
/// — keep the rounding semantics identical so a flood and an A*
/// path through the same edge see the same edge cost.
#[inline]
pub(crate) fn sum_overlay_cost(overlays: &[WeightedOverlay<'_>], pos: Position) -> u32 {
    overlays
        .iter()
        .map(|o| (o.inner.cost_at(pos) as f32 * o.weight).round() as u32)
        .sum()
}

// ---------------------------------------------------------------------------
// Cat-side overlay impls (ticket 223)
// ---------------------------------------------------------------------------

/// Routing cost overlay reading per-tile fox scent from
/// [`FoxScentMap`]. Substrate, not search state (§4.7) — the cat senses
/// scent in the environment it moves through.
///
/// Cost shape: `round(scent.clamp(0, 1) * max_cost)`. With default
/// `max_cost = 8`, max-scent tiles add 8 to A* edge cost so a cat
/// prefers a four-tile detour over crossing them.
#[derive(Clone, Copy)]
pub struct FoxScentOverlay<'a> {
    map: &'a crate::resources::FoxScentMap,
    max_cost: u32,
}

impl<'a> FoxScentOverlay<'a> {
    pub fn new(
        map: &'a crate::resources::FoxScentMap,
        sc: &crate::resources::sim_constants::ScoringConstants,
    ) -> Self {
        Self {
            map,
            max_cost: sc.fox_scent_path_cost_max,
        }
    }
}

impl TileCostOverlay for FoxScentOverlay<'_> {
    fn cost_at(&self, pos: Position) -> u32 {
        let scent = self.map.get(pos.x(), pos.y()).clamp(0.0, 1.0);
        (scent * self.max_cost as f32).round() as u32
    }
}

/// Routing cost overlay reading per-tile corruption from
/// [`TileMap`]. Substrate, not search state (§4.7).
///
/// Cost shape mirrors [`FoxScentOverlay`]:
/// `round(corruption.clamp(0, 1) * max_cost)`. The lens is constructed
/// inline at the call site (matching `CorruptionLens` in
/// `src/systems/influence_map.rs`) — no persistent resource is needed
/// since corruption lives on `Tile`, not in a dedicated map.
#[derive(Clone, Copy)]
pub struct CorruptionOverlay<'a> {
    tile_map: &'a TileMap,
    max_cost: u32,
}

impl<'a> CorruptionOverlay<'a> {
    pub fn new(
        tile_map: &'a TileMap,
        sc: &crate::resources::sim_constants::ScoringConstants,
    ) -> Self {
        Self {
            tile_map,
            max_cost: sc.corruption_path_cost_max,
        }
    }
}

impl TileCostOverlay for CorruptionOverlay<'_> {
    fn cost_at(&self, pos: Position) -> u32 {
        if !self.tile_map.in_bounds(pos.x(), pos.y()) {
            return 0;
        }
        let corr = self
            .tile_map
            .get(pos.x(), pos.y())
            .corruption
            .clamp(0.0, 1.0);
        (corr * self.max_cost as f32).round() as u32
    }
}

/// 256 R5 — routing cost overlay reading per-tile cat patrol presence
/// from [`CatPatrolDeterrentMap`]. Read by *fox* A* (the symmetric
/// counterpart to [`FoxScentOverlay`], which cats read). Foxes route
/// around active patrols rather than charging straight through them.
///
/// Cost shape mirrors [`FoxScentOverlay`]:
/// `round(deterrent.clamp(0, 1) * max_cost)`. With default
/// `max_cost = 6`, max-deterrent tiles add 6 to fox A* edge cost,
/// slightly less than fox-scent's 8 so foxes detour around patrols
/// rather than refuse to move toward prey at all.
#[derive(Clone, Copy)]
pub struct CatPatrolDeterrentOverlay<'a> {
    map: &'a crate::resources::CatPatrolDeterrentMap,
    max_cost: u32,
}

impl<'a> CatPatrolDeterrentOverlay<'a> {
    pub fn new(
        map: &'a crate::resources::CatPatrolDeterrentMap,
        sc: &crate::resources::sim_constants::ScoringConstants,
    ) -> Self {
        Self {
            map,
            max_cost: sc.cat_patrol_deterrent_path_cost_max,
        }
    }
}

impl TileCostOverlay for CatPatrolDeterrentOverlay<'_> {
    fn cost_at(&self, pos: Position) -> u32 {
        let v = self.map.get(pos.x(), pos.y()).clamp(0.0, 1.0);
        (v * self.max_cost as f32).round() as u32
    }
}

// ---------------------------------------------------------------------------
// A* pathfinding
// ---------------------------------------------------------------------------

/// Node in the A* open set. Ordered by `f_score` ascending (lowest first)
/// so `BinaryHeap` (a max-heap) pops the best candidate.
#[derive(Debug, Clone, Copy)]
struct Node {
    pos: Position,
    f_score: u32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.f_score == other.f_score
    }
}
impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse: lower f_score = higher priority.
        other.f_score.cmp(&self.f_score)
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 8-directional neighbor offsets.
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

/// Chebyshev distance — admissible heuristic for 8-directional movement
/// with minimum edge cost 1.
fn heuristic(a: &Position, b: &Position) -> u32 {
    let dx = (a.x() - b.x()).unsigned_abs();
    let dy = (a.y() - b.y()).unsigned_abs();
    dx.max(dy)
}

/// Compute an optimal path from `from` to `to` on the tile map using A*.
///
/// Returns a `Vec<Position>` of waypoints **excluding** `from` and ending
/// at `to`. Returns `None` if `to` is unreachable. Returns an empty `Vec`
/// if `from == to`.
///
/// Edge weights are `terrain.movement_cost() + sum(overlays)`, so cats
/// naturally prefer open terrain (Grass=1) over dense forest (3) or rock (4)
/// and additionally route around any non-zero overlay cost (e.g. fox scent,
/// corruption — see [`TileCostOverlay`]).
///
/// The empty slice `&[]` collapses overlay cost to zero, restoring legacy
/// terrain-only routing.
pub fn find_path(
    from: Position,
    to: Position,
    map: &TileMap,
    overlays: &[WeightedOverlay<'_>],
) -> Option<Vec<Position>> {
    if from == to {
        return Some(Vec::new());
    }
    // Ticket 398 — bounds-check BOTH endpoints. Pre-398, only `to`
    // was checked; callers that passed an out-of-bounds `from`
    // (e.g. a stale cached source position) panicked on the
    // `g_score[start_idx] = 0` write below. Returning `None` is the
    // safe semantic: an unrouteable request is indistinguishable
    // from "from-position invalid," and downstream callers already
    // handle the `None` return.
    if !map.in_bounds(from.x(), from.y()) {
        return None;
    }
    if !map.in_bounds(to.x(), to.y()) || !map.get(to.x(), to.y()).terrain.is_passable() {
        return None;
    }

    let w = map.width as usize;
    let h = map.height as usize;
    let idx = |p: &Position| (p.y() as usize) * w + (p.x() as usize);

    // g_score: cheapest known cost from `from` to each tile. u32::MAX = unvisited.
    let mut g_score = vec![u32::MAX; w * h];
    // came_from: previous tile on the best path (-1 = start / unset).
    let mut came_from: Vec<i32> = vec![-1; w * h];

    let start_idx = idx(&from);
    g_score[start_idx] = 0;

    let mut open = BinaryHeap::new();
    open.push(Node {
        pos: from,
        f_score: heuristic(&from, &to),
    });

    while let Some(current) = open.pop() {
        if current.pos == to {
            // Reconstruct path.
            let mut path = Vec::new();
            let mut ci = idx(&current.pos);
            while ci != start_idx {
                let x = (ci % w) as i32;
                let y = (ci / w) as i32;
                path.push(Position::new(x, y));
                ci = came_from[ci] as usize;
            }
            path.reverse();
            return Some(path);
        }

        let current_g = g_score[idx(&current.pos)];
        // Skip stale entries (we may push duplicates with worse f_scores).
        if current.f_score > current_g.saturating_add(heuristic(&current.pos, &to)) {
            continue;
        }

        for &(dx, dy) in &NEIGHBORS {
            let nx = current.pos.x() + dx;
            let ny = current.pos.y() + dy;
            if !map.in_bounds(nx, ny) {
                continue;
            }
            let terrain = map.get(nx, ny).terrain;
            if !terrain.is_passable() {
                continue;
            }
            let neighbor = Position::new(nx, ny);
            let ni = idx(&neighbor);
            let overlay_cost = sum_overlay_cost(overlays, neighbor);
            let tentative_g = current_g + terrain.movement_cost() + overlay_cost;
            if tentative_g < g_score[ni] {
                g_score[ni] = tentative_g;
                came_from[ni] = idx(&current.pos) as i32;
                open.push(Node {
                    pos: neighbor,
                    f_score: tentative_g + heuristic(&neighbor, &to),
                });
            }
        }
    }

    None // No path exists.
}

// ---------------------------------------------------------------------------
// Greedy step-toward pathfinding
// ---------------------------------------------------------------------------

/// Move one tile closer to `to` using greedy directional preference.
///
/// Considers candidates in directional order:
/// 1. Diagonal step (dx, dy)
/// 2. Horizontal step (dx, 0)
/// 3. Vertical step (0, dy)
///
/// At `overlays = &[]`, returns the **first passable** candidate in
/// directional order — bit-for-bit the legacy semantics from before the
/// `TileCostOverlay` substrate landed. This is the no-op-at-`&[]` invariant
/// promised by ticket 222.
///
/// At non-empty overlays, evaluates `terrain.movement_cost() +
/// sum(overlays)` per candidate and returns the **first** candidate with
/// the lowest cost (strict `<` comparison so direction order wins on ties).
/// **Do not relax this to `<=`** — that would silently invert tiebreak
/// order and perturb every cat's path on every tick.
///
/// Returns the next [`Position`] on success, or `None` if every candidate is
/// out-of-bounds or impassable (the entity is stuck).
///
/// This is intentionally simple — it is not A* and will get stuck in local
/// minima (e.g. concave obstacles). That is acceptable for Phase 1.
pub fn step_toward(
    from: &Position,
    to: &Position,
    map: &TileMap,
    overlays: &[WeightedOverlay<'_>],
) -> Option<Position> {
    if from == to {
        return None;
    }

    let dx = (to.x() - from.x()).signum();
    let dy = (to.y() - from.y()).signum();

    let candidates = [
        // Diagonal first
        (from.x() + dx, from.y() + dy),
        // Then cardinal
        (from.x() + dx, from.y()),
        (from.x(), from.y() + dy),
    ];

    if overlays.is_empty() {
        // Legacy: first passable candidate wins.
        for (nx, ny) in candidates {
            // Skip degenerate candidates that equal the current position
            // (happens when dx or dy is 0).
            if nx == from.x() && ny == from.y() {
                continue;
            }
            if map.in_bounds(nx, ny) && map.get(nx, ny).terrain.is_passable() {
                return Some(Position::new(nx, ny));
            }
        }
        return None;
    }

    // Non-empty overlays: pick lowest-cost passable candidate.
    let mut best: Option<(Position, u32)> = None;
    for (nx, ny) in candidates {
        if nx == from.x() && ny == from.y() {
            continue;
        }
        if !map.in_bounds(nx, ny) {
            continue;
        }
        let terrain = map.get(nx, ny).terrain;
        if !terrain.is_passable() {
            continue;
        }
        let candidate = Position::new(nx, ny);
        let cost = terrain.movement_cost() + sum_overlay_cost(overlays, candidate);
        // Strict `<` — first candidate in directional order wins on ties.
        if best.as_ref().is_none_or(|&(_, c)| cost < c) {
            best = Some((candidate, cost));
        }
    }

    best.map(|(p, _)| p)
}

// ---------------------------------------------------------------------------
// Anti-stacking: find a free adjacent tile
// ---------------------------------------------------------------------------

/// Find an unoccupied, passable tile at or adjacent to `target`.
///
/// Returns `target` itself if it is passable and unoccupied. Otherwise checks
/// the 8 neighbors, preferring whichever is closest (Chebyshev) to `hint` (the
/// approaching entity's current position) so the detour is minimal.
///
/// Returns `None` only when *all* 9 candidates are occupied or impassable.
pub fn find_free_adjacent(
    target: Position,
    hint: Position,
    map: &TileMap,
    occupied: &HashSet<Position>,
) -> Option<Position> {
    // Fast path: target itself is fine.
    if map.in_bounds(target.x(), target.y())
        && map.get(target.x(), target.y()).terrain.is_passable()
        && !occupied.contains(&target)
    {
        return Some(target);
    }

    // Check 8 neighbors, pick the one closest to hint.
    let mut best: Option<(Position, u32)> = None;
    for &(dx, dy) in &NEIGHBORS {
        let nx = target.x() + dx;
        let ny = target.y() + dy;
        if !map.in_bounds(nx, ny) {
            continue;
        }
        if !map.get(nx, ny).terrain.is_passable() {
            continue;
        }
        let candidate = Position::new(nx, ny);
        if occupied.contains(&candidate) {
            continue;
        }
        let dist = heuristic(&candidate, &hint); // Chebyshev
        if best.as_ref().is_none_or(|&(_, d)| dist < d) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(p, _)| p)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::{Terrain, TileMap};

    /// Helper: open 20×20 grass map.
    fn open_map() -> TileMap {
        TileMap::new(20, 20, Terrain::Grass)
    }

    /// step_toward on open terrain moves closer to the target.
    #[test]
    fn moves_closer_on_open_terrain() {
        let map = open_map();
        let from = Position::new(0, 0);
        let to = Position::new(5, 5);

        let next = step_toward(&from, &to, &map, &[]).expect("should move on open terrain");

        // Must be strictly closer in Manhattan distance
        let before = from.distance_to(&to);
        let after = next.distance_to(&to);
        assert!(
            after < before,
            "next position {next:?} is not closer to {to:?} than {from:?} (before={before}, after={after})"
        );
    }

    /// When diagonal is blocked by water, step_toward falls back to a cardinal direction.
    #[test]
    fn avoids_water_diagonal_tries_cardinal() {
        let mut map = open_map();

        // from=(0,0), to=(3,3)
        // Diagonal candidate is (1,1) — block it with water
        map.set(1, 1, Terrain::Water);

        let from = Position::new(0, 0);
        let to = Position::new(3, 3);

        let next = step_toward(&from, &to, &map, &[]).expect("should find a cardinal fallback");

        // Must not be the blocked diagonal
        assert_ne!(
            next,
            Position::new(1, 1),
            "stepped onto water tile at (1,1)"
        );

        // Must still be closer — measured radially (Euclidean).
        // Ticket 494 — under the realigned Chebyshev `distance_to`, a
        // cardinal step toward a pure-diagonal target doesn't strictly
        // decrease step-count (max(2,3) == max(3,3)), but it still makes
        // geometric progress. `euclidean_distance` is the right metric
        // for "did pathfinding move us toward the target in 2D space"
        // — the substrate's chosen cost metric (Chebyshev) shouldn't
        // bleed into a test of geometric direction.
        let before = from.euclidean_distance(&to);
        let after = next.euclidean_distance(&to);
        assert!(
            after < before,
            "fallback position {next:?} is not closer to {to:?}"
        );
    }

    /// When target is directly north and the vertical step is blocked,
    /// step_toward must return None — not the current position.
    #[test]
    fn returns_none_when_cardinal_blocked_and_axis_aligned() {
        let mut map = open_map();
        // from=(5,5), to=(5,0) — target is directly north (dx=0, dy=-1)
        // Block the only useful candidate (5,4) with water.
        map.set(5, 4, Terrain::Water);

        let from = Position::new(5, 5);
        let to = Position::new(5, 0);

        let result = step_toward(&from, &to, &map, &[]);
        assert!(
            result.is_none(),
            "expected None when only vertical candidate is blocked on axis-aligned path, got {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // A* find_path tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_path_open_terrain() {
        let map = open_map();
        let path = find_path(Position::new(0, 0), Position::new(5, 5), &map, &[])
            .expect("path should exist on open terrain");
        assert_eq!(*path.last().unwrap(), Position::new(5, 5));
        // Optimal diagonal path: 5 steps.
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn find_path_same_position() {
        let map = open_map();
        let path = find_path(Position::new(3, 3), Position::new(3, 3), &map, &[])
            .expect("same-position path should return empty vec");
        assert!(path.is_empty());
    }

    #[test]
    fn find_path_around_water_wall() {
        let mut map = open_map();
        // Build a vertical water wall at x=5, from y=0 to y=8.
        // Leave y=9 open as a gap.
        for y in 0..9 {
            map.set(5, y, Terrain::Water);
        }
        let from = Position::new(4, 4);
        let to = Position::new(6, 4);

        let path = find_path(from, to, &map, &[]).expect("should route around the wall");
        assert_eq!(*path.last().unwrap(), to);
        // Path must not cross any water tile.
        for p in &path {
            assert_ne!(
                map.get(p.x(), p.y()).terrain,
                Terrain::Water,
                "path crossed water at ({}, {})",
                p.x(),
                p.y()
            );
        }
    }

    #[test]
    fn find_path_unreachable() {
        let mut map = open_map();
        // Surround target (10,10) with water on all 8 sides.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                map.set(10 + dx, 10 + dy, Terrain::Water);
            }
        }
        let result = find_path(Position::new(0, 0), Position::new(10, 10), &map, &[]);
        assert!(result.is_none(), "path to surrounded tile should be None");
    }

    #[test]
    fn find_path_prefers_cheap_terrain() {
        let mut map = TileMap::new(10, 5, Terrain::Grass);
        // Fill a direct corridor (y=2) with DenseForest (cost 3 each).
        for x in 1..9 {
            map.set(x, 2, Terrain::DenseForest);
        }
        // There's a grass route above/below (cost 1 each).
        let from = Position::new(0, 2);
        let to = Position::new(9, 2);

        let path = find_path(from, to, &map, &[]).expect("path should exist");
        // Count how many DenseForest tiles the path crosses.
        let forest_tiles = path
            .iter()
            .filter(|p| map.get(p.x(), p.y()).terrain == Terrain::DenseForest)
            .count();
        // The optimal path should mostly avoid the forest corridor.
        assert!(
            forest_tiles <= 2,
            "path crossed {forest_tiles} forest tiles — should prefer the grass detour"
        );
    }

    #[test]
    fn find_path_to_impassable_target_returns_none() {
        let mut map = open_map();
        map.set(10, 10, Terrain::Water);
        let result = find_path(Position::new(0, 0), Position::new(10, 10), &map, &[]);
        assert!(result.is_none(), "path to impassable target should be None");
    }

    // -----------------------------------------------------------------------
    // TileCostOverlay tests (ticket 222 substrate)
    // -----------------------------------------------------------------------

    /// Test fixture: a sparse overlay that returns `cost` at one specific
    /// tile and `0` everywhere else.
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

    /// A high-cost overlay on a single tile in the direct path forces
    /// `find_path` to detour around it.
    #[test]
    fn find_path_overlay_forces_detour() {
        let map = open_map();
        let from = Position::new(0, 0);
        let to = Position::new(4, 0);

        // Direct optimal path on grass: through (1,0)..(3,0).
        // Plant a 50-cost overlay on (2,0). The 1-tile detour up through
        // (2,1) costs only ~1 extra grass step (cost 1) — vastly cheaper
        // than 50, so the path must avoid (2,0).
        let blocker = PointOverlay {
            target: Position::new(2, 0),
            cost: 50,
        };
        let overlays: [WeightedOverlay; 1] = [WeightedOverlay::new(&blocker, 1.0)];

        let path = find_path(from, to, &map, &overlays).expect("path should exist");
        assert_eq!(*path.last().unwrap(), to);
        assert!(
            !path.iter().any(|p| *p == Position::new(2, 0)),
            "path crossed the high-cost overlay tile (2,0): {path:?}"
        );
    }

    /// Empty overlay slice produces the same path as the legacy terrain-only
    /// pathfinder. Asserts the `&[]` short-circuit invariant.
    #[test]
    fn find_path_empty_overlay_matches_legacy() {
        let mut map = open_map();
        // Use the same wall scenario as `find_path_around_water_wall`.
        for y in 0..9 {
            map.set(5, y, Terrain::Water);
        }
        let from = Position::new(4, 4);
        let to = Position::new(6, 4);

        let path_a = find_path(from, to, &map, &[]).expect("path should exist");
        // Construct an overlay that always returns 0 — should be
        // indistinguishable from `&[]`.
        struct ZeroOverlay;
        impl TileCostOverlay for ZeroOverlay {
            fn cost_at(&self, _pos: Position) -> u32 {
                0
            }
        }
        let zero = ZeroOverlay;
        let overlays: [WeightedOverlay; 1] = [WeightedOverlay::new(&zero, 1.0)];
        let path_b = find_path(from, to, &map, &overlays).expect("path should exist");

        assert_eq!(
            path_a, path_b,
            "empty overlay slice and zero-cost overlay should produce the same path"
        );
    }

    /// At `&[]`, `step_toward` returns the first passable candidate in
    /// directional order — diagonal first, even when the diagonal is on
    /// expensive terrain (DenseForest=3) and the cardinal is on grass (1).
    /// This is the no-op-at-`&[]` invariant: the substrate refactor must
    /// not change fox movement (which always passes `&[]`) at terrain
    /// boundaries.
    #[test]
    fn step_toward_empty_overlay_preserves_direction_order() {
        let mut map = open_map();
        // From (0,0) toward (2,2): diagonal candidate is (1,1), horizontal
        // is (1,0). Make the diagonal expensive (DenseForest, cost 3) and
        // the horizontal cheap (Grass, cost 1).
        map.set(1, 1, Terrain::DenseForest);
        // (1,0) stays as Grass.

        let from = Position::new(0, 0);
        let to = Position::new(2, 2);

        let next = step_toward(&from, &to, &map, &[]).expect("should find a step");
        assert_eq!(
            next,
            Position::new(1, 1),
            "at &[] step_toward must pick the first passable candidate in \
             directional order (diagonal first), not the cheaper cardinal — \
             this preserves legacy fox-movement determinism. Got {next:?}"
        );
    }

    /// At non-empty overlays, `step_toward` picks the lowest-cost passable
    /// candidate, with strict-`<` so direction order wins on ties. With a
    /// per-tile overlay that makes the diagonal expensive, the cardinal
    /// becomes preferred.
    #[test]
    fn step_toward_overlay_prefers_cheaper_cardinal() {
        let map = open_map();
        let from = Position::new(0, 0);
        let to = Position::new(2, 2);

        // Plant a high overlay cost on the diagonal candidate (1,1).
        let blocker = PointOverlay {
            target: Position::new(1, 1),
            cost: 10,
        };
        let overlays: [WeightedOverlay; 1] = [WeightedOverlay::new(&blocker, 1.0)];

        let next = step_toward(&from, &to, &map, &overlays).expect("should find a step");
        assert_ne!(
            next,
            Position::new(1, 1),
            "step_toward with a high overlay on the diagonal must pick a \
             cardinal alternative, got {next:?}"
        );
    }

    // -----------------------------------------------------------------------
    // find_free_adjacent tests
    // -----------------------------------------------------------------------

    #[test]
    fn free_adjacent_returns_target_when_unoccupied() {
        let map = open_map();
        let occupied = HashSet::new();
        let result = find_free_adjacent(Position::new(5, 5), Position::new(0, 0), &map, &occupied);
        assert_eq!(result, Some(Position::new(5, 5)));
    }

    #[test]
    fn free_adjacent_jitters_when_target_occupied() {
        let map = open_map();
        let occupied: HashSet<Position> = [Position::new(5, 5)].into();
        let result = find_free_adjacent(Position::new(5, 5), Position::new(4, 5), &map, &occupied);
        let p = result.expect("should find a free neighbor");
        assert_ne!(
            p,
            Position::new(5, 5),
            "should not return the occupied tile"
        );
        // Must be adjacent to target.
        assert!(
            (p.x() - 5).abs() <= 1 && (p.y() - 5).abs() <= 1,
            "result {p:?} should be adjacent to (5,5)"
        );
    }

    #[test]
    fn free_adjacent_prefers_closer_to_hint() {
        let map = open_map();
        let occupied: HashSet<Position> = [Position::new(5, 5)].into();
        // Hint at (4, 5) — neighbor (4, 5) should be preferred (it's closest).
        let result = find_free_adjacent(Position::new(5, 5), Position::new(4, 5), &map, &occupied);
        assert_eq!(
            result,
            Some(Position::new(4, 5)),
            "should prefer the neighbor closest to hint"
        );
    }

    #[test]
    fn free_adjacent_returns_none_when_all_blocked() {
        let mut map = open_map();
        let target = Position::new(5, 5);
        // Block target + all 8 neighbors with water.
        for dy in -1..=1 {
            for dx in -1..=1 {
                map.set(5 + dx, 5 + dy, Terrain::Water);
            }
        }
        let occupied = HashSet::new();
        let result = find_free_adjacent(target, Position::new(3, 3), &map, &occupied);
        assert!(result.is_none(), "all tiles blocked — should return None");
    }

    #[test]
    fn free_adjacent_skips_impassable_neighbors() {
        let mut map = open_map();
        let target = Position::new(5, 5);
        // Occupy target, make most neighbors impassable.
        let occupied: HashSet<Position> = [target].into();
        for &(dx, dy) in &NEIGHBORS {
            if !(dx == 1 && dy == 0) {
                map.set(5 + dx, 5 + dy, Terrain::Water);
            }
        }
        let result = find_free_adjacent(target, Position::new(3, 3), &map, &occupied);
        assert_eq!(
            result,
            Some(Position::new(6, 5)),
            "only (6,5) should be passable and unoccupied"
        );
    }
}
