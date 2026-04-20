use std::collections::HashMap;

use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// FeatureCategory — valence of a tracked feature
// ---------------------------------------------------------------------------

/// Whether a feature firing represents a good, bad, or neutral event.
///
/// `Positive` features contribute to the colony's activation score and the
/// "is the colony thriving?" diagnostic. `Negative` features are tallied as a
/// raw event count — how many bad things happened — and do *not* inflate the
/// activation score. `Neutral` features are system-churn signals used for
/// per-feature breakdowns but not rolled up into any score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FeatureCategory {
    Positive,
    Negative,
    Neutral,
}

// ---------------------------------------------------------------------------
// Feature — trackable simulation features
// ---------------------------------------------------------------------------

/// Enumeration of simulation features whose activation we track.
///
/// Each variant represents a meaningful event in the simulation — not a Bevy
/// system running, but actual *work* being done (corruption spreading to a
/// new tile, a bond forming, a ShadowFox spawning, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Feature {
    CorruptionSpread,
    CorruptionTileEffect,
    ShadowFoxSpawn,
    WardDecay,
    HerbSeasonalCheck,
    RemedyApplied,
    PersonalCorruptionEffect,
    CombatResolved,
    InjuryHealed,
    FateAssigned,
    FateAwakened,
    AspirationSelected,
    AspirationCompleted,
    AspirationAbandoned,
    BondFormed,
    CoordinatorElected,
    DirectiveIssued,
    BuildingConstructed,
    BuildingTidied,
    GateProcessed,
    MoodContagion,
    PersonalityFriction,
    AnxietyInterrupt,
    PreyBred,
    PreyDenAbandoned,
    PreyDenFounded,
    DenRaided,
    WildlifeSpawned,
    DeathStarvation,
    DeathOldAge,
    DeathInjury,
    KnowledgePromoted,
    KnowledgeForgotten,
    SpiritCommunion,
    StorageUpgraded,
    DepositRejected,
    DepositFailedNoStore,
    ItemRetrieved,
    /// A cat finished cooking a raw food item at a Kitchen, flipping its
    /// `cooked` flag. Eating the item later grants a hunger multiplier.
    FoodCooked,
    KittenBorn,
    GestationAdvanced,
    KittenMatured,
    MatingOccurred,
    // --- Fox ecology ---
    FoxHuntedPrey,
    FoxStoreRaided,
    FoxStandoff,
    FoxStandoffEscalated,
    FoxRetreated,
    FoxDenEstablished,
    FoxBred,
    FoxCubMatured,
    FoxDied,
    FoxScentMarked,
    FoxAvoidedCat,
    FoxDenDefense,
    FoxAvoidedWard,
    FoxAvoidedPresence,
    ShadowFoxAvoidedWard,
    DirectiveDelivered,
    // --- Corruption & carcass systems ---
    CarcassSpawned,
    WardSiegeStarted,
    CarcassCleansed,
    CarcassHarvested,
    CorruptionPushback,
    HerbSuppressed,
    CorruptionHealthDrain,
    GatherHerbCompleted,
    WardPlaced,
    WardDespawned,
    ScryCompleted,
    CleanseCompleted,
    /// A shadow-fox was banished by a posse of cats — colony-defining Legend event.
    ShadowFoxBanished,
    /// A candidate cat was skipped over for a Fight directive because its
    /// hunger fell below the critical-interrupt floor. Emitted from
    /// `dispatch_urgent_directives` so the starvation-respects-posse guard
    /// is observable in `events.jsonl`.
    PosseCandidateExcludedStarving,
}

impl Feature {
    pub const ALL: &[Feature] = &[
        Feature::CorruptionSpread,
        Feature::CorruptionTileEffect,
        Feature::ShadowFoxSpawn,
        Feature::WardDecay,
        Feature::HerbSeasonalCheck,
        Feature::RemedyApplied,
        Feature::PersonalCorruptionEffect,
        Feature::CombatResolved,
        Feature::InjuryHealed,
        Feature::FateAssigned,
        Feature::FateAwakened,
        Feature::AspirationSelected,
        Feature::AspirationCompleted,
        Feature::AspirationAbandoned,
        Feature::BondFormed,
        Feature::CoordinatorElected,
        Feature::DirectiveIssued,
        Feature::BuildingConstructed,
        Feature::BuildingTidied,
        Feature::GateProcessed,
        Feature::MoodContagion,
        Feature::PersonalityFriction,
        Feature::AnxietyInterrupt,
        Feature::PreyBred,
        Feature::PreyDenAbandoned,
        Feature::PreyDenFounded,
        Feature::DenRaided,
        Feature::WildlifeSpawned,
        Feature::DeathStarvation,
        Feature::DeathOldAge,
        Feature::DeathInjury,
        Feature::KnowledgePromoted,
        Feature::KnowledgeForgotten,
        Feature::SpiritCommunion,
        Feature::StorageUpgraded,
        Feature::DepositRejected,
        Feature::DepositFailedNoStore,
        Feature::ItemRetrieved,
        Feature::FoodCooked,
        Feature::KittenBorn,
        Feature::GestationAdvanced,
        Feature::KittenMatured,
        Feature::MatingOccurred,
        // Fox ecology
        Feature::FoxHuntedPrey,
        Feature::FoxStoreRaided,
        Feature::FoxStandoff,
        Feature::FoxStandoffEscalated,
        Feature::FoxRetreated,
        Feature::FoxDenEstablished,
        Feature::FoxBred,
        Feature::FoxCubMatured,
        Feature::FoxDied,
        Feature::FoxScentMarked,
        Feature::FoxAvoidedCat,
        Feature::FoxDenDefense,
        Feature::FoxAvoidedWard,
        Feature::FoxAvoidedPresence,
        Feature::ShadowFoxAvoidedWard,
        Feature::DirectiveDelivered,
        // Corruption & carcass systems
        Feature::CarcassSpawned,
        Feature::WardSiegeStarted,
        Feature::CarcassCleansed,
        Feature::CarcassHarvested,
        Feature::CorruptionPushback,
        Feature::HerbSuppressed,
        Feature::CorruptionHealthDrain,
        Feature::GatherHerbCompleted,
        Feature::WardPlaced,
        Feature::WardDespawned,
        Feature::ScryCompleted,
        Feature::CleanseCompleted,
        Feature::ShadowFoxBanished,
        Feature::PosseCandidateExcludedStarving,
    ];

    /// The valence of this feature.
    ///
    /// Exhaustive match — adding a new `Feature` variant without classifying
    /// it here is a compile error, which is intentional: the activation
    /// diagnostics depend on every feature having a known valence.
    pub const fn category(self) -> FeatureCategory {
        use FeatureCategory::*;
        match self {
            // --- Positive: healthy-colony wins ---
            Feature::RemedyApplied => Positive,
            Feature::InjuryHealed => Positive,
            Feature::FateAssigned => Positive,
            Feature::FateAwakened => Positive,
            Feature::AspirationSelected => Positive,
            Feature::AspirationCompleted => Positive,
            Feature::BondFormed => Positive,
            Feature::CoordinatorElected => Positive,
            Feature::DirectiveIssued => Positive,
            Feature::DirectiveDelivered => Positive,
            Feature::BuildingConstructed => Positive,
            Feature::BuildingTidied => Positive,
            Feature::GateProcessed => Positive,
            Feature::PreyDenFounded => Positive,
            Feature::KnowledgePromoted => Positive,
            Feature::SpiritCommunion => Positive,
            Feature::StorageUpgraded => Positive,
            Feature::ItemRetrieved => Positive,
            Feature::FoodCooked => Positive,
            Feature::KittenBorn => Positive,
            Feature::GestationAdvanced => Positive,
            Feature::KittenMatured => Positive,
            Feature::MatingOccurred => Positive,
            Feature::CarcassCleansed => Positive,
            Feature::CarcassHarvested => Positive,
            Feature::GatherHerbCompleted => Positive,
            Feature::WardPlaced => Positive,
            Feature::ScryCompleted => Positive,
            Feature::CleanseCompleted => Positive,
            Feature::ShadowFoxBanished => Positive,
            // Old-age death matches the existing `deaths_old_age_bonus`
            // convention in `achievement_points` — a life well lived.
            Feature::DeathOldAge => Positive,
            // Defensive wins against corruption / shadowfoxes.
            Feature::CorruptionPushback => Positive,
            Feature::ShadowFoxAvoidedWard => Positive,

            // --- Negative: adverse events, colony loss signals ---
            Feature::DeathStarvation => Negative,
            Feature::DeathInjury => Negative,
            Feature::CorruptionSpread => Negative,
            Feature::CorruptionTileEffect => Negative,
            Feature::CorruptionHealthDrain => Negative,
            Feature::PersonalCorruptionEffect => Negative,
            Feature::ShadowFoxSpawn => Negative,
            Feature::WardDecay => Negative,
            Feature::WardDespawned => Negative,
            Feature::WardSiegeStarted => Negative,
            Feature::HerbSuppressed => Negative,
            Feature::AnxietyInterrupt => Negative,
            Feature::AspirationAbandoned => Negative,
            Feature::DenRaided => Negative,
            Feature::PreyDenAbandoned => Negative,
            Feature::DepositRejected => Negative,
            Feature::DepositFailedNoStore => Negative,
            Feature::PosseCandidateExcludedStarving => Negative,
            Feature::KnowledgeForgotten => Negative,
            Feature::FoxStoreRaided => Negative,

            // --- Neutral: system activity, no inherent valence ---
            Feature::HerbSeasonalCheck => Neutral,
            Feature::CombatResolved => Neutral,
            Feature::MoodContagion => Neutral,
            Feature::PersonalityFriction => Neutral,
            Feature::PreyBred => Neutral,
            Feature::WildlifeSpawned => Neutral,
            Feature::CarcassSpawned => Neutral,
            Feature::FoxHuntedPrey => Neutral,
            Feature::FoxStandoff => Neutral,
            Feature::FoxStandoffEscalated => Neutral,
            Feature::FoxRetreated => Neutral,
            Feature::FoxDenEstablished => Neutral,
            Feature::FoxBred => Neutral,
            Feature::FoxCubMatured => Neutral,
            Feature::FoxDied => Neutral,
            Feature::FoxScentMarked => Neutral,
            Feature::FoxAvoidedCat => Neutral,
            Feature::FoxDenDefense => Neutral,
            Feature::FoxAvoidedWard => Neutral,
            Feature::FoxAvoidedPresence => Neutral,
        }
    }
}

// ---------------------------------------------------------------------------
// SystemActivation
// ---------------------------------------------------------------------------

/// Tracks how many times each simulation feature meaningfully fires.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SystemActivation {
    pub counts: HashMap<Feature, u64>,
}

impl SystemActivation {
    /// Record one firing of a feature.
    pub fn record(&mut self, feature: Feature) {
        *self.counts.entry(feature).or_insert(0) += 1;
    }

    /// Features that have never fired.
    pub fn dead_features(&self) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .filter(|f| self.counts.get(f).copied().unwrap_or(0) == 0)
            .copied()
            .collect()
    }

    /// Number of distinct features that have fired at least once.
    pub fn features_active(&self) -> u32 {
        self.counts.values().filter(|&&c| c > 0).count() as u32
    }

    /// Activation score restricted to a single feature category.
    ///
    /// For `Positive`, this is the main colony-thriving signal. For other
    /// categories the score is still computable but less meaningful — prefer
    /// `negative_event_count` for negative-valence features.
    pub fn activation_score_in(
        &self,
        category: FeatureCategory,
        breadth_bonus: f64,
        depth_bonus: f64,
    ) -> f64 {
        self.counts
            .iter()
            .filter(|(feature, &count)| count > 0 && feature.category() == category)
            .map(|(_, &count)| breadth_bonus + depth_bonus * (1.0 + count as f64).ln())
            .sum()
    }

    /// Positive-only activation score. This is the one that should feed
    /// `ColonyScore::aggregate`; mixing in negative/neutral features made the
    /// aggregate reward colony distress.
    pub fn positive_activation_score(&self, breadth_bonus: f64, depth_bonus: f64) -> f64 {
        self.activation_score_in(FeatureCategory::Positive, breadth_bonus, depth_bonus)
    }

    /// Raw count of all negative-valence feature firings.
    ///
    /// "How many bad things happened" is the right question for negative
    /// events — the log-scaled breadth+depth score is designed to reward
    /// diverse activity, which is the opposite of what we want for failures.
    pub fn negative_event_count(&self) -> u64 {
        self.counts
            .iter()
            .filter(|(feature, _)| feature.category() == FeatureCategory::Negative)
            .map(|(_, &count)| count)
            .sum()
    }

    /// Distinct features in a given category that have fired at least once.
    pub fn features_active_in(&self, category: FeatureCategory) -> u32 {
        self.counts
            .iter()
            .filter(|(feature, &count)| count > 0 && feature.category() == category)
            .count() as u32
    }

    /// Total number of features in a given category across `Feature::ALL`.
    pub fn features_total_in(category: FeatureCategory) -> u32 {
        Feature::ALL
            .iter()
            .filter(|f| f.category() == category)
            .count() as u32
    }

    /// Features in a given category that have never fired.
    pub fn dead_features_in(&self, category: FeatureCategory) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .filter(|f| f.category() == category && self.counts.get(f).copied().unwrap_or(0) == 0)
            .copied()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_increments() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::BondFormed);
        sa.record(Feature::CombatResolved);
        assert_eq!(sa.counts[&Feature::BondFormed], 2);
        assert_eq!(sa.counts[&Feature::CombatResolved], 1);
    }

    #[test]
    fn dead_features_returns_unfired() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let dead = sa.dead_features();
        assert!(dead.contains(&Feature::CorruptionSpread));
        assert!(!dead.contains(&Feature::BondFormed));
        assert_eq!(dead.len(), Feature::ALL.len() - 1);
    }

    #[test]
    fn features_active_count() {
        let mut sa = SystemActivation::default();
        assert_eq!(sa.features_active(), 0);
        sa.record(Feature::BondFormed);
        sa.record(Feature::CombatResolved);
        assert_eq!(sa.features_active(), 2);
    }

    #[test]
    fn positive_activation_score_empty() {
        let sa = SystemActivation::default();
        assert_eq!(sa.positive_activation_score(20.0, 5.0), 0.0);
    }

    #[test]
    fn positive_activation_score_one_feature() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let score = sa.positive_activation_score(20.0, 5.0);
        let expected = 20.0 + 5.0 * 2.0_f64.ln();
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn positive_activation_score_scales_with_breadth() {
        let mut sa = SystemActivation::default();
        for feature in Feature::ALL {
            sa.record(*feature);
        }
        let score = sa.positive_activation_score(20.0, 0.0);
        let expected = 20.0 * SystemActivation::features_total_in(FeatureCategory::Positive) as f64;
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn every_feature_has_a_category() {
        for feature in Feature::ALL {
            let _ = feature.category();
        }
    }

    #[test]
    fn category_counts_match_plan() {
        let mut positive = 0;
        let mut negative = 0;
        let mut neutral = 0;
        for feature in Feature::ALL {
            match feature.category() {
                FeatureCategory::Positive => positive += 1,
                FeatureCategory::Negative => negative += 1,
                FeatureCategory::Neutral => neutral += 1,
            }
        }
        assert_eq!(positive + negative + neutral, Feature::ALL.len());
        assert_eq!(positive, 33);
        assert_eq!(negative, 20);
        assert_eq!(neutral, 20);
    }

    #[test]
    fn representative_classifications() {
        assert_eq!(Feature::BondFormed.category(), FeatureCategory::Positive);
        assert_eq!(Feature::DeathOldAge.category(), FeatureCategory::Positive);
        assert_eq!(
            Feature::ShadowFoxBanished.category(),
            FeatureCategory::Positive
        );
        assert_eq!(
            Feature::DeathStarvation.category(),
            FeatureCategory::Negative
        );
        assert_eq!(
            Feature::CorruptionSpread.category(),
            FeatureCategory::Negative
        );
        assert_eq!(
            Feature::FoxStoreRaided.category(),
            FeatureCategory::Negative
        );
        assert_eq!(Feature::MoodContagion.category(), FeatureCategory::Neutral);
        assert_eq!(Feature::FoxHuntedPrey.category(), FeatureCategory::Neutral);
        assert_eq!(Feature::CombatResolved.category(), FeatureCategory::Neutral);
    }

    #[test]
    fn positive_activation_score_ignores_negatives() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed); // positive
        sa.record(Feature::DeathStarvation); // negative — should not contribute
        sa.record(Feature::MoodContagion); // neutral — should not contribute
        let score = sa.positive_activation_score(20.0, 5.0);
        let expected = 20.0 + 5.0 * 2.0_f64.ln(); // one positive feature firing once
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn negative_event_count_sums_only_negatives() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::CorruptionSpread);
        sa.record(Feature::BondFormed); // positive — should not contribute
        sa.record(Feature::MoodContagion); // neutral — should not contribute
        assert_eq!(sa.negative_event_count(), 3);
    }

    #[test]
    fn features_active_in_partitions_by_category() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::KittenBorn);
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::MoodContagion);
        assert_eq!(sa.features_active_in(FeatureCategory::Positive), 2);
        assert_eq!(sa.features_active_in(FeatureCategory::Negative), 1);
        assert_eq!(sa.features_active_in(FeatureCategory::Neutral), 1);
    }

    #[test]
    fn features_total_in_matches_category_counts() {
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Positive),
            33
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Negative),
            20
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Neutral),
            20
        );
    }

    #[test]
    fn dead_features_in_filters_by_category() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let dead_pos = sa.dead_features_in(FeatureCategory::Positive);
        assert!(!dead_pos.contains(&Feature::BondFormed));
        assert!(dead_pos.contains(&Feature::KittenBorn));
        // All negative features are dead (none fired).
        let dead_neg = sa.dead_features_in(FeatureCategory::Negative);
        assert_eq!(
            dead_neg.len(),
            SystemActivation::features_total_in(FeatureCategory::Negative) as usize
        );
    }

    #[test]
    fn serde_round_trip() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::CorruptionSpread);
        let json = serde_json::to_string(&sa).unwrap();
        let sa2: SystemActivation = serde_json::from_str(&json).unwrap();
        assert_eq!(sa.counts, sa2.counts);
    }
}
