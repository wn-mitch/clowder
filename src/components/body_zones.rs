use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Body zones — Ticket 095 Phase 1 (Cat)
//
// Spec: docs/systems/body-zones.md. Phase 1 lands the substrate for cats
// (13 named parts, 5-tier PartCondition). Predator and prey models arrive in
// Phase 2/3 (separate tickets). Plan: ~/.claude/plans/it-s-time-to-start-lively-wilkinson.md.
//
// The `CatBodyModel` is introduced co-resident with `Health` during Stage A
// (shadow). Stage B retires `Health.injuries` and switches readers to
// `health_derived()`.
// ---------------------------------------------------------------------------

/// 13 named anatomical parts on the cat body. Discriminant order is the
/// canonical array index for `BodyZoneConstants::pain_weights` and
/// `permanent_at_destroyed`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BodyPart {
    Whiskers = 0,
    Ears = 1,
    MouthJaw = 2,
    Scruff = 3,
    Throat = 4,
    Flanks = 5,
    Belly = 6,
    FrontLeftPaw = 7,
    FrontRightPaw = 8,
    RearLeftPaw = 9,
    RearRightPaw = 10,
    Haunches = 11,
    Tail = 12,
}

pub const CAT_BODY_PART_COUNT: usize = 13;

impl BodyPart {
    pub const ALL: [BodyPart; CAT_BODY_PART_COUNT] = [
        BodyPart::Whiskers,
        BodyPart::Ears,
        BodyPart::MouthJaw,
        BodyPart::Scruff,
        BodyPart::Throat,
        BodyPart::Flanks,
        BodyPart::Belly,
        BodyPart::FrontLeftPaw,
        BodyPart::FrontRightPaw,
        BodyPart::RearLeftPaw,
        BodyPart::RearRightPaw,
        BodyPart::Haunches,
        BodyPart::Tail,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<BodyPart> {
        Self::ALL.get(i).copied()
    }

    pub fn category(self) -> PartCategory {
        match self {
            BodyPart::Whiskers | BodyPart::MouthJaw => PartCategory::Sensory,
            BodyPart::Ears | BodyPart::Belly | BodyPart::Scruff => PartCategory::SoftTissue,
            BodyPart::Throat => PartCategory::Throat,
            BodyPart::Flanks
            | BodyPart::Haunches
            | BodyPart::FrontLeftPaw
            | BodyPart::FrontRightPaw
            | BodyPart::RearLeftPaw
            | BodyPart::RearRightPaw => PartCategory::Structural,
            BodyPart::Tail => PartCategory::Tail,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            BodyPart::Whiskers => "whiskers",
            BodyPart::Ears => "ears",
            BodyPart::MouthJaw => "mouth/jaw",
            BodyPart::Scruff => "scruff",
            BodyPart::Throat => "throat",
            BodyPart::Flanks => "flanks",
            BodyPart::Belly => "belly",
            BodyPart::FrontLeftPaw => "front-left paw",
            BodyPart::FrontRightPaw => "front-right paw",
            BodyPart::RearLeftPaw => "rear-left paw",
            BodyPart::RearRightPaw => "rear-right paw",
            BodyPart::Haunches => "haunches",
            BodyPart::Tail => "tail",
        }
    }
}

/// Healing-rate category. Spec §Cat Healing Rates (`docs/systems/body-zones.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PartCategory {
    SoftTissue,
    Structural,
    Sensory,
    Throat,
    Tail,
}

/// Functional condition tier of a body part. `tissue_damage` decides which
/// tier; thresholds live in `BodyZoneConstants::condition_thresholds`.
#[repr(u8)]
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum PartCondition {
    #[default]
    Healthy = 0,
    Bruised = 1,
    Wounded = 2,
    Mangled = 3,
    Destroyed = 4,
}

impl PartCondition {
    /// Derive condition from tissue damage and threshold lower bounds.
    /// `thresholds[0]` = Bruised lower bound, `[1]` = Wounded, `[2]` = Mangled,
    /// `[3]` = Destroyed. Values are continuous in `[0.0, 1.0]`.
    pub fn from_tissue_damage(tissue_damage: f32, thresholds: &[f32; 4]) -> Self {
        if tissue_damage >= thresholds[3] {
            PartCondition::Destroyed
        } else if tissue_damage >= thresholds[2] {
            PartCondition::Mangled
        } else if tissue_damage >= thresholds[1] {
            PartCondition::Wounded
        } else if tissue_damage >= thresholds[0] {
            PartCondition::Bruised
        } else {
            PartCondition::Healthy
        }
    }

    pub fn step_down(self) -> Self {
        match self {
            PartCondition::Healthy => PartCondition::Healthy,
            PartCondition::Bruised => PartCondition::Healthy,
            PartCondition::Wounded => PartCondition::Bruised,
            PartCondition::Mangled => PartCondition::Wounded,
            PartCondition::Destroyed => PartCondition::Mangled,
        }
    }
}

/// Per-part state.
///
/// `tissue_damage` is continuous in `[0.0, 1.0]`. `condition` is the derived
/// tier (kept materialized to avoid recomputing on every read). `permanent`
/// flags a part whose Destroyed condition does not heal — set at the moment
/// the part first reaches Destroyed if its category is configured permanent
/// in `BodyZoneConstants::permanent_at_destroyed`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BodyPartState {
    pub tissue_damage: f32,
    pub condition: PartCondition,
    pub permanent: bool,
}

impl Default for BodyPartState {
    fn default() -> Self {
        Self {
            tissue_damage: 0.0,
            condition: PartCondition::Healthy,
            permanent: false,
        }
    }
}

/// Anatomical body model for a cat (13 parts). Replaces
/// `Health.injuries: Vec<Injury>` as the canonical injury substrate; ships
/// co-resident with `Health` during Stage A and becomes sole source of truth
/// at Stage B cutover.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CatBodyModel {
    pub parts: [BodyPartState; CAT_BODY_PART_COUNT],
}

impl Default for CatBodyModel {
    fn default() -> Self {
        Self {
            parts: std::array::from_fn(|_| BodyPartState::default()),
        }
    }
}

impl CatBodyModel {
    pub fn part(&self, p: BodyPart) -> &BodyPartState {
        &self.parts[p.index()]
    }

    pub fn part_mut(&mut self, p: BodyPart) -> &mut BodyPartState {
        &mut self.parts[p.index()]
    }

    /// Sum of `tissue_damage * pain_weight` across all parts. Compare against
    /// `pain_incapacitation_threshold` (spec §Cat Pain System default 0.9) and
    /// against `max_possible_pain` for the normalized `health_derived`.
    pub fn total_pain(&self, weights: &[f32; CAT_BODY_PART_COUNT]) -> f32 {
        self.parts
            .iter()
            .zip(weights.iter())
            .map(|(part, w)| part.tissue_damage * *w)
            .sum()
    }

    /// `1.0 - total_pain / max_possible_pain`, clamped to `[0.0, 1.0]`.
    /// `max_possible_pain` is the sum of pain weights — invariant for a given
    /// constants set, so caller passes it in cached. Equivalent to spec
    /// §Shared Formulas `health_derived`.
    pub fn health_derived(
        &self,
        weights: &[f32; CAT_BODY_PART_COUNT],
        max_possible_pain: f32,
    ) -> f32 {
        if max_possible_pain <= 0.0 {
            return 1.0;
        }
        let pain_fraction = (self.total_pain(weights) / max_possible_pain).clamp(0.0, 1.0);
        1.0 - pain_fraction
    }

    /// Apply raw damage to one part. Updates `tissue_damage`, recomputes
    /// `condition`, and sets `permanent = true` iff the part first reaches
    /// `Destroyed` and its category is configured permanent.
    /// Returns the post-application `PartCondition` for the caller to thread
    /// into a `BodyPartInjury` message.
    pub fn apply_damage(
        &mut self,
        part: BodyPart,
        damage: f32,
        thresholds: &[f32; 4],
        permanent_at_destroyed: &[bool; CAT_BODY_PART_COUNT],
    ) -> PartCondition {
        let idx = part.index();
        let state = &mut self.parts[idx];
        state.tissue_damage = (state.tissue_damage + damage).clamp(0.0, 1.0);
        let new_condition = PartCondition::from_tissue_damage(state.tissue_damage, thresholds);
        if new_condition == PartCondition::Destroyed
            && state.condition != PartCondition::Destroyed
            && permanent_at_destroyed[idx]
        {
            state.permanent = true;
        }
        state.condition = new_condition;
        new_condition
    }

    /// Heal one tick on every non-permanent-Destroyed part. Caller supplies
    /// the per-tick decrement (already converted from
    /// `BodyZoneConstants::healing_*` durations × time_scale). Permanent parts
    /// at Destroyed stay locked.
    pub fn heal_tick(
        &mut self,
        per_part_decrement: &[f32; CAT_BODY_PART_COUNT],
        thresholds: &[f32; 4],
    ) {
        for (idx, state) in self.parts.iter_mut().enumerate() {
            if state.condition == PartCondition::Destroyed && state.permanent {
                continue;
            }
            if state.tissue_damage <= 0.0 {
                continue;
            }
            state.tissue_damage = (state.tissue_damage - per_part_decrement[idx]).max(0.0);
            state.condition = PartCondition::from_tissue_damage(state.tissue_damage, thresholds);
        }
    }

    /// True if any part is at Wounded or worse. Used by the `Injured` marker
    /// writer (Stage B; in Stage A the legacy `Health.injuries`-driven writer
    /// is canonical).
    pub fn any_wounded_or_worse(&self) -> bool {
        self.parts
            .iter()
            .any(|p| p.condition >= PartCondition::Wounded)
    }

    /// Iterate (BodyPart, &BodyPartState) pairs in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (BodyPart, &BodyPartState)> {
        BodyPart::ALL
            .iter()
            .copied()
            .zip(self.parts.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_THRESHOLDS: [f32; 4] = [0.01, 0.26, 0.61, 0.91];
    const TEST_PERMANENT: [bool; CAT_BODY_PART_COUNT] = [
        false, // whiskers — regrow
        true,  // ears — torn tips persist
        true,  // mouth/jaw
        false, false, // scruff, throat (fatal before Mangled)
        false, false, false, false, false, false, // flanks, belly, four paws
        true,  // haunches — permanent limp
        true,  // tail — permanent crook
    ];

    fn test_weights() -> [f32; CAT_BODY_PART_COUNT] {
        [0.5, 0.5, 1.5, 0.8, 3.0, 1.5, 0.8, 1.0, 1.0, 1.0, 1.0, 2.0, 0.5]
    }

    #[test]
    fn condition_from_tissue_damage_matches_spec() {
        let t = &TEST_THRESHOLDS;
        assert_eq!(PartCondition::from_tissue_damage(0.0, t), PartCondition::Healthy);
        assert_eq!(PartCondition::from_tissue_damage(0.1, t), PartCondition::Bruised);
        assert_eq!(PartCondition::from_tissue_damage(0.3, t), PartCondition::Wounded);
        assert_eq!(PartCondition::from_tissue_damage(0.7, t), PartCondition::Mangled);
        assert_eq!(PartCondition::from_tissue_damage(0.95, t), PartCondition::Destroyed);
    }

    #[test]
    fn default_model_has_zero_pain_and_full_health() {
        let m = CatBodyModel::default();
        let w = test_weights();
        let max = w.iter().sum::<f32>();
        assert_eq!(m.total_pain(&w), 0.0);
        assert_eq!(m.health_derived(&w, max), 1.0);
    }

    #[test]
    fn permanent_destroyed_persists_after_healing() {
        let mut m = CatBodyModel::default();
        m.apply_damage(BodyPart::Ears, 0.95, &TEST_THRESHOLDS, &TEST_PERMANENT);
        assert_eq!(m.part(BodyPart::Ears).condition, PartCondition::Destroyed);
        assert!(m.part(BodyPart::Ears).permanent);
        // Aggressively heal — permanent destroyed stays locked.
        let decrement = [1.0_f32; CAT_BODY_PART_COUNT];
        m.heal_tick(&decrement, &TEST_THRESHOLDS);
        assert_eq!(m.part(BodyPart::Ears).condition, PartCondition::Destroyed);
        assert!(m.part(BodyPart::Ears).permanent);
    }

    #[test]
    fn non_permanent_destroyed_heals_back_down() {
        let mut m = CatBodyModel::default();
        m.apply_damage(BodyPart::Scruff, 0.95, &TEST_THRESHOLDS, &TEST_PERMANENT);
        assert_eq!(m.part(BodyPart::Scruff).condition, PartCondition::Destroyed);
        assert!(!m.part(BodyPart::Scruff).permanent);
        let decrement = [1.0_f32; CAT_BODY_PART_COUNT];
        m.heal_tick(&decrement, &TEST_THRESHOLDS);
        assert!(m.part(BodyPart::Scruff).condition < PartCondition::Destroyed);
    }

    #[test]
    fn total_pain_weights_correctly() {
        let mut m = CatBodyModel::default();
        let w = test_weights();
        m.apply_damage(BodyPart::Throat, 0.5, &TEST_THRESHOLDS, &TEST_PERMANENT);
        // throat weight = 3.0 → expected pain = 0.5 * 3.0 = 1.5
        assert!((m.total_pain(&w) - 1.5).abs() < 1e-5);
    }

    #[test]
    fn any_wounded_or_worse_threshold() {
        let mut m = CatBodyModel::default();
        assert!(!m.any_wounded_or_worse());
        m.apply_damage(BodyPart::Tail, 0.1, &TEST_THRESHOLDS, &TEST_PERMANENT); // Bruised
        assert!(!m.any_wounded_or_worse());
        m.apply_damage(BodyPart::Tail, 0.3, &TEST_THRESHOLDS, &TEST_PERMANENT); // Wounded
        assert!(m.any_wounded_or_worse());
    }
}
