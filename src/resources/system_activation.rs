use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::components::joint_intention::PracticeKind;

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
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
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
    /// Ticket 118 — substrate-driven plan preemption fired. The
    /// `check_modifier_preemption` system found at least one acute-class
    /// modifier whose `preempts_in_flight()` predicate returned true
    /// for an in-flight cat, dropped the cat's plan via
    /// `plan_substrate::try_preempt`, and the cat will re-elect on the
    /// next tick. Distinct from `AnxietyInterrupt` (the legacy Maslow
    /// override that 118+119 are retiring): this fires whenever the
    /// substrate is asking for behavioral expression, not only on the
    /// hardcoded `health < critical_threshold` predicate. Negative
    /// valence — interrupts are colony stress signals.
    ModifierPreemption,
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
    ShadowFoxAvoidedCatScent,
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
    /// 035: a cat completed a Bury chain — corpse received `Buried`
    /// marker, was despawned, and a `Grave` entity was spawned at
    /// the same tile. Tally: drives the `burial` continuity canary
    /// via the paired `EventKind::BurialFired` event.
    BurialPerformed,
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
    /// Ticket 288 — `morale_break` consumed by the GOAP step-fail
    /// dispatcher. The cat's EngageThreat step refused to continue
    /// (HP below `fight_bail_health_threshold`) and the dispatcher
    /// released the disposition commitment so L3 can re-elect on the
    /// next tick — rather than the legacy in-disposition replan path
    /// that left wounded Guarding cats marching toward ambushes.
    /// Neutral; rare-event class (depends on a wounded cat reaching
    /// EngageThreat) so `expected_to_fire_per_soak() => false` until
    /// post-fix baseline shows reliable firing.
    CommitmentDropMoraleBreak,
    // ---------- Ticket 127 — JointIntention (subsumes §7.M PairingActivity) ----------
    /// `crate::ai::joint_intention::author_joint_intentions` inserted a
    /// [`crate::components::JointIntention`] on an eligible cat for the
    /// named practice. For Courtship this is the 1:1 successor to
    /// `PairingIntentionEmitted` — same emission semantics, same canary
    /// classification. `expected_to_fire_per_soak() => true` for
    /// Courtship.
    JointIntentionEmitted { practice: PracticeKind },
    /// The author's drop gate fired on a held JointIntention. Neutral
    /// — drops are normal state transitions, not an adverse signal
    /// (mirrors `PairingDropped`). Stays
    /// `expected_to_fire_per_soak() => false` (bursty).
    JointIntentionDropped { practice: PracticeKind },
    /// A practice-biased resolver was invoked with
    /// `target == JointIntention.partner` AND
    /// `joint.practice == bias-reader's practice filter`, applying the
    /// practice's `bias_multiplier` to that tick's fondness +
    /// familiarity deltas. Per-amplification fire site (single-seed
    /// observable on any healthy chain). Successor to
    /// `PairingBiasApplied`. `expected_to_fire_per_soak() => true` for
    /// Courtship.
    JointBiasApplied { practice: PracticeKind },
    /// The author advanced the cat's `PracticeStage` to the next
    /// observable stage in the practice's transition table (Courtship:
    /// Approach → Courting → Mating → Bonded). Positive — stage
    /// progression is the structural signal that the practice is
    /// progressing rather than stalling. `expected_to_fire_per_soak()
    /// => true` for Courtship (any healthy soak crosses
    /// Approach → Courting in the seed-42 window).
    JointStageAdvanced { practice: PracticeKind },
    /// A paired cat's `PracticeStage` differed from its partner's at
    /// the lower-Entity-index side of the pair this tick — the
    /// substrate hook for "codified irony" (one cat believes they're
    /// courting while the other is just being friendly). Counted once
    /// per pair per tick. Neutral — mismatch windows are healthy
    /// narrative texture, not a regression signal.
    /// `expected_to_fire_per_soak() => false` — a perfectly-synced
    /// healthy colony can have zero mismatch ticks.
    JointStageMismatchTickAccrued { practice: PracticeKind },

    /// Ticket 080 — `evaluate_target_taking` gated a candidate to 0.0
    /// because its `Reserved.owner` named a cat other than the scoring
    /// cat. Neutral — observability of the resource-reservation
    /// substrate; firing means the gate is doing real work.
    /// `expected_to_fire_per_soak() => false` until the producer side
    /// (`record_target_picked` writes) ships in a follow-on; without
    /// producers, no `Reserved` is ever written, so the gate cannot
    /// activate.
    ReservationContended,
    /// Ticket 073 — the `target_recent_failure` Consideration scored
    /// below 1.0 for a candidate (i.e., the cooldown penalty was
    /// load-bearing for at least one (cat, action, target) tuple this
    /// tick). Neutral — recently-failed candidates being penalized is
    /// normal substrate behavior; the count is a soft soak-delta
    /// sentinel for "is the cooldown actually getting applied?".
    /// Stays `expected_to_fire_per_soak() => false` until soak data
    /// confirms a healthy seed-42 run sees ≥1 cooldown penalty fire.
    TargetCooldownApplied,
    /// Ticket 104 — `HideDse` selected and a freeze cycle concluded
    /// (resolved as `Advance` in `resolve_hide`). Positive — colony
    /// is exercising the third predator-avoidance valence ("remain
    /// still and hope") alongside Flee and Fight.
    /// `expected_to_fire_per_soak() => false` initially: Phase 1 ships
    /// dormant (the `HideEligible` marker is never authored), and
    /// even after activation the freeze valence is rare-event class
    /// — it requires the cornered-and-overmatched gate from modifier
    /// 105 to trip, which most healthy soaks won't see.
    HideFreezeFired,
    /// Ticket 149 — a discrete hunt attempt resolved (kill / lost /
    /// abandoned). Paired 1:1 with `EventKind::HuntAttempt` events;
    /// every successful kill, every approach loss, every abandon
    /// fires this. Lets the never-fired canary catch a silently-dead
    /// hunting pipeline, and gives the per-discrete-attempt rate a
    /// canonical activation footer surface alongside the events
    /// stream. Defaults to `expected_to_fire_per_soak() => true`
    /// because any healthy colony attempts prey ≥1 per 15-min soak.
    HuntAttempted,

    // --- 176: inventory-disposal Features ---
    /// 176: a cat completed a `Discarding` plan — dropped a carried
    /// item on the ground at their current position. Items remain
    /// real entities (`ItemLocation::OnGround`) so the colony hasn't
    /// lost the resource — another cat can plan a `PickingUp` to
    /// retrieve it. Neutral category: a Discarding completion is
    /// neither a colony win nor a loss, just a state transition.
    /// Defaults to `expected_to_fire_per_soak() => false` — the
    /// disposal DSEs ship dormant (default-zero scoring); once
    /// balance-tuning lifts the saturation surfaces this should fire
    /// in healthy colonies but the canary stays opt-in until then.
    ItemDropped,
    /// 176: a cat completed a `Trashing` plan — carried an item to
    /// the nearest Midden building and deposited it. Midden capacity
    /// is unlimited so the deposit never fails on capacity. The
    /// item entity remains real (stored at the midden); future scope:
    /// midden items decay faster via the existing rot ecology.
    /// Neutral / `expected => false` until balance-tuning activates.
    ItemTrashed,
    /// 176: a cat completed a `Handing` plan — transferred a carried
    /// item from their inventory to a target cat's inventory.
    /// Neutral / `expected => false` until balance-tuning activates.
    ItemHandedOff,
    /// 176: an `engage_prey` or `forage_item` resolver couldn't fit
    /// a fresh catch / forage into the cat's full inventory and
    /// dropped it on the ground at the action's position instead.
    /// Items remain real entities (no silent destruction); the
    /// negative classification is the anomaly signal — chronic
    /// `OverflowToGround` correlates with full-inventory cats whose
    /// disposal DSEs aren't clearing the surplus.
    /// `expected_to_fire_per_soak() => false`: in a healthy Maslow-
    /// ascending colony this should be near-zero; non-zero counts
    /// inform the post-soak `verdict` whether disposal is keeping up.
    OverflowToGround,
    /// Ticket 228 — emitted when cat-side path resolution falls back
    /// from per-cat `RouteCostField` gradient walking to A\* with
    /// overlay slices. Fires whenever the field is missing
    /// (pre-flood; despawned and respawned cats), stale (older than
    /// the replan window), or doesn't reach the destination
    /// (cost beyond `MAX_COST_BUDGET`). `expected_to_fire_per_soak()
    /// => false` — a healthy soak should rarely hit this code path
    /// since every replan rebuilds the field; chronic counts indicate
    /// field-staleness or build-correctness bugs.
    RouteCostFieldFallback,
    /// Ticket 230 — `PickFleeTarget` resolved a substrate-aware flee
    /// destination against the cat's per-replan `RouteCostField`. Fires
    /// once per Fleeing-disposition adoption (the first step in the
    /// chain). `expected_to_fire_per_soak() => false`: cascade from
    /// `ThreatProximityAdrenalineFlee` lifting Flee, which itself is
    /// rare on a healthy colony. (Pre-251 also `AcuteHealthAdrenalineFlee`
    /// lifted Flee on injury; 251 retired that lift — but Flee adoption
    /// was already statistical-zero in seed-42 healthy soaks per the
    /// 252 audit, so the retirement does not change Flee's adoption
    /// rate.) Will promote to `true` after the post-230 baseline shows
    /// it firing reliably across seeds.
    FleeTargetPicked,
    /// Ticket 230 — `HoldUntilSafe` completed: the cat held a low-
    /// `RouteCostField` tile with safety need above
    /// `flee_safety_need_threshold` for `flee_hold_ticks`. Closes the
    /// Fleeing trip, which is the canonical signal that the modifier-
    /// driven thrash spiral is broken. `expected_to_fire_per_soak()
    /// => false` initially (cascade from `FleeTargetPicked`); promote
    /// after the soak baseline stabilizes.
    FleeRecovered,

    // --- Ticket 126 BDI intention lifecycle ---
    /// Ticket 126 — L2 evaluator inserted a `HeldIntention` Component
    /// alongside the cat's `GoapPlan` after the softmax winner was
    /// selected. Positive — every adoption is one cat committing to a
    /// goal-shaped intention with a margin-derived strength.
    /// `expected_to_fire_per_soak() => true`: any seed-42 run with at
    /// least one cat picking a non-Idle disposition produces ≥1.
    IntentionAdopted,
    /// Ticket 126 — held intention reached `achievement_believed`
    /// (the §7.2 `DropBranch::Achieved` path) and was cleared.
    /// Positive — every fulfilment is one closed goal cycle. The
    /// `IntentionFulfilled / IntentionAdopted` ratio is the substrate
    /// completion rate. `expected_to_fire_per_soak() => true`.
    IntentionFulfilled,
    /// Ticket 126 — held intention dropped on a non-fulfilment
    /// trigger (preempted, became impossible, target invalid,
    /// expired, or desire drift). Per-cause detail recorded against
    /// the focal-cat trace's `L3Commitment.abandon_reason`; the
    /// activation counter is unparameterised because drops are bursty
    /// (matches `PairingDropped`'s precedent). `expected_to_fire_per_soak()
    /// => false`.
    IntentionAbandoned,
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
        Feature::ModifierPreemption,
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
        Feature::ShadowFoxAvoidedCatScent,
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
        // 035: burial canary.
        Feature::BurialPerformed,
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
        // Ticket 288 — morale_break rebind to commitment release.
        Feature::CommitmentDropMoraleBreak,
        // Ticket 127 — JointIntention (subsumes §7.M L2 PairingActivity
        // from ticket 027b). Each parameterized variant is
        // enumerated per-practice; for 127 only Courtship exists.
        Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        },
        Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        },
        Feature::JointBiasApplied {
            practice: PracticeKind::Courtship,
        },
        Feature::JointStageAdvanced {
            practice: PracticeKind::Courtship,
        },
        Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        },
        // §sub-epic 071 — planning-substrate hardening
        Feature::ReservationContended,
        Feature::TargetCooldownApplied,
        // Ticket 104 — Hide/Freeze DSE infrastructure (dormant in Phase 1).
        Feature::HideFreezeFired,
        Feature::HuntAttempted,
        // 176: inventory-disposal Features. Stage 2 ships them
        // dormant via default-zero scoring on the disposal DSEs;
        // balance-tuning lifts the saturation surfaces in a follow-on.
        Feature::ItemDropped,
        Feature::ItemTrashed,
        Feature::ItemHandedOff,
        Feature::OverflowToGround,
        Feature::RouteCostFieldFallback,
        // 230: Fleeing chain completion signals.
        Feature::FleeTargetPicked,
        Feature::FleeRecovered,
        // 126: BDI intention lifecycle.
        Feature::IntentionAdopted,
        Feature::IntentionFulfilled,
        Feature::IntentionAbandoned,
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
            Feature::ShadowFoxAvoidedCatScent => Positive,
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
            // Ticket 127 — JointIntention positive valences (subsumes
            // §7.M L2 PairingActivity from ticket 027b).
            Feature::JointIntentionEmitted { .. } => Positive,
            Feature::JointBiasApplied { .. } => Positive,
            Feature::JointStageAdvanced { .. } => Positive,
            // Ticket 104 — Hide/Freeze valence
            Feature::HideFreezeFired => Positive,
            // Ticket 149 — discrete hunt attempts (kill or lost) are
            // healthy-colony activity; the never-fired canary catches a
            // dead hunting pipeline.
            Feature::HuntAttempted => Positive,

            // 230: Fleeing chain completions. Both Positive — they
            // signal the substrate-aware Fleeing path is firing
            // (and, in the case of FleeRecovered, completing) instead
            // of the legacy thrash loop.
            Feature::FleeTargetPicked => Positive,
            Feature::FleeRecovered => Positive,

            // 126: BDI intention lifecycle. Adoption + fulfilment
            // are colony-positive (cats are committing and closing
            // goal cycles); abandon is Neutral (drops are state
            // transitions, the per-cause classification lives on the
            // trace not the activation counter).
            Feature::IntentionAdopted => Positive,
            Feature::IntentionFulfilled => Positive,
            Feature::IntentionAbandoned => Neutral,

            // 176: inventory-disposal completions are state-transition
            // signals — neither a colony win nor a loss, just "the cat
            // moved an item." Neutral keeps them out of the
            // positive/negative balance scoring while still surfacing
            // them in the activation footer for the never-fired canary.
            Feature::ItemDropped => Neutral,
            Feature::ItemTrashed => Neutral,
            Feature::ItemHandedOff => Neutral,
            // 250: burial demoted from Positive to Neutral. Caring for
            // the dead is conditional on death occurring; post-247 /
            // 248 the substrate keeps colonies healthy enough that
            // burial is genuinely rare. Treating zero burials as a
            // never-fired-canary defect produced false `verdict: fail`
            // results across post-246 / 247 / 248 baselines. Neutral
            // valence + `expected_to_fire_per_soak() => false` (below)
            // demotes the canary while preserving the footer tally.
            Feature::BurialPerformed => Neutral,

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
            Feature::ModifierPreemption => Negative,
            Feature::AspirationAbandoned => Negative,
            Feature::DenRaided => Negative,
            Feature::PreyDenAbandoned => Negative,
            Feature::DepositRejected => Negative,
            // 176: catches that overflow inventory and end up on the
            // ground. Anomaly signal — chronic counts mean disposal
            // DSEs aren't keeping up with intake. Item entities still
            // exist (items-are-real); only the colony's inventory
            // efficiency suffers.
            Feature::OverflowToGround => Negative,
            Feature::RouteCostFieldFallback => Negative,
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
            Feature::CommitmentDropMoraleBreak => Neutral,
            // §7.M L2 PairingActivity drop is a state transition,
            // not an adverse event.
            // Ticket 127 — JointIntention neutral valences. Drops are
            // bursty state transitions (mirrors the prior
            // PairingDropped which 127 subsumes). Stage mismatch is
            // healthy narrative texture (codified irony), not a
            // regression signal.
            Feature::JointIntentionDropped { .. } => Neutral,
            Feature::JointStageMismatchTickAccrued { .. } => Neutral,
            // Ticket 080 — reservation contention is observability
            // signal, not adverse.
            Feature::ReservationContended => Neutral,
            // Ticket 073 — recently-failed-target cooldown penalty fired
            // for at least one candidate. State-tracking signal, not
            // adverse.
            Feature::TargetCooldownApplied => Neutral,
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
            // 260: shadow-fox cat-scent avoidance — depends on
            // colony scent gradient overlap with ShadowFox patrol
            // route. Same exemption logic as `ShadowFoxAvoidedWard`
            // and `FoxAvoidedPresence`: structural verification lives
            // in the `fox_cat_scent_avoidance` scenario, not the
            // seed-42 canary.
            Feature::ShadowFoxAvoidedCatScent => false,
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
            // Ticket 083 — Farm-dormancy reconciliation. Wave 2
            // substrate hardening + L2 PairingActivity raise the
            // food economy (more efficient hunts and cooking, higher
            // median food_fraction) enough that Farm DSE's
            // CompensatedProduct(food_scarcity, diligence,
            // garden_distance) correctly gates dormant on healthy
            // soaks. The original "silent-dead farming pipeline"
            // class of bug — Farm firing but `tend`/`harvest` no-ops
            // — is now a type/test failure under Phase 5a's
            // `record_if_witnessed` discipline + step-resolver tests
            // on `tend.rs`/`harvest.rs`, not a runtime canary's job.
            // Promote back to `true` when ticket 084 ties Farm to
            // herb/ward stockpile demand so gardens stay productive
            // when food is full but Thornbriar is short.
            Feature::CropTended => false,
            Feature::CropHarvested => false,
            // Ticket 080 — `ReservationContended` is exempt until the
            // producer side (`record_target_picked` writes) ships.
            Feature::ReservationContended => false,
            // Ticket 073 — Neutral feature; promotion to canary waits
            // for empirical baseline data.
            Feature::TargetCooldownApplied => false,
            // Ticket 104 — Hide/Freeze valence is rare-event class
            // and Phase-1 dormant (HideEligible never authored).
            // Promote when the activation system + lift land and
            // canonical seed-42 produces ≥1 freeze per soak.
            Feature::HideFreezeFired => false,
            // 176: inventory-disposal Features ship dormant via
            // default-zero scoring on the disposal DSEs. Promote to
            // `true` only after balance-tuning lifts the saturation
            // surfaces so seed-42 produces ≥1 of each per soak.
            // OverflowToGround stays opt-in — chronic counts are an
            // anomaly signal, but in a balanced colony it should be
            // near-zero so the canary would false-fail.
            Feature::ItemDropped => false,
            Feature::ItemTrashed => false,
            Feature::ItemHandedOff => false,
            Feature::OverflowToGround => false,
            // 228: should rarely fire in a healthy soak (every replan
            // rebuilds the field). Chronic counts indicate field
            // staleness or build-correctness bugs; the canary is the
            // tripwire that surfaces them.
            Feature::RouteCostFieldFallback => false,
            // 230: Fleeing chain Features. Cascade-from-rare-event:
            // `ThreatProximityAdrenalineFlee` should rarely lift Flee
            // to win selection on a healthy colony, so the trip
            // signals stay opt-in. (Pre-251 also
            // `AcuteHealthAdrenalineFlee` lifted Flee on injury — 251
            // retired that lift but Flee adoption was already
            // statistical-zero in seed-42 healthy soaks.) Promote to
            // `true` after the post-230 multi-seed baseline shows
            // reliable firing.
            Feature::FleeTargetPicked => false,
            Feature::FleeRecovered => false,
            // 126: BDI abandon is bursty (per `PairingDropped`'s
            // precedent). Adoption + fulfilment fall through to the
            // `_ => true` default and are canary-validated.
            Feature::IntentionAbandoned => false,
            // Ticket 127 — JointIntention: drops are bursty (mirrors
            // `PairingDropped`); stage mismatch is healthy-sometimes-
            // zero. Both stay opt-out. Emitted / BiasApplied /
            // StageAdvanced fall through to `_ => true`.
            Feature::JointIntentionDropped { .. } => false,
            Feature::JointStageMismatchTickAccrued { .. } => false,
            // 250: burial is conditional on death. Post-247 / 248 the
            // substrate keeps colonies healthy enough that deaths
            // (and therefore burials) are genuinely rare; treating
            // zero burials as a never-fired-canary defect produced
            // false `verdict: fail` results. Demoted alongside the
            // valence change above. Footer tally still emits when
            // burials happen.
            Feature::BurialPerformed => false,
            // 288: morale_break-driven commitment release. Rare-event
            // class (requires a wounded cat to reach EngageThreat with
            // HP below `fight_bail_health_threshold`). Promote to
            // `true` after the post-288 baseline shows reliable firing.
            Feature::CommitmentDropMoraleBreak => false,
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
        Feature::ModifierPreemption => "ModifierPreemption",
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
        Feature::ShadowFoxAvoidedCatScent => "ShadowFoxAvoidedCatScent",
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
        Feature::BurialPerformed => "BurialPerformed",
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
        Feature::CommitmentDropMoraleBreak => "CommitmentDropMoraleBreak",
        // Ticket 127 — JointIntention. Display names embed the practice
        // slug so the footer / canary output distinguishes per-practice
        // counters when future practices land.
        Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        } => "JointIntentionEmitted_Courtship",
        Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        } => "JointIntentionDropped_Courtship",
        Feature::JointBiasApplied {
            practice: PracticeKind::Courtship,
        } => "JointBiasApplied_Courtship",
        Feature::JointStageAdvanced {
            practice: PracticeKind::Courtship,
        } => "JointStageAdvanced_Courtship",
        Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        } => "JointStageMismatchTickAccrued_Courtship",
        Feature::ReservationContended => "ReservationContended",
        Feature::TargetCooldownApplied => "TargetCooldownApplied",
        Feature::HideFreezeFired => "HideFreezeFired",
        Feature::HuntAttempted => "HuntAttempted",
        Feature::ItemDropped => "ItemDropped",
        Feature::ItemTrashed => "ItemTrashed",
        Feature::ItemHandedOff => "ItemHandedOff",
        Feature::OverflowToGround => "OverflowToGround",
        Feature::RouteCostFieldFallback => "RouteCostFieldFallback",
        Feature::FleeTargetPicked => "FleeTargetPicked",
        Feature::FleeRecovered => "FleeRecovered",
        Feature::IntentionAdopted => "IntentionAdopted",
        Feature::IntentionFulfilled => "IntentionFulfilled",
        Feature::IntentionAbandoned => "IntentionAbandoned",
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
        // Ticket 080 added 1 Neutral (ReservationContended) for the
        // resource-reservation substrate. Ticket 073 added 1 Neutral
        // (TargetCooldownApplied) for the RecentTargetFailures cooldown.
        // Ticket 104 added 1 Positive (HideFreezeFired) for the
        // Hide/Freeze valence — Phase-1 dormant via the HideEligible
        // gate, but already classified.
        // Ticket 149 added 1 Positive (HuntAttempted) for the
        // discrete-attempt instrumentation that disambiguates per-Hunt-
        // action rate from per-discrete-attempt rate.
        // Ticket 176 added 1 Negative (OverflowToGround — anomaly
        // signal when engage_prey/forage_item overflow inventory) +
        // 3 Neutral (ItemDropped, ItemTrashed, ItemHandedOff) for the
        // disposal-action surface (Drop / Trash / Handoff).
        // Ticket 118 added 1 Negative (ModifierPreemption — substrate-
        // driven plan preemption from acute-class lurch modifiers).
        // Ticket 228 added 1 Negative (RouteCostFieldFallback — A*
        // fallback observability for the per-cat route-cost field).
        // Ticket 230 added 2 Positive (FleeTargetPicked,
        // FleeRecovered — Fleeing chain end-to-end signals).
        // Ticket 035 added 1 Positive (BurialPerformed — burial
        // continuity-canary signal). Ticket 250 demoted it to
        // Neutral after post-247 / 248 substrate stability made
        // deaths (and therefore burials) genuinely rare in healthy
        // colonies — net 0 to positive count, +1 to neutral.
        // Ticket 126 added 2 Positive (IntentionAdopted,
        // IntentionFulfilled) + 1 Neutral (IntentionAbandoned) for
        // the BDI intention substrate lifecycle.
        // Ticket 127 Commit A added 3 Positive (JointIntentionEmitted,
        // JointBiasApplied, JointStageAdvanced) + 2 Neutral
        // (JointIntentionDropped, JointStageMismatchTickAccrued), all
        // parameterized on PracticeKind (Courtship only in 127).
        // Commit C deleted the 3 PA Features (2 Positive
        // PairingIntentionEmitted/PairingBiasApplied + 1 Neutral
        // PairingDropped), net -2 Positive / -1 Neutral.
        // Ticket 288 added 1 Neutral (CommitmentDropMoraleBreak — the
        // morale_break-driven commitment release counter).
        // Ticket 260 added 1 Positive (ShadowFoxAvoidedCatScent — the
        // substrate-visible scent-channel sibling of the existing
        // ShadowFoxAvoidedWard magic-channel feature; both are
        // exempt from the seed-42 canary because firing depends on
        // ShadowFox/ward/colony spatial overlap).
        assert_eq!(positive, 55);
        assert_eq!(negative, 23);
        assert_eq!(neutral, 35);
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
        // Ticket 250: BurialPerformed demoted from Positive to Neutral
        // (post-247 / 248 substrate makes deaths and burials genuinely
        // rare in healthy colonies), shifting -1 positive / +1 neutral.
        // Ticket 127 Commit A: +3 Positive / +0 Negative / +2 Neutral
        // for the JointIntention substrate (5 parameterized features,
        // Courtship-only in 127). Commit C: -2 Positive / -1 Neutral
        // for the PA Feature deletions (PairingIntentionEmitted /
        // PairingBiasApplied / PairingDropped).
        // Ticket 288: +1 Neutral (CommitmentDropMoraleBreak).
        // Ticket 260: +1 Positive (ShadowFoxAvoidedCatScent — the
        // scent-channel sibling of ShadowFoxAvoidedWard).
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Positive),
            55
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Negative),
            23
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Neutral),
            35
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
        assert!(missing.contains(&"FoodEaten"));
        assert!(missing.contains(&"Socialized"));
        assert!(missing.contains(&"MentoredCat"));
        // Rare-legend features are excluded.
        assert!(!missing.contains(&"ShadowFoxBanished"));
        assert!(!missing.contains(&"FateAwakened"));
        // Ticket 083 — Farm-dormancy reconciliation. Crop features
        // are demoted; they no longer flag the canary.
        assert!(!missing.contains(&"CropTended"));
        assert!(!missing.contains(&"CropHarvested"));
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
        sa.record(Feature::Socialized);
        let after = sa.never_fired_expected_positives();
        assert_eq!(after.len(), before.len() - 2);
        assert!(!after.contains(&"FoodEaten"));
        assert!(!after.contains(&"Socialized"));
    }

    #[test]
    fn expected_to_fire_per_soak_classification() {
        // Core-subsystem trunks must be expected.
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
        // Ticket 127 — JointIntention canary classifications: the
        // three Positive variants carry the per-practice fire-rate
        // expectation (PracticeKind::Courtship in 127); the two
        // Neutral variants (drop + mismatch) are bursty / healthy-
        // sometimes-zero respectively.
        assert!(
            Feature::JointIntentionEmitted {
                practice: PracticeKind::Courtship,
            }
            .expected_to_fire_per_soak()
        );
        assert!(
            Feature::JointBiasApplied {
                practice: PracticeKind::Courtship,
            }
            .expected_to_fire_per_soak()
        );
        assert!(
            Feature::JointStageAdvanced {
                practice: PracticeKind::Courtship,
            }
            .expected_to_fire_per_soak()
        );
        assert!(
            !Feature::JointIntentionDropped {
                practice: PracticeKind::Courtship,
            }
            .expected_to_fire_per_soak()
        );
        assert!(
            !Feature::JointStageMismatchTickAccrued {
                practice: PracticeKind::Courtship,
            }
            .expected_to_fire_per_soak()
        );
        // Rare-legend events must be exempted.
        assert!(!Feature::ShadowFoxBanished.expected_to_fire_per_soak());
        assert!(!Feature::FateAwakened.expected_to_fire_per_soak());
        assert!(!Feature::ScryCompleted.expected_to_fire_per_soak());
        // 250: burial is conditional on death; post-247 / 248
        // substrate keeps colonies healthy enough that deaths are
        // genuinely rare. Demoted to neutral + exempted from the
        // never-fired canary.
        assert!(!Feature::BurialPerformed.expected_to_fire_per_soak());
        assert_eq!(
            Feature::BurialPerformed.category(),
            FeatureCategory::Neutral
        );
        // Cascade-exempt: silent strictly because their trunk is
        // silent. Promoting them to expected would multiply one
        // root-cause failure into N canary entries.
        assert!(!Feature::GestationAdvanced.expected_to_fire_per_soak());
        assert!(!Feature::KittenBorn.expected_to_fire_per_soak());
        assert!(!Feature::KittenFed.expected_to_fire_per_soak());
        assert!(!Feature::ItemRetrieved.expected_to_fire_per_soak());
        // Ticket 083 — Farm-dormancy reconciliation. Demoted: the
        // food-economy lift correctly silences Farm via its
        // CompensatedProduct gate. Re-promote when ticket 084 ties
        // Farm to herb/ward demand.
        assert!(!Feature::CropTended.expected_to_fire_per_soak());
        assert!(!Feature::CropHarvested.expected_to_fire_per_soak());
    }
}
