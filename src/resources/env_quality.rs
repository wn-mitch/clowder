//! Ticket 101 — five tile-resolution influence maps for ambient spatial
//! pressure: **comfort, cleanliness, beauty, mystery, corruption**.
//!
//! Each map is a flat row-major `Vec<f32>` at one cell per tile
//! (`bucket_size = 1`). Sources stamp influence outward with signed
//! linear falloff; cats sample their tile position as `EvalInput`
//! scalars (`local_comfort` … `local_corruption`) and those scalars
//! thread through the IAUS modifier pipeline.
//!
//! Unlike `CarcassScentMap` / `TremorMap` (positive-only, deposit-and-
//! decay), env-quality cells carry **signed** intensities in
//! `[-1.0, 1.0]` — corpses push cleanliness negative, ruins push
//! beauty negative. The shared [`stamp`] helper applies a linear
//! falloff (`peak × (1 - dist / radius).max(0)`), adds onto the
//! existing cell value, and clamps to `[-1.0, 1.0]`.
//!
//! Corruption gets its own influence map for spatial perception only —
//! the magic system's `corruption_tile_effects` (mood + health drain
//! on hot tiles) stays unchanged. The map exists so a future DSE or
//! consideration can read the corruption gradient before a cat steps
//! onto the threshold tile.

use bevy_ecs::prelude::*;

use crate::components::personality::Personality;
use crate::resources::sim_constants::EnvironmentalQualityConstants;

/// Flat tile-resolution field shared by the five env-quality maps.
/// Held as `pub field: EnvField` inside each map struct so the
/// trait-impls (InfluenceMap, Default) can be distinct per type while
/// the storage + accessor code is authored once.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvField {
    /// Row-major `[-1.0, 1.0]` intensity per tile.
    pub marks: Vec<f32>,
    pub width: usize,
    pub height: usize,
}

impl EnvField {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            marks: vec![0.0; width * height],
            width,
            height,
        }
    }

    /// Default field sized for the canonical 120×90 world map.
    pub fn default_map() -> Self {
        Self::new(120, 90)
    }

    /// Flat index for a world tile, or `None` if OOB.
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let ux = x as usize;
        let uy = y as usize;
        if ux >= self.width || uy >= self.height {
            return None;
        }
        Some(uy * self.width + ux)
    }

    /// Sample at a world position. Returns `0.0` for OOB.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.index(x, y).map(|i| self.marks[i]).unwrap_or(0.0)
    }

    /// Zero every cell — called once per sweep before re-stamping.
    pub fn clear(&mut self) {
        self.marks.fill(0.0);
    }

    /// Apply a flat additive offset to every cell, then clamp.
    /// Used for the weather overlay on the comfort map.
    pub fn add_global(&mut self, delta: f32) {
        for v in &mut self.marks {
            *v = (*v + delta).clamp(-1.0, 1.0);
        }
    }

    /// Final clamp pass — every cell to `[-1.0, 1.0]`. Called after
    /// stamping completes so additive overlap can be detected without
    /// clamping per-stamp (the precedent saves clamp() per cell, not
    /// per stamp).
    pub fn clamp_all(&mut self) {
        for v in &mut self.marks {
            *v = v.clamp(-1.0, 1.0);
        }
    }
}

impl Default for EnvField {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Stamp a radial signed contribution into a field.
///
/// `peak` is the value at `(cx, cy)`; falloff is **linear** to zero at
/// `radius` (manhattan distance). `radius == 0` is a single-cell stamp
/// (used by the terrain pass which writes bare on-tile contributions).
/// Contributions are **additive** — two overlapping stamps sum. Cells
/// are clamped to `[-1.0, 1.0]` at write time so intermediate values
/// never overflow during a multi-source sweep.
pub fn stamp(field: &mut EnvField, cx: i32, cy: i32, peak: f32, radius: f32) {
    if peak == 0.0 {
        return;
    }
    if radius <= 0.0 {
        if let Some(i) = field.index(cx, cy) {
            field.marks[i] = (field.marks[i] + peak).clamp(-1.0, 1.0);
        }
        return;
    }
    let r = radius.round() as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let dist = dx.abs() + dy.abs();
            if dist > r {
                continue;
            }
            let falloff = 1.0 - (dist as f32) / (r as f32);
            let contribution = peak * falloff;
            if contribution == 0.0 {
                continue;
            }
            if let Some(i) = field.index(cx + dx, cy + dy) {
                field.marks[i] = (field.marks[i] + contribution).clamp(-1.0, 1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The five map resources
// ---------------------------------------------------------------------------

macro_rules! env_quality_map {
    ($Name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $Name {
            pub field: EnvField,
        }

        impl $Name {
            pub fn new(width: usize, height: usize) -> Self {
                Self {
                    field: EnvField::new(width, height),
                }
            }

            pub fn default_map() -> Self {
                Self {
                    field: EnvField::default_map(),
                }
            }

            pub fn get(&self, x: i32, y: i32) -> f32 {
                self.field.get(x, y)
            }

            pub fn clear(&mut self) {
                self.field.clear();
            }
        }

        impl Default for $Name {
            fn default() -> Self {
                Self::default_map()
            }
        }
    };
}

env_quality_map!(
    ComfortMap,
    "101 — comfort influence map. Stamped from terrain ease, building \
     proximity scaled by `structure.condition`, and a global weather \
     overlay (`Weather::comfort_modifier()`). Sampled by cats as the \
     `local_comfort` scalar and combined with the personality axes \
     `warmth` and `(1 − independence)` in `EnvironmentalQualityModifier`."
);

env_quality_map!(
    CleanlinessMap,
    "101 — cleanliness influence map. Negative stamps from unburied \
     `Dead` entities and dirty buildings (`structure.cleanliness < \
     buildings.dirty_threshold`). Mud terrain pushes the on-tile cell \
     negative. Sampled as `local_cleanliness` and scaled by the \
     `anxiety` personality axis."
);

env_quality_map!(
    BeautyMap,
    "101 — beauty influence map. Positive stamps from FairyRing, \
     Garden, StandingStone, DeepPool, plus aesthetic upkeep on \
     well-conditioned Den / Hearth. Negative on AncientRuin. \
     Suppressed by `tile.corruption` on-tile. Sampled as \
     `local_beauty` and scaled by the `spirituality` axis."
);

env_quality_map!(
    MysteryMap,
    "101 — mystery influence map. Sources are `Tile.mystery` values \
     already seeded at world-gen (FairyRing, StandingStone, DeepPool, \
     AncientRuin). Stamped outward with a short falloff so adjacent \
     tiles feel the resonance. Sampled as `local_mystery` and scaled \
     by the `curiosity` axis."
);

env_quality_map!(
    CorruptionInfluenceMap,
    "101 — corruption influence map. Stamps `Tile.corruption` outward \
     with a 3-tile default radius so cats can perceive the gradient \
     before crossing the threshold. The magic system's behavioral \
     response (mood drain, health drain via `corruption_tile_effects`) \
     stays unchanged — this map is pure spatial perception, sampled \
     as `local_corruption`. Not consumed by `EnvironmentalQualityModifier`."
);

// ---------------------------------------------------------------------------
// Combined modifier value — shared by EnvironmentalQualityModifier and the
// feature-emit system so both code paths run the same arithmetic.
// ---------------------------------------------------------------------------

/// Combine the four mood-relevant axes (comfort, cleanliness, beauty,
/// mystery — not corruption) into a single additive modifier value.
///
/// Each axis is personality-scaled (warmth + (1 − independence) for
/// comfort; anxiety amplifies cleanliness response; spirituality scales
/// beauty; curiosity scales mystery), summed, weighted, and clamped to
/// `[constants.combined_min, constants.combined_max]` (default
/// `[-0.3, 0.3]`).
///
/// `local_corruption` is intentionally excluded — the magic system owns
/// the response to corruption, not this modifier.
pub fn combined_env_quality(
    local_comfort: f32,
    local_cleanliness: f32,
    local_beauty: f32,
    local_mystery: f32,
    personality: &Personality,
    constants: &EnvironmentalQualityConstants,
) -> f32 {
    let comfort_contrib = local_comfort
        * (1.0 + personality.warmth * constants.warmth_bonus)
        * (1.0 - personality.independence * constants.independence_dampen);
    let cleanliness_contrib =
        local_cleanliness * (1.0 + personality.anxiety * constants.anxiety_bonus);
    let beauty_contrib =
        local_beauty * (1.0 + personality.spirituality * constants.spirituality_bonus);
    let mystery_contrib = local_mystery * (1.0 + personality.curiosity * constants.curiosity_bonus);

    let combined = (comfort_contrib + cleanliness_contrib + beauty_contrib + mystery_contrib)
        * constants.combination_weight;
    combined.clamp(constants.combined_min, constants.combined_max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::SimConstants;

    fn neutral_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    #[test]
    fn stamp_at_source_uses_peak() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 10, 10, 0.6, 3.0);
        assert!((f.get(10, 10) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn stamp_linear_falloff() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 10, 10, 1.0, 4.0);
        // Manhattan distance 0 → 1.0
        assert!((f.get(10, 10) - 1.0).abs() < 1e-6);
        // Manhattan distance 1 → 0.75
        assert!((f.get(11, 10) - 0.75).abs() < 1e-6);
        assert!((f.get(10, 11) - 0.75).abs() < 1e-6);
        // Manhattan distance 2 → 0.5
        assert!((f.get(12, 10) - 0.5).abs() < 1e-6);
        // Manhattan distance 4 → 0.0 (radius edge — falloff zeroes it)
        assert_eq!(f.get(14, 10), 0.0);
        // Beyond radius
        assert_eq!(f.get(15, 10), 0.0);
    }

    #[test]
    fn stamp_zero_radius_is_single_cell() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 5, 5, 0.4, 0.0);
        assert!((f.get(5, 5) - 0.4).abs() < 1e-6);
        assert_eq!(f.get(6, 5), 0.0);
        assert_eq!(f.get(4, 5), 0.0);
    }

    #[test]
    fn stamp_additive_overlap_sums() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 10, 10, 0.3, 2.0);
        stamp(&mut f, 11, 10, 0.3, 2.0);
        // Both stamps contribute at (10,10): 0.3 (peak from first) +
        // 0.3 * 0.5 (one step away in second) = 0.45.
        assert!((f.get(10, 10) - 0.45).abs() < 1e-6);
        // Symmetric at (11,10): same total.
        assert!((f.get(11, 10) - 0.45).abs() < 1e-6);
    }

    #[test]
    fn stamp_signed_negative_peak_stamps_negative() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 5, 5, -0.4, 2.0);
        assert!((f.get(5, 5) + 0.4).abs() < 1e-6);
        assert!((f.get(6, 5) + 0.2).abs() < 1e-6);
    }

    #[test]
    fn stamp_clamps_at_one() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 5, 5, 0.8, 0.0);
        stamp(&mut f, 5, 5, 0.8, 0.0);
        assert!((f.get(5, 5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn stamp_clamps_at_negative_one() {
        let mut f = EnvField::new(30, 30);
        stamp(&mut f, 5, 5, -0.8, 0.0);
        stamp(&mut f, 5, 5, -0.8, 0.0);
        assert!((f.get(5, 5) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn get_out_of_bounds_returns_zero() {
        let f = EnvField::new(30, 30);
        assert_eq!(f.get(-1, 5), 0.0);
        assert_eq!(f.get(5, -1), 0.0);
        assert_eq!(f.get(30, 5), 0.0);
        assert_eq!(f.get(5, 30), 0.0);
        assert_eq!(f.get(9999, 9999), 0.0);
    }

    #[test]
    fn clear_zeroes_every_cell() {
        let mut f = EnvField::new(10, 10);
        stamp(&mut f, 5, 5, 0.5, 3.0);
        f.clear();
        for v in &f.marks {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn add_global_shifts_every_cell_uniformly() {
        let mut f = EnvField::new(10, 10);
        stamp(&mut f, 5, 5, 0.2, 0.0);
        f.add_global(0.1);
        // The stamped cell shifted from 0.2 to 0.3.
        assert!((f.get(5, 5) - 0.3).abs() < 1e-6);
        // An untouched cell shifted from 0.0 to 0.1.
        assert!((f.get(0, 0) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn add_global_clamps() {
        let mut f = EnvField::new(10, 10);
        stamp(&mut f, 5, 5, 0.95, 0.0);
        f.add_global(0.5);
        // Clamped to 1.0
        assert!((f.get(5, 5) - 1.0).abs() < 1e-6);
        f.add_global(-2.0);
        // Clamped to -1.0
        assert!((f.get(5, 5) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn comfort_map_default_sized_for_world() {
        let m = ComfortMap::default_map();
        assert_eq!(m.field.width, 120);
        assert_eq!(m.field.height, 90);
        assert_eq!(m.field.marks.len(), 120 * 90);
    }

    #[test]
    fn combine_neutral_personality_matches_combination_weight() {
        let constants = SimConstants::default().environmental_quality;
        let p = neutral_personality();
        let combined = combined_env_quality(0.0, 0.0, 0.0, 0.0, &p, &constants);
        assert_eq!(combined, 0.0);
    }

    #[test]
    fn combine_high_warmth_amplifies_comfort() {
        let constants = SimConstants::default().environmental_quality;
        let mut high = neutral_personality();
        high.warmth = 1.0;
        let mut low = neutral_personality();
        low.warmth = 0.0;
        let combined_high = combined_env_quality(0.5, 0.0, 0.0, 0.0, &high, &constants);
        let combined_low = combined_env_quality(0.5, 0.0, 0.0, 0.0, &low, &constants);
        assert!(combined_high > combined_low);
    }

    #[test]
    fn combine_high_anxiety_amplifies_cleanliness_penalty() {
        let constants = SimConstants::default().environmental_quality;
        let mut high = neutral_personality();
        high.anxiety = 1.0;
        let mut low = neutral_personality();
        low.anxiety = 0.0;
        // Negative cleanliness — a corpse nearby. High anxiety should
        // produce a *more negative* combined value.
        let combined_high = combined_env_quality(0.0, -0.5, 0.0, 0.0, &high, &constants);
        let combined_low = combined_env_quality(0.0, -0.5, 0.0, 0.0, &low, &constants);
        assert!(combined_high < combined_low);
    }

    #[test]
    fn combine_high_curiosity_amplifies_mystery_lift() {
        let constants = SimConstants::default().environmental_quality;
        let mut high = neutral_personality();
        high.curiosity = 1.0;
        let mut low = neutral_personality();
        low.curiosity = 0.0;
        let combined_high = combined_env_quality(0.0, 0.0, 0.0, 0.4, &high, &constants);
        let combined_low = combined_env_quality(0.0, 0.0, 0.0, 0.4, &low, &constants);
        assert!(combined_high > combined_low);
    }

    #[test]
    fn combine_clamps_to_configured_bounds() {
        let constants = SimConstants::default().environmental_quality;
        let p = neutral_personality();
        // Saturate every positive axis to push beyond the upper clamp.
        let combined_pos = combined_env_quality(1.0, 1.0, 1.0, 1.0, &p, &constants);
        assert!(combined_pos <= constants.combined_max + 1e-6);
        let combined_neg = combined_env_quality(-1.0, -1.0, -1.0, -1.0, &p, &constants);
        assert!(combined_neg >= constants.combined_min - 1e-6);
    }
}
