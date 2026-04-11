use std::collections::HashMap;

use bevy_ecs::prelude::*;

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
    ItemRetrieved,
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
    ScryCompleted,
    CleanseCompleted,
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
        Feature::ItemRetrieved,
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
        Feature::ScryCompleted,
        Feature::CleanseCompleted,
    ];
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

    /// Compute the activation score.
    ///
    /// - `breadth_bonus`: flat points per feature that fires at all.
    /// - `depth_bonus`: `depth_bonus * ln(1 + count)` per active feature.
    pub fn activation_score(&self, breadth_bonus: f64, depth_bonus: f64) -> f64 {
        self.counts
            .values()
            .filter(|&&count| count > 0)
            .map(|&count| breadth_bonus + depth_bonus * (1.0 + count as f64).ln())
            .sum()
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
    fn activation_score_empty() {
        let sa = SystemActivation::default();
        assert_eq!(sa.activation_score(20.0, 5.0), 0.0);
    }

    #[test]
    fn activation_score_one_feature() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let score = sa.activation_score(20.0, 5.0);
        let expected = 20.0 + 5.0 * 2.0_f64.ln();
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn activation_score_scales_with_breadth() {
        let mut sa = SystemActivation::default();
        for feature in Feature::ALL {
            sa.record(*feature);
        }
        let score = sa.activation_score(20.0, 0.0);
        let expected = 20.0 * Feature::ALL.len() as f64;
        assert!((score - expected).abs() < 1e-10);
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
