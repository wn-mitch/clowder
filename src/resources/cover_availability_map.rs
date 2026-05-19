//! Tile-resolution boolean influence map: "is there a low-cover terrain
//! tile within sprint-radius of this position?"
//!
//! Retires the per-cat O(radius²) Chebyshev disc scan that ticket 170's
//! `update_hide_eligible_markers` shipped as the v1 HideEligible author
//! (ticket 423). Cover availability is a terrain property — two cats at
//! the same tile see identical cover availability — so per-cat scanning
//! recomputes colony-wide-identical information once per cat per tick.
//! Influence-map family precedent: `CarcassScentMap`, `ExplorationMap`,
//! `FoxScentMap`, etc.
//!
//! **Rebuild cadence — dirty-flag.** Terrain mutates rarely (building
//! construction completion, magic remedies). The map carries a `dirty`
//! flag set on construction (so the first tick rebuilds) and re-set by
//! `mark_dirty()` at every terrain-mutation site. The `update_cover_
//! availability_map` system rebuilds only when `dirty` — steady-state
//! ticks pay zero cost.
//!
//! **Cell semantic.** `1.0` if any tile within Chebyshev distance
//! `sprint_radius` is `Terrain::is_low_cover()`; `0.0` otherwise. The
//! HideEligible author reads `map.get(pos.x, pos.y) > threshold` where
//! threshold defaults to 0.5 (the boolean midpoint). v1 ships boolean;
//! a distance-gradient variant is owned by the 170 balance follow-on.

use bevy_ecs::prelude::*;

use crate::resources::map::TileMap;

/// Tile-resolution boolean map flagging tiles within sprint-radius of
/// any `Terrain::is_low_cover()` tile. Rebuilt lazily via the `dirty`
/// flag.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoverAvailabilityMap {
    /// Flat row-major grid of cell values in `{0.0, 1.0}`.
    marks: Vec<f32>,
    width: i32,
    height: i32,
    /// `true` ⇒ next `update_cover_availability_map` tick rebuilds and
    /// clears. Initial: `true` (cold-start rebuild covers the worldgen-
    /// established terrain).
    dirty: bool,
}

impl CoverAvailabilityMap {
    /// Build an empty map sized for a `width × height` tile grid.
    /// Starts dirty so the first scheduler tick performs the cold-start
    /// stamp without requiring callers to remember.
    pub fn new(width: i32, height: i32) -> Self {
        let len = (width as usize) * (height as usize);
        Self {
            marks: vec![0.0; len],
            width,
            height,
            dirty: true,
        }
    }

    /// Map sized for the canonical 120×90 world; ergonomic default for
    /// resource insertion at startup.
    pub fn default_map() -> Self {
        Self::new(120, 90)
    }

    /// Mark the map as needing a rebuild on the next tick. Callers:
    /// every terrain-mutation site (building completion, magic remedy,
    /// any future fire/weather terrain change). Missing a caller
    /// silently lags `HideEligible` by one mutation event until another
    /// caller fires.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `true` if a rebuild is pending. Read by `update_cover_
    /// availability_map`; consumers should not need this.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Sample at a world position. Returns `0.0` for out-of-bounds
    /// (consistent with `CarcassScentMap::get`).
    pub fn get(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return 0.0;
        }
        self.marks[(y * self.width + x) as usize]
    }

    /// Rebuild the map from the current `TileMap`, stamping outward
    /// from every `is_low_cover()` tile by `sprint_radius`. Adopts
    /// `TileMap`'s dimensions if they've drifted (scenarios use smaller
    /// maps than the canonical 120×90 default). Clears the `dirty`
    /// flag. Cost: O(low_cover_tile_count × sprint_radius²).
    pub fn rebuild(&mut self, map: &TileMap, sprint_radius: i32) {
        let w = map.width;
        let h = map.height;
        let len = (w as usize) * (h as usize);
        if self.width != w || self.height != h || self.marks.len() != len {
            self.marks = vec![0.0; len];
            self.width = w;
            self.height = h;
        } else {
            self.marks.fill(0.0);
        }
        if sprint_radius < 0 {
            self.dirty = false;
            return;
        }
        for ty in 0..h {
            for tx in 0..w {
                if !map.get(tx, ty).terrain.is_low_cover() {
                    continue;
                }
                // Stamp the (2·sprint_radius + 1)² Chebyshev disc
                // centered on (tx, ty). The mark mirrors the predicate
                // the v1 disc scan computed per-cat — a cat at
                // (tx + dx, ty + dy) sees this tile within sprint_radius
                // iff |dx| ≤ R AND |dy| ≤ R.
                let xmin = (tx - sprint_radius).max(0);
                let xmax = (tx + sprint_radius).min(w - 1);
                let ymin = (ty - sprint_radius).max(0);
                let ymax = (ty + sprint_radius).min(h - 1);
                for sy in ymin..=ymax {
                    for sx in xmin..=xmax {
                        self.marks[(sy * w + sx) as usize] = 1.0;
                    }
                }
            }
        }
        self.dirty = false;
    }

    /// Width in tiles — for tests that want to assert grid shape.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Height in tiles.
    pub fn height(&self) -> i32 {
        self.height
    }
}

impl Default for CoverAvailabilityMap {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Bevy system: rebuild on the dirty flag. Registered in Chain 2a
/// before `update_hide_eligible_markers` so the marker author reads
/// fresh data on the same tick as terrain mutations propagated.
pub fn update_cover_availability_map(
    mut cover_map: ResMut<CoverAvailabilityMap>,
    tile_map: Res<TileMap>,
    constants: Res<crate::resources::sim_constants::SimConstants>,
) {
    if !cover_map.is_dirty() {
        return;
    }
    let sprint_radius = constants.escape_viability.sprint_radius;
    cover_map.rebuild(&tile_map, sprint_radius);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;

    fn make_map(w: i32, h: i32, fill: Terrain) -> TileMap {
        TileMap::new(w, h, fill)
    }

    #[test]
    fn cold_start_is_dirty() {
        let cover = CoverAvailabilityMap::new(10, 10);
        assert!(cover.is_dirty());
        assert_eq!(cover.width(), 10);
        assert_eq!(cover.height(), 10);
    }

    #[test]
    fn rebuild_clears_dirty_flag() {
        let mut cover = CoverAvailabilityMap::new(10, 10);
        let map = make_map(10, 10, Terrain::Grass);
        cover.rebuild(&map, 3);
        assert!(!cover.is_dirty());
    }

    #[test]
    fn mark_dirty_re_enables_rebuild() {
        let mut cover = CoverAvailabilityMap::new(10, 10);
        let map = make_map(10, 10, Terrain::Grass);
        cover.rebuild(&map, 3);
        assert!(!cover.is_dirty());
        cover.mark_dirty();
        assert!(cover.is_dirty());
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let cover = CoverAvailabilityMap::new(10, 10);
        assert_eq!(cover.get(-1, 5), 0.0);
        assert_eq!(cover.get(5, -1), 0.0);
        assert_eq!(cover.get(10, 5), 0.0);
        assert_eq!(cover.get(5, 10), 0.0);
    }

    #[test]
    fn no_low_cover_yields_zero_everywhere() {
        let mut cover = CoverAvailabilityMap::new(10, 10);
        let map = make_map(10, 10, Terrain::Grass);
        cover.rebuild(&map, 3);
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(cover.get(x, y), 0.0, "expected 0.0 at ({x},{y})");
            }
        }
    }

    #[test]
    fn light_forest_stamps_neighborhood() {
        let mut map = make_map(10, 10, Terrain::Grass);
        map.set(5, 5, Terrain::LightForest);
        let mut cover = CoverAvailabilityMap::new(10, 10);
        cover.rebuild(&map, 3);

        // Cell at (5, 5) itself sees cover.
        assert_eq!(cover.get(5, 5), 1.0);
        // Corner of the 3-disc.
        assert_eq!(cover.get(2, 2), 1.0);
        assert_eq!(cover.get(8, 8), 1.0);
        // One step outside the 3-disc.
        assert_eq!(cover.get(1, 5), 0.0);
        assert_eq!(cover.get(9, 5), 0.0);
        assert_eq!(cover.get(5, 1), 0.0);
        assert_eq!(cover.get(5, 9), 0.0);
    }

    #[test]
    fn map_boundary_does_not_panic() {
        let mut map = make_map(10, 10, Terrain::Grass);
        // Low-cover tile at (0, 0) — stamp would underflow if not clamped.
        map.set(0, 0, Terrain::LightForest);
        let mut cover = CoverAvailabilityMap::new(10, 10);
        cover.rebuild(&map, 5);
        assert_eq!(cover.get(0, 0), 1.0);
        assert_eq!(cover.get(5, 5), 1.0); // edge of disc
        assert_eq!(cover.get(6, 6), 0.0);
    }

    #[test]
    fn all_qualifying_terrains_stamp() {
        // is_low_cover() = passable && shelter > 0 && !occludes_sight.
        // Qualifying terrains: LightForest, AncientRuin, Den, Hearth,
        // Stores, Workshop, Watchtower.
        for terrain in [
            Terrain::LightForest,
            Terrain::AncientRuin,
            Terrain::Den,
            Terrain::Hearth,
            Terrain::Stores,
            Terrain::Workshop,
            Terrain::Watchtower,
        ] {
            assert!(terrain.is_low_cover(), "{terrain:?} should be low-cover");
            let mut map = make_map(10, 10, Terrain::Grass);
            map.set(5, 5, terrain);
            let mut cover = CoverAvailabilityMap::new(10, 10);
            cover.rebuild(&map, 1);
            assert_eq!(
                cover.get(5, 5),
                1.0,
                "{terrain:?} should stamp its own tile"
            );
        }
    }

    #[test]
    fn dense_forest_does_not_stamp() {
        // DenseForest is shelter-rich but occludes_sight — disqualified
        // by is_low_cover(). The "low" in "low cover" means sight passes
        // through (cat hides but can still see).
        assert!(!Terrain::DenseForest.is_low_cover());
        let mut map = make_map(10, 10, Terrain::Grass);
        map.set(5, 5, Terrain::DenseForest);
        let mut cover = CoverAvailabilityMap::new(10, 10);
        cover.rebuild(&map, 3);
        assert_eq!(cover.get(5, 5), 0.0);
    }
}
