use std::collections::BTreeMap;

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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
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
    /// An adult completed a FeedKitten step, transferring food from
    /// inventory to a dependent kitten. Positive-feature signal for
    /// the Caretake system's activity (Phase 4c.3).
    KittenFed,
    /// A cat advanced a Garden's `CropState.growth` via a TendCrops
    /// step. Positive-feature signal for the Farming system's
    /// activity (Phase 4c.4 — previously absent, which kept the
    /// Activation canary from catching the silent-dead farming
    /// pipeline for months).
    CropTended,
    /// A cat harvested a Garden at full growth, spawning food (or
    /// Thornbriar) into Stores. Paired with `CropTended` — splits the
    /// two distinct "farming is alive" signals so a partial failure
    /// (tending fires but harvest never does, or vice versa) is
    /// visible in the activation footer.
    CropHarvested,
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

    // --- §Phase 5a: silent-advance audit (see StepOutcome<W>) ---
    // Each of these was a "dead subsystem" candidate — the step resolver
    // could Advance without doing its real-world work, and no Feature
    // fired, so the Activation canary went blind to silent failures.
    //
    /// A cat consumed a food item at a Stores building, restoring
    /// hunger. Gated on actual food consumption (store non-empty AND
    /// a food item was found), not just time-out at the Stores zone.
    FoodEaten,
    /// Two cats completed a socialize-target interaction (non-groom,
    /// non-mentor): relationships mutated, social need boosted. Gated
    /// on a real target partner — a socialize step with no target no
    /// longer fires this.
    Socialized,
    /// An adult groomed another cat: relationship + restoration
    /// effect applied. Distinct from `self_groom` (cleanliness need).
    GroomedOther,
    /// A mentor-apprentice interaction occurred: skill/knowledge
    /// transfer + relationship shift.
    MentoredCat,
    /// A cat engaged wildlife in combat-posture: safety-need swing +
    /// combat-skill growth. Paired with the existing
    /// `CombatResolved` (emitted from `src/systems/combat.rs` when a
    /// fight resolves) — ThreatEngaged is "the step ran and found a
    /// target", CombatResolved is "combat terminated with a winner".
    ThreatEngaged,
    /// A cat successfully delivered one unit of build material from
    /// inventory to a ConstructionSite (incrementing the site's
    /// delivered ledger by one). Ticket 038 promoted this from a
    /// dead-wired emission to a real per-unit witness: each haul of
    /// the founding wagon-dismantling pile produces one event.
    MaterialsDelivered,
    /// A cat picked up a build-material `Item` from the ground
    /// (location flipped from `OnGround` to `Carried(cat)`, slot
    /// added to inventory). Paired with `MaterialsDelivered` —
    /// every delivery is preceded by exactly one pickup. Ticket 038.
    MaterialPickedUp,
    /// A cat completed a building-repair pass (condition ≥ 1.0).
    /// Not a silent-advance fix per se (repair already returns Fail
    /// on missing target), but previously unsignalled — repairs were
    /// invisible to the Activation canary.
    BuildingRepaired,
    /// A mating attempt completed without producing a pregnancy
    /// (Tom×Tom, or Queen who is already pregnant, etc). The social
    /// / belonging-tier interaction still occurred. Paired with
    /// `MatingOccurred` which fires only when a `Pregnancy` was
    /// inserted.
    CourtshipInteraction,
    /// Phase 6a §7.2 — drop-trigger gate fired for a cat's held
    /// `GoapPlan`: the `CommitmentStrategy` dispatch said to drop
    /// (achievement believed, planner hard-fail under SingleMinded,
    /// or satiation under OpenMinded) and the plan was removed.
    /// Distinct from `AnxietyInterrupt` (which is the Maslow
    /// event-driven preemption that bypasses §7.2 entirely).
    /// Neutral — the gate is a reconsideration signal, not a
    /// healthy-colony win or adverse event by itself.
    ///
    /// Aggregate counter — retained for back-compat with any
    /// dashboard reading it. Branch-specific counters below replace
    /// it for canary purposes.
    CommitmentDropTriggered,
    /// §7.2 gate dropped a `Blind` plan (Resting, Guarding). The
    /// Blind strategy drops only on `achievement_believed`, so this
    /// fires when a rest cycle or patrol completes. Expected to fire
    /// at least once per 15-min soak when Resting is reached.
    CommitmentDropBlind,
    /// §7.2 gate dropped a `SingleMinded` plan on `achievement_believed`
    /// (Hunt/Build/Forage/etc. goal met). Distinct from `…ReplanCap`
    /// below, which covers the `achievable_believed == false` hard-
    /// fail branch of the same strategy.
    CommitmentDropSingleMinded,
    /// §7.2 gate dropped an `OpenMinded` plan on `still_goal == false`
    /// (satiation / desire drift). Fires for Socializing satiation
    /// and future Exploring curiosity-drift.
    CommitmentDropOpenMinded,
    /// §7.2 `achievable_believed == false` hard-fail channel: the
    /// planner exhausted `max_replans` retries on a goal-shaped plan
    /// and the gate let it go. Fires alongside the executor's own
    /// "abandoned" narrative event at `goap.rs:~3144`; tracking it
    /// as a distinct Feature lets canaries catch planner collapse
    /// separately from legitimate completion.
    CommitmentDropReplanCap,
    /// Ticket 027b §7.M — `crate::ai::pairing::author_pairing_intentions`
    /// inserted a [`crate::components::PairingActivity`] Intention on
    /// a cat with a Friends-or-better orientation-compatible peer in
    /// range. Positive — every PairingIntentionEmitted is a step
    /// toward closing the structural Friends → Partners gap that
    /// stalled mating cadence in the seed-42 baseline. Stays
    /// `expected_to_fire_per_soak() => false` in Commit A because no
    /// reader yet uses the Intention; Commit B promotes when the bias
    /// readers ship.
    PairingIntentionEmitted,
    /// Ticket 027b §7.M — the L2 drop gate fired on a held Intention
    /// (partner died/banished/incapacitated, bond lost, life-stage
    /// transitioned, season cycled out, or both relationship axes
    /// collapsed below their floors). Neutral — drops are normal
    /// state transitions, not an adverse signal. Stays
    /// `expected_to_fire_per_soak() => false` because drops are
    /// bursty (a healthy 15-min soak may have zero drops).
    PairingDropped,
    /// Ticket 027b §7.M — a target-picker resolver picked the L2
    /// Intention partner *and* would not have picked them without the
    /// Intention's hard-1.0 pin (pre-pin bond_score < 1.0). Isolates
    /// "L2 actually changed target selection" from "Pairing was held
    /// but the cat would've picked them anyway". Wired in Commit B;
    /// activation stays unfired (and `expected_to_fire_per_soak() =>
    /// false`) until then.
    PairingBiasApplied,
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
        Feature::KittenFed,
        Feature::CropTended,
        Feature::CropHarvested,
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
        // §Phase 5a silent-advance audit
        Feature::FoodEaten,
        Feature::Socialized,
        Feature::GroomedOther,
        Feature::MentoredCat,
        Feature::ThreatEngaged,
        Feature::MaterialsDelivered,
        Feature::MaterialPickedUp,
        Feature::BuildingRepaired,
        Feature::CourtshipInteraction,
        // §Phase 6a §7.2 drop-trigger gate
        Feature::CommitmentDropTriggered,
        Feature::CommitmentDropBlind,
        Feature::CommitmentDropSingleMinded,
        Feature::CommitmentDropOpenMinded,
        Feature::CommitmentDropReplanCap,
        // §7.M L2 PairingActivity (ticket 027b)
        Feature::PairingIntentionEmitted,
        Feature::PairingDropped,
        Feature::PairingBiasApplied,
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
            Feature::KittenFed => Positive,
            Feature::CropTended => Positive,
            Feature::CropHarvested => Positive,
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
            // §Phase 5a silent-advance audit — healthy-subsystem activity
            Feature::FoodEaten => Positive,
            Feature::Socialized => Positive,
            Feature::GroomedOther => Positive,
            Feature::MentoredCat => Positive,
            Feature::ThreatEngaged => Positive,
            Feature::MaterialsDelivered => Positive,
            Feature::MaterialPickedUp => Positive,
            Feature::BuildingRepaired => Positive,
            Feature::CourtshipInteraction => Positive,
            // §7.M L2 PairingActivity (ticket 027b)
            Feature::PairingIntentionEmitted => Positive,
            Feature::PairingBiasApplied => Positive,

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
            Feature::CommitmentDropTriggered => Neutral,
            Feature::CommitmentDropBlind => Neutral,
            Feature::CommitmentDropSingleMinded => Neutral,
            Feature::CommitmentDropOpenMinded => Neutral,
            Feature::CommitmentDropReplanCap => Neutral,
            // §7.M L2 PairingActivity drop is a state transition,
            // not an adverse event.
            Feature::PairingDropped => Neutral,
        }
    }

    /// Whether a healthy canonical soak (seed 42, 900s release) is
    /// *expected* to fire this feature at least once.
    ///
    /// Used by the "never-fired-but-expected" canary introduced in
    /// §Phase 5a to catch silently-dead subsystems: a Positive
    /// feature returning `true` here must appear in the footer with
    /// `count >= 1`, otherwise the canary fails.
    ///
    /// Features marked `false` are legitimately rare — colony-
    /// defining events (a banishment, a shadow-fox ward save, a
    /// fated awakening) that may not occur in every soak. They're
    /// excluded from the canary rather than treated as dead.
    pub const fn expected_to_fire_per_soak(self) -> bool {
        match self {
            // --- Rare legend / colony-defining events — exempt ---
            Feature::ShadowFoxBanished => false,
            Feature::FateAwakened => false,
            Feature::SpiritCommunion => false,
            Feature::ShadowFoxAvoidedWard => false,
            Feature::ShadowFoxSpawn => false,
            Feature::WardSiegeStarted => false,
            Feature::DeathOldAge => false,
            Feature::AspirationCompleted => false,
            Feature::AspirationAbandoned => false,
            // Fox-ecology ambient signals that depend on world state
            // (a fox may or may not spawn / be in range in 15 min).
            Feature::FoxStoreRaided => false,
            Feature::FoxStandoffEscalated => false,
            Feature::FoxRetreated => false,
            Feature::FoxDenEstablished => false,
            Feature::FoxBred => false,
            Feature::FoxCubMatured => false,
            Feature::FoxDied => false,
            Feature::FoxAvoidedWard => false,
            Feature::FoxAvoidedPresence => false,
            Feature::FoxDenDefense => false,
            // Corruption-specific: may not fire in a clean run.
            Feature::CarcassSpawned => false,
            Feature::CarcassCleansed => false,
            Feature::CarcassHarvested => false,
            Feature::CorruptionPushback => false,
            Feature::HerbSuppressed => false,
            Feature::CorruptionHealthDrain => false,
            Feature::PersonalCorruptionEffect => false,
            Feature::RemedyApplied => false,
            Feature::InjuryHealed => false,
            // Misc rare events.
            Feature::PosseCandidateExcludedStarving => false,
            Feature::DepositFailedNoStore => false,
            Feature::GateProcessed => false,
            // Magic that requires specific actor priors.
            Feature::ScryCompleted => false,
            Feature::CleanseCompleted => false,
            Feature::WardPlaced => false,
            Feature::WardDespawned => false,
            // Building/craft that depends on plan cadence.
            Feature::BuildingRepaired => false,
            Feature::BuildingTidied => false,
            Feature::StorageUpgraded => false,
            // Ticket 027 Bug 1 wired this to `social::check_bonds`'s
            // courtship-drift gate, so it now fires whenever any
            // compatible adult pair drifts. Promoted out of the
            // rare-legend list to gate the never-fired canary.
            // §Phase 5a empirical calibration (seed-42 soak): these
            // are exempt because they depend on conditions the sim
            // can't guarantee in 15 min of wall-clock:
            //
            // - PreyDenFounded: multi-day event, rare in 15 min.
            // - KittenMatured: maturation takes sim-days; depends on
            //   kitten survival + enough sim-time to cross the threshold.
            //   Also a cascade dependency on `KittenBorn` firing first.
            // - ThreatEngaged: requires wildlife in range + surviving
            //   to fight_duration without morale break.
            // - BuildingRepaired: routing gap — the GOAP path
            //   internalises repair without emitting the Feature, so
            //   the legacy disposition-chain path is the only emitter.
            //   Tracked separately (no current ticket).
            //
            // - MaterialsDelivered / MaterialPickedUp: ticket 038 wired
            //   the full Pickup → Carry → Deliver pipeline (planner +
            //   step resolvers + executor dispatch) but parked the
            //   founding wagon-dismantling spawn behind the
            //   CLOWDER_FOUNDING_HAUL env var while balance work
            //   resolves an early-game starvation regression. With the
            //   spawn parked, no cat encounters a build-material pile,
            //   so neither Feature fires. When the spawn is activated
            //   (post-tuning), promote both back to `true`.
            //
            // The four "trunk" Features `FoodCooked`, `MatingOccurred`,
            // `GroomedOther`, `MentoredCat` deliberately stay in the
            // expected set even though they fire at zero in current
            // soaks — the canary flagging them RED is accurate and
            // tracks load-bearing tickets:
            // - FoodCooked   → ticket 036 (no kitchen built)
            // - GroomedOther → ticket 037 (silent-advance via GroomingFired)
            // - MentoredCat  → known mastery-decay dynamic
            // - MatingOccurred → ticket 027 (mating cadence cascade)
            //
            // Cascade-exempt: each entry below is silent strictly
            // because its trunk Feature is silent. Listing them as
            // `expected_to_fire_per_soak()` would multiply a single
            // root-cause failure into N canary entries; demoting them
            // to `false` keeps the canary signal one-per-trunk. When
            // the trunk's ticket lands, these will start firing and
            // can be promoted back to `true` if you want to track
            // them as independent canaries.
            // - GestationAdvanced / KittenBorn / KittenFed: cascade
            //   from MatingOccurred (ticket 027).
            // - ItemRetrieved: cascade from FoodCooked (ticket 036) —
            //   nothing in stores worth retrieving until cooking
            //   produces output.
            Feature::PreyDenFounded => false,
            Feature::KittenMatured => false,
            Feature::ThreatEngaged => false,
            // Ticket 038 — parked behind CLOWDER_FOUNDING_HAUL. See
            // block comment above.
            Feature::MaterialsDelivered => false,
            Feature::MaterialPickedUp => false,
            // Cascade-exempt (see block comment above).
            Feature::GestationAdvanced => false,
            Feature::KittenBorn => false,
            Feature::KittenFed => false,
            Feature::ItemRetrieved => false,
            // Ticket 027b §7.M L2 PairingActivity — **activation
            // deferred**. The author system at
            // `crate::ai::pairing::author_pairing_intentions` is built,
            // tested, and ready, but its schedule edge in
            // `plugins/simulation.rs` is commented out because
            // registering it in chain 2a perturbs Bevy 0.18's
            // topological sort enough to drop seed-42 from
            // Starvation=0 to Starvation=3 (scheduler-shift hazard
            // also documented on ticket 061). Both Pairing Positive
            // features therefore cannot fire and must be exempt
            // from the never-fired canary until activation lands.
            // When the schedule edge is uncommented, remove these
            // two false-arms so the canary can validate them.
            Feature::PairingIntentionEmitted => false,
            Feature::PairingBiasApplied => false,
            // Every other feature is expected to fire per soak.
            _ => true,
        }
    }
}

/// Stable lower-snake-ish names for `Feature::*` variants used in
/// diagnostic output (JSON-friendly, match the `serde` default of
/// variant-name-as-string). Used by the never-fired canary so the
/// offender list in the footer is human-readable rather than a
/// Debug dump.
pub fn feature_name(f: Feature) -> &'static str {
    // Mirror serde's default (PascalCase variant name). Exhaustive
    // match, so a new variant added without updating this table is a
    // compile error.
    match f {
        Feature::CorruptionSpread => "CorruptionSpread",
        Feature::CorruptionTileEffect => "CorruptionTileEffect",
        Feature::ShadowFoxSpawn => "ShadowFoxSpawn",
        Feature::WardDecay => "WardDecay",
        Feature::HerbSeasonalCheck => "HerbSeasonalCheck",
        Feature::RemedyApplied => "RemedyApplied",
        Feature::PersonalCorruptionEffect => "PersonalCorruptionEffect",
        Feature::CombatResolved => "CombatResolved",
        Feature::InjuryHealed => "InjuryHealed",
        Feature::FateAssigned => "FateAssigned",
        Feature::FateAwakened => "FateAwakened",
        Feature::AspirationSelected => "AspirationSelected",
        Feature::AspirationCompleted => "AspirationCompleted",
        Feature::AspirationAbandoned => "AspirationAbandoned",
        Feature::BondFormed => "BondFormed",
        Feature::CoordinatorElected => "CoordinatorElected",
        Feature::DirectiveIssued => "DirectiveIssued",
        Feature::BuildingConstructed => "BuildingConstructed",
        Feature::BuildingTidied => "BuildingTidied",
        Feature::GateProcessed => "GateProcessed",
        Feature::MoodContagion => "MoodContagion",
        Feature::PersonalityFriction => "PersonalityFriction",
        Feature::AnxietyInterrupt => "AnxietyInterrupt",
        Feature::PreyBred => "PreyBred",
        Feature::PreyDenAbandoned => "PreyDenAbandoned",
        Feature::PreyDenFounded => "PreyDenFounded",
        Feature::DenRaided => "DenRaided",
        Feature::WildlifeSpawned => "WildlifeSpawned",
        Feature::DeathStarvation => "DeathStarvation",
        Feature::DeathOldAge => "DeathOldAge",
        Feature::DeathInjury => "DeathInjury",
        Feature::KnowledgePromoted => "KnowledgePromoted",
        Feature::KnowledgeForgotten => "KnowledgeForgotten",
        Feature::SpiritCommunion => "SpiritCommunion",
        Feature::StorageUpgraded => "StorageUpgraded",
        Feature::DepositRejected => "DepositRejected",
        Feature::DepositFailedNoStore => "DepositFailedNoStore",
        Feature::ItemRetrieved => "ItemRetrieved",
        Feature::FoodCooked => "FoodCooked",
        Feature::KittenBorn => "KittenBorn",
        Feature::GestationAdvanced => "GestationAdvanced",
        Feature::KittenMatured => "KittenMatured",
        Feature::MatingOccurred => "MatingOccurred",
        Feature::KittenFed => "KittenFed",
        Feature::CropTended => "CropTended",
        Feature::CropHarvested => "CropHarvested",
        Feature::FoxHuntedPrey => "FoxHuntedPrey",
        Feature::FoxStoreRaided => "FoxStoreRaided",
        Feature::FoxStandoff => "FoxStandoff",
        Feature::FoxStandoffEscalated => "FoxStandoffEscalated",
        Feature::FoxRetreated => "FoxRetreated",
        Feature::FoxDenEstablished => "FoxDenEstablished",
        Feature::FoxBred => "FoxBred",
        Feature::FoxCubMatured => "FoxCubMatured",
        Feature::FoxDied => "FoxDied",
        Feature::FoxScentMarked => "FoxScentMarked",
        Feature::FoxAvoidedCat => "FoxAvoidedCat",
        Feature::FoxDenDefense => "FoxDenDefense",
        Feature::FoxAvoidedWard => "FoxAvoidedWard",
        Feature::FoxAvoidedPresence => "FoxAvoidedPresence",
        Feature::ShadowFoxAvoidedWard => "ShadowFoxAvoidedWard",
        Feature::DirectiveDelivered => "DirectiveDelivered",
        Feature::CarcassSpawned => "CarcassSpawned",
        Feature::WardSiegeStarted => "WardSiegeStarted",
        Feature::CarcassCleansed => "CarcassCleansed",
        Feature::CarcassHarvested => "CarcassHarvested",
        Feature::CorruptionPushback => "CorruptionPushback",
        Feature::HerbSuppressed => "HerbSuppressed",
        Feature::CorruptionHealthDrain => "CorruptionHealthDrain",
        Feature::GatherHerbCompleted => "GatherHerbCompleted",
        Feature::WardPlaced => "WardPlaced",
        Feature::WardDespawned => "WardDespawned",
        Feature::ScryCompleted => "ScryCompleted",
        Feature::CleanseCompleted => "CleanseCompleted",
        Feature::ShadowFoxBanished => "ShadowFoxBanished",
        Feature::PosseCandidateExcludedStarving => "PosseCandidateExcludedStarving",
        Feature::FoodEaten => "FoodEaten",
        Feature::Socialized => "Socialized",
        Feature::GroomedOther => "GroomedOther",
        Feature::MentoredCat => "MentoredCat",
        Feature::ThreatEngaged => "ThreatEngaged",
        Feature::MaterialsDelivered => "MaterialsDelivered",
        Feature::MaterialPickedUp => "MaterialPickedUp",
        Feature::BuildingRepaired => "BuildingRepaired",
        Feature::CourtshipInteraction => "CourtshipInteraction",
        Feature::CommitmentDropTriggered => "CommitmentDropTriggered",
        Feature::CommitmentDropBlind => "CommitmentDropBlind",
        Feature::CommitmentDropSingleMinded => "CommitmentDropSingleMinded",
        Feature::CommitmentDropOpenMinded => "CommitmentDropOpenMinded",
        Feature::CommitmentDropReplanCap => "CommitmentDropReplanCap",
        Feature::PairingIntentionEmitted => "PairingIntentionEmitted",
        Feature::PairingDropped => "PairingDropped",
        Feature::PairingBiasApplied => "PairingBiasApplied",
    }
}

// ---------------------------------------------------------------------------
// SystemActivation
// ---------------------------------------------------------------------------

/// Tracks how many times each simulation feature meaningfully fires.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SystemActivation {
    /// Per-feature firing counts. Stored as a `BTreeMap` (not `HashMap`) so
    /// (1) `activation_score_in`'s f64 sum is associative across processes —
    /// `HashMap` iteration order varies with `RandomState`'s per-process seed,
    /// and float addition is non-associative, so a `HashMap` here produced
    /// 1-ULP drift in `positive_activation_score` on otherwise-identical seed-42
    /// runs; (2) the JSON serialization order in events.jsonl is stable, which
    /// `just verdict` and the determinism test assume.
    pub counts: BTreeMap<Feature, u64>,
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

    /// §Phase 5a never-fired canary: Positive features that a
    /// canonical soak is expected to fire at least once, but
    /// didn't. Returns the names as strings for direct JSON-
    /// friendly emission in the footer.
    ///
    /// A non-empty list means a silently-dead subsystem — the
    /// exact failure mode that kept the farming bug (CropTended
    /// never firing) invisible for months before §Phase 4c.4.
    pub fn never_fired_expected_positives(&self) -> Vec<&'static str> {
        Feature::ALL
            .iter()
            .filter(|f| {
                f.category() == FeatureCategory::Positive
                    && f.expected_to_fire_per_soak()
                    && self.counts.get(f).copied().unwrap_or(0) == 0
            })
            .map(|f| feature_name(*f))
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
        // 36 pre-existing Positive + 8 added in §Phase 5a (FoodEaten,
        // Socialized, GroomedOther, MentoredCat, ThreatEngaged,
        // MaterialsDelivered, BuildingRepaired, CourtshipInteraction).
        // Phase 6a added 1 Neutral (CommitmentDropTriggered) +
        // 4 branch-specific Neutrals (Blind / SingleMinded /
        // OpenMinded / ReplanCap) for the §7.2 commitment-gate
        // tracing split. Ticket 038 added 1 Positive
        // (MaterialPickedUp, paired with the resurrected
        // MaterialsDelivered). Ticket 027b added 2 Positive
        // (PairingIntentionEmitted, PairingBiasApplied) + 1 Neutral
        // (PairingDropped) for the §7.M L2 PairingActivity layer.
        assert_eq!(positive, 47);
        assert_eq!(negative, 20);
        assert_eq!(neutral, 26);
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
            47
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Negative),
            20
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Neutral),
            26
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

    #[test]
    fn never_fired_expected_positives_reports_silently_dead() {
        // Empty SA: every expected-Positive is missing from counts,
        // so the full expected set is reported.
        let sa = SystemActivation::default();
        let missing = sa.never_fired_expected_positives();
        // Representative check: trunk Features (each one independent)
        // show up when the tracker is empty.
        assert!(missing.contains(&"MatingOccurred"));
        assert!(missing.contains(&"CropTended"));
        assert!(missing.contains(&"FoodEaten"));
        assert!(missing.contains(&"Socialized"));
        assert!(missing.contains(&"MentoredCat"));
        // Rare-legend features are excluded.
        assert!(!missing.contains(&"ShadowFoxBanished"));
        assert!(!missing.contains(&"FateAwakened"));
        // Cascade-exempt features are excluded — they cascade from
        // their trunk and don't add independent canary signal.
        assert!(!missing.contains(&"KittenFed"));
        assert!(!missing.contains(&"GestationAdvanced"));
        assert!(!missing.contains(&"KittenBorn"));
        assert!(!missing.contains(&"ItemRetrieved"));
    }

    #[test]
    fn never_fired_expected_positives_shrinks_as_features_fire() {
        let mut sa = SystemActivation::default();
        let before = sa.never_fired_expected_positives();
        sa.record(Feature::FoodEaten);
        sa.record(Feature::CropTended);
        let after = sa.never_fired_expected_positives();
        assert_eq!(after.len(), before.len() - 2);
        assert!(!after.contains(&"FoodEaten"));
        assert!(!after.contains(&"CropTended"));
    }

    #[test]
    fn expected_to_fire_per_soak_classification() {
        // Core-subsystem trunks must be expected.
        assert!(Feature::CropTended.expected_to_fire_per_soak());
        assert!(Feature::FoodEaten.expected_to_fire_per_soak());
        assert!(Feature::Socialized.expected_to_fire_per_soak());
        assert!(Feature::MentoredCat.expected_to_fire_per_soak());
        // Trunks of chains-with-open-tickets stay expected so the
        // canary keeps flagging them RED until their tickets land.
        assert!(Feature::MatingOccurred.expected_to_fire_per_soak());
        assert!(Feature::FoodCooked.expected_to_fire_per_soak());
        assert!(Feature::GroomedOther.expected_to_fire_per_soak());
        // Promoted by ticket 027 Bug 1 (courtship-drift emits per-tick).
        assert!(Feature::CourtshipInteraction.expected_to_fire_per_soak());
        // Rare-legend events must be exempted.
        assert!(!Feature::ShadowFoxBanished.expected_to_fire_per_soak());
        assert!(!Feature::FateAwakened.expected_to_fire_per_soak());
        assert!(!Feature::ScryCompleted.expected_to_fire_per_soak());
        // Cascade-exempt: silent strictly because their trunk is
        // silent. Promoting them to expected would multiply one
        // root-cause failure into N canary entries.
        assert!(!Feature::GestationAdvanced.expected_to_fire_per_soak());
        assert!(!Feature::KittenBorn.expected_to_fire_per_soak());
        assert!(!Feature::KittenFed.expected_to_fire_per_soak());
        assert!(!Feature::ItemRetrieved.expected_to_fire_per_soak());
    }
}
