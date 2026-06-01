use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::resources::sim_constants::TremorConstants;

/// Spatial grid tracking the substrate-vibration field — the §5.6 tremor
/// channel as an `InfluenceMap`. Ticket 100.
///
/// Same shape as `PreyScentMap` (`marks` row-major over `grid_w × grid_h`
/// 3-tile buckets) but with markedly faster decay: scent persists for a
/// sim-day; tremor empties a full bucket in roughly 1–3 ticks. The
/// rationale lives in the design: vibration is a *behavioral* signal,
/// not a chemical residue. A running cat deposits heavily; a stalking
/// cat deposits nearly nothing; a stationary cat fades within a few
/// ticks.
///
/// Aggregate across species by construction — prey cannot discriminate
/// cat vibration from fox vibration. That's the ecological reality the
/// channel encodes; per-species splits don't help and would force the
/// detection layer to invent a non-existent classification ability.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TremorMap {
    /// Flat row-major grid of vibration intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl TremorMap {
    /// Build a tremor grid for a map of `map_w × map_h` tiles.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            marks: vec![0.0; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    /// Default grid sized for the standard 120×90 map with 3-tile
    /// buckets — matches `PreyScentMap::default_map()` so an emitter's
    /// vibration and scent share the same bucket alignment.
    pub fn default_map() -> Self {
        Self::new(120, 90, 3)
    }

    /// Convert a world position to a flat grid index.
    pub fn bucket_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let bx = (x / self.bucket_size) as usize;
        let by = (y / self.bucket_size) as usize;
        if bx >= self.grid_w || by >= self.grid_h {
            return None;
        }
        Some(by * self.grid_w + bx)
    }

    /// Get the vibration intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Deposit vibration at a world position, clamped to 1.0.
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Decay all marks by a fixed amount per tick.
    pub fn decay_all(&mut self, decay: f32) {
        for v in &mut self.marks {
            *v = (*v - decay).max(0.0);
        }
    }

    /// Find the highest-vibration bucket within manhattan `radius`.
    /// Returns the world-tile center of that bucket, or `None` if all
    /// nearby buckets are zero. Mirrors `PreyScentMap::highest_nearby`
    /// so prey AI can ask "where is vibration strongest near me?"
    /// without iterating cat entities.
    pub fn highest_nearby(&self, x: i32, y: i32, radius: i32) -> Option<(i32, i32)> {
        let mut best_val = 0.0f32;
        let mut best_pos = None;
        let bx_center = x / self.bucket_size;
        let by_center = y / self.bucket_size;
        let bucket_radius = radius / self.bucket_size + 1;
        for by in (by_center - bucket_radius)..=(by_center + bucket_radius) {
            for bx in (bx_center - bucket_radius)..=(bx_center + bucket_radius) {
                if bx < 0 || by < 0 {
                    continue;
                }
                let ubx = bx as usize;
                let uby = by as usize;
                if ubx >= self.grid_w || uby >= self.grid_h {
                    continue;
                }
                let idx = uby * self.grid_w + ubx;
                let val = self.marks[idx];
                let wx = bx * self.bucket_size + self.bucket_size / 2;
                let wy = by * self.bucket_size + self.bucket_size / 2;
                let dist = (wx - x).abs() + (wy - y).abs();
                if dist <= radius && val > best_val {
                    best_val = val;
                    best_pos = Some((wx, wy));
                }
            }
        }
        best_pos
    }

    /// Peak intensity within `radius` — the scalar version of
    /// `highest_nearby`. Used by `try_detect_cat` to gate
    /// `PreyAiState::Alert` against `TremorConstants::detect_threshold`.
    pub fn peak_nearby(&self, x: i32, y: i32, radius: i32) -> f32 {
        let mut best = 0.0f32;
        let bx_center = x / self.bucket_size;
        let by_center = y / self.bucket_size;
        let bucket_radius = radius / self.bucket_size + 1;
        for by in (by_center - bucket_radius)..=(by_center + bucket_radius) {
            for bx in (bx_center - bucket_radius)..=(bx_center + bucket_radius) {
                if bx < 0 || by < 0 {
                    continue;
                }
                let ubx = bx as usize;
                let uby = by as usize;
                if ubx >= self.grid_w || uby >= self.grid_h {
                    continue;
                }
                let idx = uby * self.grid_w + ubx;
                let wx = bx * self.bucket_size + self.bucket_size / 2;
                let wy = by * self.bucket_size + self.bucket_size / 2;
                let dist = (wx - x).abs() + (wy - y).abs();
                if dist <= radius && self.marks[idx] > best {
                    best = self.marks[idx];
                }
            }
        }
        best
    }
}

impl Default for TremorMap {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Map an `Action` to its vibration-emission multiplier. The actual
/// per-tick deposit is `signature.tremor_baseline × action_tremor_mul ×
/// constants.deposit_per_tick`, so this number controls how much of the
/// emitter's static body-mass baseline is converted into tile vibration
/// for the current behavior.
///
/// Exhaustive `match` — adding a new `Action` variant without
/// classifying its tremor emission is a compile error. That's the
/// silent-canary discipline from CLAUDE.md ("classifier functions whose
/// purpose is detecting silent failures … must not use catch-all
/// arms").
pub const fn action_tremor_mul(action: Action, c: &TremorConstants) -> f32 {
    match action {
        // 100: Stalk is the cat-flattens-and-creeps behavior — vibration
        // collapses by design. The whole point of the action.
        Action::Stalk => c.action_tremor_stalk,
        // 100: Pounce is the explosive spring — peak deposit. Prey
        // gets the spike, but pounce range is by construction inside
        // the terminal grab window.
        Action::Pounce => c.action_tremor_pounce,
        // Stationary / quiescent actions emit zero.
        Action::Sleep
        | Action::Idle
        | Action::Hide
        | Action::GroomSelf
        | Action::Vigil
        | Action::GriefSit => c.action_tremor_idle,
        // Combat: chaotic body motion. High but below pounce.
        Action::Fight => c.action_tremor_fight,
        // High-speed motion. Used for Flee and chase-phase Hunt
        // (set by `resolve_engage_prey` when phase = Chasing) — the
        // resolver leaves `current_action.action = Action::Hunt` for
        // chasing, so Hunt itself classifies as run-tier emission.
        Action::Flee | Action::Hunt => c.action_tremor_run,
        // Everything else is "walking around at normal pace" — the
        // baseline emission level. Includes movement-routine actions
        // (Wander/Explore/Patrol/Forage/travel-shaped sub-actions),
        // production actions tethered to a fixed spot (Cook/Bury/
        // HerbcraftGather/Build/Farm/etc.), and the resolver-phase
        // and HTN-stub variants whose vibration profile is mundane.
        Action::Eat
        | Action::Forage
        | Action::Wander
        | Action::Socialize
        | Action::GroomOther
        | Action::Explore
        | Action::Patrol
        | Action::Build
        | Action::Farm
        | Action::HerbcraftGather
        | Action::HerbcraftRemedy
        | Action::HerbcraftSetWard
        | Action::MagicScry
        | Action::MagicDurableWard
        | Action::MagicCleanse
        | Action::MagicColonyCleanse
        | Action::MagicHarvest
        | Action::MagicCommune
        | Action::Coordinate
        | Action::Mentor
        | Action::Mate
        | Action::Caretake
        | Action::Cook
        | Action::Drop
        | Action::Trash
        | Action::Handoff
        | Action::PickUp
        | Action::Bury
        | Action::WearItem
        | Action::Craft
        | Action::PetitionCoordinator
        | Action::ReleaseGrief
        | Action::Wean
        | Action::Teach
        | Action::Release
        | Action::DryFood
        | Action::SmokeMeat
        | Action::TendSmokingRack
        | Action::BegForFood => c.action_tremor_walk,
    }
}

// ---------------------------------------------------------------------------
// InfluenceMap impl
// ---------------------------------------------------------------------------

impl crate::systems::influence_map::InfluenceMap for TremorMap {
    fn metadata(&self) -> crate::systems::influence_map::MapMetadata {
        crate::systems::influence_map::MapMetadata {
            // 100: aggregate substrate vibration — `Faction::Neutral`
            // matches `CarcassScentMap` and the other faction-agnostic
            // perception fields. The L1 trace slug stays lowercase
            // kebab-style per §11.3 jq convention.
            name: "tremor",
            channel: crate::systems::sensing::ChannelKind::Tremor,
            faction: crate::systems::influence_map::Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: crate::components::physical::Position) -> f32 {
        self.get(pos.x(), pos.y())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::SimConstants;

    #[test]
    fn deposit_and_decay() {
        let mut map = TremorMap::new(30, 30, 3);
        map.deposit(6, 6, 0.5);
        assert!((map.get(6, 6) - 0.5).abs() < f32::EPSILON);
        map.decay_all(0.1);
        assert!((map.get(6, 6) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = TremorMap::new(30, 30, 3);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn decay_floors_at_zero() {
        let mut map = TremorMap::new(30, 30, 3);
        map.deposit(3, 3, 0.2);
        map.decay_all(0.5);
        assert_eq!(map.get(3, 3), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = TremorMap::new(30, 30, 3);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(1000, 5), 0.0);
    }

    #[test]
    fn highest_nearby_finds_strongest_bucket() {
        let mut map = TremorMap::new(30, 30, 3);
        map.deposit(10, 10, 0.9);
        map.deposit(0, 0, 0.3);
        let best = map.highest_nearby(8, 8, 5);
        assert!(best.is_some());
        let (wx, wy) = best.unwrap();
        assert!((wx - 10).abs() <= 3);
        assert!((wy - 10).abs() <= 3);
    }

    #[test]
    fn peak_nearby_returns_max_value() {
        let mut map = TremorMap::new(30, 30, 3);
        map.deposit(10, 10, 0.9);
        map.deposit(0, 0, 0.3);
        // Peak within radius 5 of (8, 8) → bucket at (10, 10).
        assert!((map.peak_nearby(8, 8, 5) - 0.9).abs() < 0.01);
        // Out-of-range distant deposit doesn't leak in.
        assert_eq!(map.peak_nearby(25, 25, 2), 0.0);
    }

    #[test]
    fn highest_nearby_returns_none_when_all_zero() {
        let map = TremorMap::new(30, 30, 3);
        assert!(map.highest_nearby(15, 15, 10).is_none());
    }

    #[test]
    fn action_tremor_mul_ordered_by_loudness() {
        // 100: action-emission ordering is the load-bearing invariant —
        // sleep ≤ stalk ≤ walk ≤ fight ≤ run ≤ pounce. If a future
        // tuning pass swaps any of these, this assertion catches it
        // before a soak.
        let c = SimConstants::default().tremor;
        assert!(c.action_tremor_idle <= c.action_tremor_stalk);
        assert!(c.action_tremor_stalk < c.action_tremor_walk);
        assert!(c.action_tremor_walk < c.action_tremor_fight);
        assert!(c.action_tremor_fight < c.action_tremor_run);
        assert!(c.action_tremor_run < c.action_tremor_pounce);

        // Dispatch sanity — Stalk dispatches to stalk multiplier and
        // Pounce dispatches to pounce multiplier (not run, not fight).
        assert!(
            (action_tremor_mul(Action::Stalk, &c) - c.action_tremor_stalk).abs() < f32::EPSILON
        );
        assert!(
            (action_tremor_mul(Action::Pounce, &c) - c.action_tremor_pounce).abs() < f32::EPSILON
        );
        assert!((action_tremor_mul(Action::Sleep, &c) - c.action_tremor_idle).abs() < f32::EPSILON);
        assert!(
            (action_tremor_mul(Action::Fight, &c) - c.action_tremor_fight).abs() < f32::EPSILON
        );
        assert!((action_tremor_mul(Action::Flee, &c) - c.action_tremor_run).abs() < f32::EPSILON);
        // Mundane action → walk tier.
        assert!((action_tremor_mul(Action::Eat, &c) - c.action_tremor_walk).abs() < f32::EPSILON);
    }
}
